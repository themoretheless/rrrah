//! Four-plane RGBE reference interpolation. Input samples must already be
//! normalized with explicitly resolved black/white levels and four WB gains.
//! Output retains all four camera planes; color conversion is a separate step.
/// Bilinear reconstruction on each plane's two-pixel lattice. At sensor edges,
/// extend the nearest site of the same plane, preserving CFA phase. No allocation.
pub fn interpolate_planes(
    quad: [u32; 4],
    position: [u32; 2],
    dimensions: [u32; 2],
    sample: impl Fn(u32, u32) -> f64,
) -> Option<[f64; 4]> {
    if dimensions.iter().any(|&v| v < 2) || position[0] >= dimensions[0] || position[1] >= dimensions[1] {
        return None;
    }
    let mut seen = [false; 4];
    for channel in quad {
        let flag = seen.get_mut(channel as usize)?;
        if *flag {
            return None;
        }
        *flag = true;
    }
    fn axis(coordinate: u32, phase: u32, size: u32) -> (u32, u32, f64) {
        let last = phase + (size - 1 - phase) / 2 * 2;
        let lower = (phase + coordinate.saturating_sub(phase) / 2 * 2).min(last);
        let upper = lower.saturating_add(2).min(last);
        let weight = if lower == upper {
            0.
        } else {
            ((coordinate as f64 - lower as f64) / (upper - lower) as f64).clamp(0., 1.)
        };
        (lower, upper, weight)
    }
    let mut output = [0f64; 4];
    for (phase, channel) in quad.into_iter().enumerate() {
        let (x0, x1, tx) = axis(position[0], (phase % 2) as u32, dimensions[0]);
        let (y0, y1, ty) = axis(position[1], (phase / 2) as u32, dimensions[1]);
        let values = [sample(x0, y0), sample(x1, y0), sample(x0, y1), sample(x1, y1)];
        if values.iter().any(|v| !v.is_finite()) {
            return None;
        }
        let blend = |a: f64, b: f64, t: f64| a * (1. - t) + b * t;
        let value = blend(
            blend(values[0], values[1], tx),
            blend(values[2], values[3], tx),
            ty,
        );
        if !value.is_finite() {
            return None;
        }
        output[channel as usize] = value;
    }
    Some(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    const QUAD: [u32; 4] = [3, 0, 2, 1];
    #[test]
    fn distinct_constants_hdr_and_edges_preserve_each_plane() {
        let levels = [0.1, 0.7, 2.4, 1.3];
        for size in [[2, 2], [3, 5], [12, 10]] {
            for y in 0..size[1] {
                for x in 0..size[0] {
                    assert_eq!(
                        interpolate_planes(QUAD, [x, y], size, |sx, sy| levels
                            [QUAD[((sy % 2) * 2 + sx % 2) as usize] as usize])
                        .unwrap(),
                        levels
                    );
                }
            }
        }
        assert_eq!(
            interpolate_planes(QUAD, [3, 3], [8, 8], |_, _| f64::MAX).unwrap(),
            [f64::MAX; 4]
        );
    }
    #[test]
    fn analytic_plane_ramps_reconstruct_and_measured_sites_remain_exact() {
        let plane = |channel: u32, x: u32, y: u32| {
            10. + channel as f64 + (channel + 1) as f64 * x as f64 + (4 - channel) as f64 * y as f64
        };
        for y in 2..8 {
            for x in 2..10 {
                let output = interpolate_planes(QUAD, [x, y], [12, 10], |sx, sy| {
                    plane(QUAD[((sy % 2) * 2 + sx % 2) as usize], sx, sy)
                })
                .unwrap();
                for channel in 0..4 {
                    assert_eq!(output[channel as usize], plane(channel, x, y));
                }
            }
        }
        for y in 0..10 {
            for x in 0..12 {
                let output = interpolate_planes(QUAD, [x, y], [12, 10], |sx, sy| {
                    plane(QUAD[((sy % 2) * 2 + sx % 2) as usize], sx, sy)
                })
                .unwrap();
                let measured = QUAD[((y % 2) * 2 + x % 2) as usize];
                assert_eq!(output[measured as usize], plane(measured, x, y));
            }
        }
    }
    #[test]
    fn invalid_geometry_planes_or_nonfinite_samples_are_rejected() {
        for quad in [[0, 1, 1, 2], [0, 1, 2, 4], [255, 0, 1, 2]] {
            assert!(interpolate_planes(quad, [0, 0], [2, 2], |_, _| 1.).is_none());
        }
        for (position, size) in [([0, 0], [1, 2]), ([2, 0], [2, 2]), ([0, 2], [2, 2])] {
            assert!(interpolate_planes(QUAD, position, size, |_, _| 1.).is_none());
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(interpolate_planes(QUAD, [0, 0], [2, 2], |_, _| value).is_none());
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CalibrationError {
    #[error(transparent)]
    Frame(#[from] crate::FrameError),
    #[error(transparent)]
    Profile(#[from] crate::CameraProfileError),
    #[error(transparent)]
    Memory(#[from] crate::BufferError),
    #[error("invalid RGBE calibration: {0}")]
    Invalid(&'static str),
    #[error("RGBE normalization cancelled")]
    Cancelled,
}
/// Explicit source calibration. Levels are sensor-phase ordered; WB and matrix
/// planes are R/G/B/E ordered. Only scalar or exact 2x2 level grids are accepted.
#[derive(Debug, Clone)]
pub struct Calibration {
    dimensions: [u32; 2],
    quad: [u32; 4],
    camera_to_rgb: [[f64; 4]; 3],
    black: [f32; 4],
    white: [f32; 4],
    gains: [f32; 4],
}
impl Calibration {
    pub fn new(metadata: &crate::RawMetadata) -> Result<Self, CalibrationError> {
        metadata.validate()?;
        if metadata.photometric != crate::Photometric::Cfa || metadata.width < 2 || metadata.height < 2 {
            return Err(CalibrationError::Invalid("requires a four-plane CFA sensor"));
        }
        let quad = metadata
            .cfa
            .as_ref()
            .ok_or(CalibrationError::Invalid("missing CFA"))?
            .rgbe_quad()?;
        let grid = &metadata.black_level;
        let black = if grid.width == 1 && grid.height == 1 && grid.components == 1 {
            [grid.values[0]; 4]
        } else if grid.width == 2 && grid.height == 2 && grid.components == 1 {
            grid.values[..].try_into().unwrap()
        } else {
            return Err(CalibrationError::Invalid("unsupported black-level layout"));
        };
        let white = match metadata.white_level.0.as_slice() {
            [one] => [*one; 4],
            values if values.len() == 4 => values.try_into().unwrap(),
            _ => return Err(CalibrationError::Invalid("unsupported white-level layout")),
        };
        for phase in 0..4 {
            let range = white[phase] - black[phase];
            let largest =
                (65535. - black[phase]).max(0.) / range * metadata.white_balance[quad[phase] as usize];
            if !range.is_finite() || range <= 0. || !largest.is_finite() {
                return Err(CalibrationError::Invalid(
                    "level/gain range is not finite and positive",
                ));
            }
        }
        let camera_to_rgb =
            crate::camera4_to_linear_srgb_precise(metadata.xyz_to_camera.map(|r| r.map(f64::from)))?;
        Ok(Self {
            dimensions: [metadata.width, metadata.height],
            quad,
            camera_to_rgb,
            black,
            white,
            gains: metadata.white_balance,
        })
    }
    pub fn dimensions(&self) -> [u32; 2] {
        self.dimensions
    }
    pub fn quad(&self) -> [u32; 4] {
        self.quad
    }
    pub fn camera_to_rgb(&self) -> [[f64; 4]; 3] {
        self.camera_to_rgb
    }
    /// Convert reconstructed R/G/B/E planes to scene-linear sRGB. Negative
    /// color components and HDR values are retained for the display pipeline.
    /// Returns None for nonfinite inputs or arithmetic overflow.
    pub fn linear_rgb(&self, planes: [f64; 4]) -> Option<[f64; 3]> {
        if planes.iter().any(|v| !v.is_finite()) {
            return None;
        }
        let rgb = self.camera_to_rgb.map(|row| {
            row.into_iter()
                .zip(planes)
                .map(|(coefficient, sample)| coefficient * sample)
                .sum::<f64>()
        });
        rgb.iter().all(|v| v.is_finite()).then_some(rgb)
    }
    /// Reconstruct one sensor-coordinate pixel directly from calibrated u16
    /// samples, without allocating a full normalized or RGBA sensor buffer.
    pub fn reconstruct_linear_rgb(
        &self,
        position: [u32; 2],
        sample: impl Fn(u32, u32) -> u16,
    ) -> Option<[f64; 3]> {
        let planes = interpolate_planes(self.quad, position, self.dimensions, |x, y| {
            f64::from(self.normalize_site(x, y, sample(x, y)).unwrap())
        })?;
        self.linear_rgb(planes)
    }
    pub fn normalize_site(&self, x: u32, y: u32, value: u16) -> Option<f32> {
        if x >= self.dimensions[0] || y >= self.dimensions[1] {
            return None;
        }
        let phase = ((y % 2) * 2 + x % 2) as usize;
        Some(
            (f32::from(value) - self.black[phase]).max(0.) / (self.white[phase] - self.black[phase])
                * self.gains[self.quad[phase] as usize],
        )
    }
    pub fn normalize_managed(
        mosaic: &crate::DecodedMosaic,
        budget: &crate::MemoryBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, crate::PixelBuffer<f32>), CalibrationError> {
        if cancelled() {
            return Err(CalibrationError::Cancelled);
        }
        let calibration = Self::new(&mosaic.metadata)?;
        let expected = (calibration.dimensions[0] as usize)
            .checked_mul(calibration.dimensions[1] as usize)
            .ok_or(crate::FrameError::DimensionOverflow)?;
        if mosaic.pixels.len() != expected {
            return Err(crate::FrameError::InvalidPixelCount {
                expected,
                actual: mosaic.pixels.len(),
            }
            .into());
        }
        let mut output = budget.try_buffer(expected, 0f32)?;
        let width = calibration.dimensions[0] as usize;
        for (y, (source, destination)) in mosaic
            .pixels
            .chunks_exact(width)
            .zip(output.chunks_exact_mut(width))
            .enumerate()
        {
            if cancelled() {
                return Err(CalibrationError::Cancelled);
            }
            for (x, (sample, result)) in source.iter().zip(destination).enumerate() {
                *result = calibration.normalize_site(x as u32, y as u32, *sample).unwrap();
            }
        }
        Ok((calibration, output.freeze().into()))
    }
}

#[cfg(test)]
mod calibration_tests {
    use super::*;
    use crate::{
        CfaColor::*, CfaPattern, DecodedMosaic, LevelGrid, MemoryBudget, Orientation, Photometric,
        RawMetadata, WhiteLevel,
    };
    fn metadata() -> RawMetadata {
        RawMetadata {
            make: "synthetic".into(),
            model: "RGBE".into(),
            width: 2,
            height: 2,
            components_per_pixel: 1,
            bits_per_sample: 16,
            photometric: Photometric::Cfa,
            cfa: Some(CfaPattern {
                width: 2,
                height: 2,
                cells: vec![Emerald, Red, Blue, Green],
            }),
            black_level: LevelGrid {
                width: 2,
                height: 2,
                components: 1,
                values: vec![10., 20., 30., 40.],
            },
            white_level: WhiteLevel(vec![110., 220., 330., 440.]),
            white_balance: [2., 3., 4., 5.],
            xyz_to_camera: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0., 1., 0.]],
            active_area: None,
            crop_area: None,
            orientation: Orientation::Normal,
        }
    }
    #[test]
    fn source_phase_levels_and_four_gains_preserve_hdr() {
        let c = Calibration::new(&metadata()).unwrap();
        assert_eq!(c.quad(), [3, 0, 2, 1]);
        for (x, y, black, white, gain) in [
            (0, 0, 10, 110, 5.),
            (1, 0, 20, 220, 2.),
            (0, 1, 30, 330, 4.),
            (1, 1, 40, 440, 3.),
        ] {
            assert_eq!(c.normalize_site(x, y, 0), Some(0.));
            assert_eq!(c.normalize_site(x, y, black), Some(0.));
            assert_eq!(c.normalize_site(x, y, white), Some(gain));
            assert_eq!(c.normalize_site(x, y, 2 * white - black), Some(2. * gain));
        }
        assert_eq!(c.normalize_site(2, 0, 100), None);
        assert_eq!(c.dimensions(), [2, 2]);
        assert!(c.camera_to_rgb().iter().flatten().all(|v| v.is_finite()));
    }
    #[test]
    fn all_four_color_terms_negative_and_hdr_are_preserved() {
        let mut c = Calibration::new(&metadata()).unwrap();
        // Authored independent coefficients make loss/merging of E observable.
        c.camera_to_rgb = [[1., 2., 3., 4.], [-1., 0., 0., -2.], [0., 0., 1., 0.5]];
        assert_eq!(c.linear_rgb([2., 3., 4., 5.]), Some([40., -12., 6.5]));
        assert_eq!(c.linear_rgb([0., 0., 0., 1.]), Some([4., -2., 0.5]));
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(c.linear_rgb([0., 0., 0., bad]).is_none());
        }
        assert!(c.linear_rgb([f64::MAX; 4]).is_none());
        // The four 2x2 measured sites represent R=2 G=3 B=4 E=5.
        let source = [110, 220, 330, 440];
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(
                    c.reconstruct_linear_rgb([x, y], |sx, sy| source[(sy * 2 + sx) as usize]),
                    Some([40., -12., 6.5])
                );
            }
        }
        let calls = std::cell::Cell::new(0);
        assert!(
            c.reconstruct_linear_rgb([2, 0], |_, _| {
                calls.set(calls.get() + 1);
                0
            })
            .is_none()
        );
        assert_eq!(calls.get(), 0);
    }
    #[test]
    fn managed_thumbnail_reserves_before_output_and_releases_on_failure() {
        let mosaic = DecodedMosaic::new(metadata(), std::sync::Arc::new(vec![110, 220, 330, 440])).unwrap();
        let denied = MemoryBudget::new(15);
        assert!(matches!(
            mosaic.thumbnail_rgba8_managed(2, &denied),
            Err(crate::ThumbnailError::Memory(_))
        ));
        assert_eq!(denied.peak(), 0);
        let budget = MemoryBudget::new(16);
        let pixels = mosaic.thumbnail_rgba8_managed(2, &budget).unwrap();
        assert_eq!(&pixels[..], mosaic.thumbnail_rgba8(2));
        assert!(pixels.is_managed());
        let held = pixels.clone();
        drop(pixels);
        assert_eq!(budget.used(), 16);
        drop(held);
        assert_eq!(budget.used(), 0);
        let mut unsupported = mosaic;
        unsupported.metadata.xyz_to_camera = [[0.; 3]; 4];
        assert!(matches!(
            unsupported.thumbnail_rgba8_managed(2, &budget),
            Err(crate::ThumbnailError::Develop)
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn cancelled_thumbnail_releases_admitted_preview() {
        let mosaic = DecodedMosaic::new(metadata(), std::sync::Arc::new(vec![110, 220, 330, 440])).unwrap();
        let budget = MemoryBudget::new(16);
        assert!(matches!(
            mosaic.thumbnail_rgba8_managed_with_cancel(2, &budget, &|| true),
            Err(crate::ThumbnailError::Cancelled)
        ));
        assert_eq!(budget.peak(), 0);
        let calls = std::cell::Cell::new(0);
        let cancelled = || {
            let n = calls.get();
            calls.set(n + 1);
            n >= 2
        };
        assert!(matches!(
            mosaic.thumbnail_rgba8_managed_with_cancel(2, &budget, &cancelled),
            Err(crate::ThumbnailError::Cancelled)
        ));
        assert_eq!(budget.peak(), 16);
        assert_eq!(budget.used(), 0);
        assert!(mosaic.thumbnail_rgba8_with_cancel(2, &|| true).is_empty());
    }
    #[test]
    fn malformed_calibration_is_rejected() {
        let mut m = metadata();
        m.white_level.0[0] = 10.;
        assert!(Calibration::new(&m).is_err());
        let mut m = metadata();
        m.white_balance[3] = f32::MAX;
        assert!(Calibration::new(&m).is_err());
        let mut m = metadata();
        m.xyz_to_camera = [[0.; 3]; 4];
        assert!(Calibration::new(&m).is_err());
        let mut m = metadata();
        m.black_level.width = 4;
        m.black_level.height = 1;
        assert!(Calibration::new(&m).is_err());
    }
    #[test]
    fn managed_admission_cancel_and_last_owner_release() {
        let mosaic = DecodedMosaic::new(metadata(), std::sync::Arc::new(vec![110, 220, 330, 440])).unwrap();
        let budget = MemoryBudget::new(16);
        assert!(matches!(
            Calibration::normalize_managed(&mosaic, &budget, &|| true),
            Err(CalibrationError::Cancelled)
        ));
        assert_eq!(budget.peak(), 0);
        assert!(Calibration::normalize_managed(&mosaic, &MemoryBudget::new(15), &|| false).is_err());
        let calls = std::cell::Cell::new(0);
        let cancel = || {
            let n = calls.get();
            calls.set(n + 1);
            n == 2
        };
        assert!(matches!(
            Calibration::normalize_managed(&mosaic, &budget, &cancel),
            Err(CalibrationError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        let (_, output) = Calibration::normalize_managed(&mosaic, &budget, &|| false).unwrap();
        assert_eq!(&output[..], &[5., 2., 4., 3.]);
        let held = output.clone();
        drop(output);
        assert_eq!(budget.used(), 16);
        drop(held);
        assert_eq!(budget.used(), 0);
        let mut invalid = mosaic;
        invalid.metadata.height = 3;
        assert!(matches!(
            Calibration::normalize_managed(&invalid, &budget, &|| false),
            Err(CalibrationError::Frame(
                crate::FrameError::InvalidPixelCount { .. }
            ))
        ));
        assert_eq!(budget.used(), 0);
    }
}
