//! Sensor-coordinate Bayer calibration for native compute input.
//! Crop/orientation and display transfer remain presentation operations.
use crate::{DecodedMosaic, FrameError, MemoryBudget, PixelBuffer, RawMetadata};

#[derive(Debug, thiserror::Error)]
pub enum CalibrationError {
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error(transparent)]
    Memory(#[from] crate::BufferError),
    #[error("invalid Bayer calibration: {0}")]
    Invalid(&'static str),
    #[error("Bayer normalization cancelled")]
    Cancelled,
}

/// Borrows the source metadata: repeat grids are neither copied nor allocated.
#[derive(Debug)]
pub struct Calibration<'a> {
    metadata: &'a RawMetadata,
    quad: [u32; 4],
    white: [f32; 4],
    phase_gains: [f32; 4],
    camera_to_rgb: [[f32; 3]; 3],
}
impl<'a> Calibration<'a> {
    pub fn new(metadata: &'a RawMetadata) -> Result<Self, CalibrationError> {
        Self::new_with_cancel(metadata, &mut || false)
    }
    fn new_with_cancel(
        metadata: &'a RawMetadata,
        cancelled: &mut impl FnMut() -> bool,
    ) -> Result<Self, CalibrationError> {
        metadata.validate()?;
        if metadata.photometric != crate::Photometric::Cfa || metadata.width < 2 || metadata.height < 2 {
            return Err(CalibrationError::Invalid("requires a Bayer CFA sensor"));
        }
        let cfa = metadata
            .cfa
            .as_ref()
            .ok_or(CalibrationError::Invalid("missing CFA"))?;
        let quad = cfa.bayer_quad()?;
        let white = metadata.white_level.bayer_quad(cfa)?;
        let first_green = quad.iter().position(|&c| c == 1).unwrap();
        let phase_gains = std::array::from_fn(|phase| {
            let channel = if quad[phase] == 1 && phase != first_green {
                3
            } else {
                quad[phase] as usize
            };
            metadata.white_balance[channel]
        });
        let grid = &metadata.black_level;
        if grid.components != 1 {
            return Err(CalibrationError::Invalid("black grid must have one component"));
        }
        // Odd repeat periods change CFA parity; inspect their joint period.
        for y in 0..u32::from(grid.height) * 2 {
            if cancelled() {
                return Err(CalibrationError::Cancelled);
            }
            for x in 0..u32::from(grid.width) * 2 {
                let phase = ((y % 2) * 2 + x % 2) as usize;
                let range = white[phase] - grid.at(y, x, 0).unwrap();
                if !range.is_finite() || range <= 0.0 {
                    return Err(CalibrationError::Invalid(
                        "black/white range must be finite and positive",
                    ));
                }
            }
        }
        let camera_to_rgb = crate::camera_to_linear_srgb(metadata.xyz_to_camera)
            .ok_or(CalibrationError::Invalid("invalid camera matrix"))?;
        if camera_to_rgb.iter().flatten().any(|v| !v.is_finite()) {
            return Err(CalibrationError::Invalid("nonfinite camera transform"));
        }
        Ok(Self {
            metadata,
            quad,
            white,
            phase_gains,
            camera_to_rgb,
        })
    }
    pub fn quad(&self) -> [u32; 4] {
        self.quad
    }
    pub fn camera_to_rgb(&self) -> [[f32; 3]; 3] {
        self.camera_to_rgb
    }
    /// Preserve native WB scale and values above file white. Only sensor black is clamped.
    pub fn normalize_site(&self, x: u32, y: u32, encoded: u16) -> Option<f32> {
        if x >= self.metadata.width || y >= self.metadata.height {
            return None;
        }
        let phase = ((y % 2) * 2 + x % 2) as usize;
        let black = self.metadata.black_level.at(y, x, 0)?;
        let value =
            (f32::from(encoded) - black).max(0.0) / (self.white[phase] - black) * self.phase_gains[phase];
        value.is_finite().then_some(value)
    }
    pub fn normalize_managed(
        mosaic: &'a DecodedMosaic,
        budget: &MemoryBudget,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<(Self, PixelBuffer<f32>), CalibrationError> {
        if cancelled() {
            return Err(CalibrationError::Cancelled);
        }
        let calibration = Self::new_with_cancel(&mosaic.metadata, &mut cancelled)?;
        let expected = (mosaic.metadata.width as usize)
            .checked_mul(mosaic.metadata.height as usize)
            .ok_or(FrameError::DimensionOverflow)?;
        if mosaic.pixels.len() != expected {
            return Err(FrameError::InvalidPixelCount {
                expected,
                actual: mosaic.pixels.len(),
            }
            .into());
        }
        let mut output = budget.try_buffer(expected, 0.0f32)?;
        for (index, (&encoded, normalized)) in mosaic.pixels.iter().zip(output.iter_mut()).enumerate() {
            if index % 4096 == 0 && cancelled() {
                return Err(CalibrationError::Cancelled);
            }
            *normalized = calibration
                .normalize_site(
                    (index % mosaic.metadata.width as usize) as u32,
                    (index / mosaic.metadata.width as usize) as u32,
                    encoded,
                )
                .ok_or(CalibrationError::Invalid("normalized sample exceeds float range"))?;
        }
        if cancelled() {
            return Err(CalibrationError::Cancelled);
        }
        Ok((calibration, output.freeze().into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CfaColor::*, CfaPattern, LevelGrid, Orientation, Photometric, WhiteLevel};
    fn mosaic(cells: Vec<crate::CfaColor>) -> DecodedMosaic {
        let inverse = crate::invert_3x3(crate::SRGB_TO_XYZ_D65).unwrap();
        let metadata = RawMetadata {
            make: "authored".into(),
            model: "Bayer calibration".into(),
            width: 17,
            height: 13,
            components_per_pixel: 1,
            bits_per_sample: 16,
            photometric: Photometric::Cfa,
            cfa: Some(CfaPattern {
                width: 2,
                height: 2,
                cells,
            }),
            black_level: LevelGrid {
                width: 3,
                height: 5,
                components: 1,
                values: (0..15).map(|i| (i * 2) as f32).collect(),
            },
            white_level: WhiteLevel(vec![100., 200., 300., 400.]),
            white_balance: [2., 3., 4., 5.],
            xyz_to_camera: [inverse[0], inverse[1], inverse[2], [0.; 3]],
            active_area: None,
            crop_area: None,
            orientation: Orientation::Normal,
        };
        let values = (0..13)
            .flat_map(|y| {
                let metadata = &metadata;
                (0..17).map(move |x| {
                    let phase = ((y % 2) * 2 + x % 2) as usize;
                    (2. * metadata.white_level.0[phase] - metadata.black_level.at(y, x, 0).unwrap()) as u16
                })
            })
            .collect::<Vec<_>>();
        DecodedMosaic::new(metadata, std::sync::Arc::new(values)).unwrap()
    }
    #[test]
    fn odd_repeat_grid_and_distinct_green_gains_preserve_hdr_all_cfa() {
        for cells in [
            vec![Red, Green, Green, Blue],
            vec![Blue, Green, Green, Red],
            vec![Green, Red, Blue, Green],
            vec![Green, Blue, Red, Green],
        ] {
            let mosaic = mosaic(cells);
            let budget = MemoryBudget::new(17 * 13 * 4);
            let (calibration, normalized) =
                Calibration::normalize_managed(&mosaic, &budget, || false).unwrap();
            for (row, values) in calibration.camera_to_rgb().iter().enumerate() {
                for (column, &value) in values.iter().enumerate() {
                    let expected = if row == column { 1.0 } else { 0.0 };
                    assert!((value - expected).abs() < 1e-5);
                }
            }
            let quad = calibration.quad();
            let first_green = quad.iter().position(|&c| c == 1).unwrap();
            for y in 0..13 {
                for x in 0..17 {
                    let phase = ((y % 2) * 2 + x % 2) as usize;
                    let channel = if quad[phase] == 1 && phase != first_green {
                        3
                    } else {
                        quad[phase] as usize
                    };
                    assert_eq!(normalized[(y * 17 + x) as usize], [4., 6., 8., 10.][channel]);
                    assert_eq!(calibration.normalize_site(x, y, 0), Some(0.));
                }
            }
            assert_eq!(calibration.normalize_site(17, 0, 100), None);
            let shared = normalized.clone();
            drop(normalized);
            assert_eq!(budget.used(), 17 * 13 * 4);
            drop(shared);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn admission_cancel_invalid_levels_and_retry_release_output() {
        let mosaic = mosaic(vec![Red, Green, Green, Blue]);
        let denied = MemoryBudget::new(17 * 13 * 4 - 1);
        assert!(matches!(
            Calibration::normalize_managed(&mosaic, &denied, || false),
            Err(CalibrationError::Memory(_))
        ));
        assert_eq!(denied.used(), 0);
        let budget = MemoryBudget::new(17 * 13 * 4);
        assert!(matches!(
            Calibration::normalize_managed(&mosaic, &budget, || budget.used() > 0),
            Err(CalibrationError::Cancelled)
        ));
        assert_eq!(budget.peak(), 17 * 13 * 4);
        assert_eq!(budget.used(), 0);
        let (_, retry) = Calibration::normalize_managed(&mosaic, &budget, || false).unwrap();
        assert_eq!(retry[0], 4.);
        drop(retry);
        assert_eq!(budget.used(), 0);
        let mut bad = mosaic.clone();
        bad.metadata.black_level.values[14] = 100.;
        assert!(Calibration::normalize_managed(&bad, &budget, || false).is_err());
        assert_eq!(budget.used(), 0);
        bad = mosaic.clone();
        bad.metadata.xyz_to_camera = [[0.; 3]; 4];
        assert!(Calibration::normalize_managed(&bad, &budget, || false).is_err());
        bad = mosaic;
        bad.pixels = std::sync::Arc::new(vec![1u16; 3]).into();
        assert!(matches!(
            Calibration::normalize_managed(&bad, &budget, || false),
            Err(CalibrationError::Frame(FrameError::InvalidPixelCount { .. }))
        ));
        assert_eq!(budget.used(), 0);
    }
}
