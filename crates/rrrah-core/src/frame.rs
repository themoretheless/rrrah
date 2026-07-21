use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CfaColor {
    Red = 0,
    Green = 1,
    Blue = 2,
    Cyan = 3,
    Magenta = 4,
    Yellow = 5,
    White = 6,
    Unknown = 255,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfaPattern {
    pub width: u8,
    pub height: u8,
    pub cells: Vec<CfaColor>,
}

impl CfaPattern {
    pub fn color_at(&self, row: u32, column: u32) -> Option<CfaColor> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let x = column % u32::from(self.width);
        let y = row % u32::from(self.height);
        self.cells.get((y * u32::from(self.width) + x) as usize).copied()
    }

    /// Returns a row-major 2x2 RGB Bayer quad encoded for WGSL.
    pub fn bayer_quad(&self) -> Result<[u32; 4], FrameError> {
        if self.width != 2 || self.height != 2 || self.cells.len() != 4 {
            return Err(FrameError::UnsupportedCfa {
                width: self.width,
                height: self.height,
            });
        }
        let mut encoded = [0_u32; 4];
        for (dst, color) in encoded.iter_mut().zip(&self.cells) {
            *dst = match color {
                CfaColor::Red => 0,
                CfaColor::Green => 1,
                CfaColor::Blue => 2,
                _ => {
                    return Err(FrameError::UnsupportedCfa {
                        width: self.width,
                        height: self.height,
                    });
                }
            };
        }
        let red = encoded.iter().filter(|&&value| value == 0).count();
        let green = encoded.iter().filter(|&&value| value == 1).count();
        let blue = encoded.iter().filter(|&&value| value == 2).count();
        if (red, green, blue) != (1, 2, 1) {
            return Err(FrameError::UnsupportedCfa {
                width: self.width,
                height: self.height,
            });
        }
        Ok(encoded)
    }

    pub fn validate(&self) -> Result<(), FrameError> {
        let expected = usize::from(self.width)
            .checked_mul(usize::from(self.height))
            .ok_or(FrameError::DimensionOverflow)?;
        if expected == 0 || expected != self.cells.len() {
            return Err(FrameError::InvalidCfaLength {
                expected,
                actual: self.cells.len(),
            });
        }
        Ok(())
    }
}

/// A repeating black-level grid. Values are row-major, then component-major.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelGrid {
    pub width: u8,
    pub height: u8,
    pub components: u8,
    pub values: Vec<f32>,
}

impl LevelGrid {
    pub fn validate(&self) -> Result<(), FrameError> {
        let expected = usize::from(self.width)
            .checked_mul(usize::from(self.height))
            .and_then(|value| value.checked_mul(usize::from(self.components)))
            .ok_or(FrameError::DimensionOverflow)?;
        if expected == 0 || expected != self.values.len() {
            return Err(FrameError::InvalidLevelGrid {
                expected,
                actual: self.values.len(),
            });
        }
        if self.values.iter().any(|value| !value.is_finite()) {
            return Err(FrameError::NonFiniteMetadata("black level"));
        }
        Ok(())
    }

    pub fn at(&self, row: u32, column: u32, component: u8) -> Option<f32> {
        if self.width == 0 || self.height == 0 || component >= self.components {
            return None;
        }
        let x = (column % u32::from(self.width)) as usize;
        let y = (row % u32::from(self.height)) as usize;
        let index = (y * usize::from(self.width) + x) * usize::from(self.components) + usize::from(component);
        self.values.get(index).copied()
    }

    pub fn bayer_quad(&self) -> Result<[f32; 4], FrameError> {
        self.validate()?;
        if self.values.len() == 1 {
            return Ok([self.values[0]; 4]);
        }
        if self.width == 2 && self.height == 2 && self.components == 1 {
            return Ok([self.values[0], self.values[1], self.values[2], self.values[3]]);
        }
        // Preserve a position-dependent approximation for unusual DNG repeat
        // grids. The architecture keeps the full grid so a tiled shader can
        // consume it later; the first viewport shader samples its top-left 2x2.
        Ok([
            self.at(0, 0, 0).unwrap_or(0.0),
            self.at(0, 1, 0).unwrap_or(0.0),
            self.at(1, 0, 0).unwrap_or(0.0),
            self.at(1, 1, 0).unwrap_or(0.0),
        ])
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhiteLevel(pub Vec<f32>);

impl WhiteLevel {
    pub fn bayer_quad(&self, cfa: &CfaPattern) -> Result<[f32; 4], FrameError> {
        if self.0.is_empty() || self.0.iter().any(|value| !value.is_finite()) {
            return Err(FrameError::NonFiniteMetadata("white level"));
        }
        if self.0.len() == 1 {
            return Ok([self.0[0]; 4]);
        }
        if self.0.len() == 4 {
            return Ok([self.0[0], self.0[1], self.0[2], self.0[3]]);
        }
        let quad = cfa.bayer_quad()?;
        let mut result = [0.0; 4];
        for (index, color) in quad.into_iter().enumerate() {
            result[index] = *self.0.get(color as usize).unwrap_or(&self.0[0]);
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    Normal,
    HorizontalFlip,
    Rotate180,
    VerticalFlip,
    Transpose,
    Rotate90,
    Transverse,
    Rotate270,
    Unknown,
}

impl Orientation {
    pub const fn swaps_dimensions(self) -> bool {
        matches!(
            self,
            Self::Transpose | Self::Rotate90 | Self::Transverse | Self::Rotate270
        )
    }

    pub const fn shader_code(self) -> u32 {
        match self {
            Self::Normal | Self::Unknown => 0,
            Self::HorizontalFlip => 1,
            Self::Rotate180 => 2,
            Self::VerticalFlip => 3,
            Self::Transpose => 4,
            Self::Rotate90 => 5,
            Self::Transverse => 6,
            Self::Rotate270 => 7,
        }
    }

    /// Maps normalized display coordinates back to sensor/crop coordinates.
    ///
    /// The mapping follows the EXIF/TIFF orientation convention used by the
    /// decoder and by the WGSL viewport shader. Keeping it here gives CPU
    /// tile scheduling and GPU rendering one canonical transform to test.
    pub fn map_display_uv(self, uv: [f32; 2]) -> [f32; 2] {
        let [u, v] = uv;
        match self {
            Self::HorizontalFlip => [1.0 - u, v],
            Self::Rotate180 => [1.0 - u, 1.0 - v],
            Self::VerticalFlip => [u, 1.0 - v],
            Self::Transpose => [v, u],
            Self::Rotate90 => [1.0 - v, u],
            Self::Transverse => [1.0 - v, 1.0 - u],
            Self::Rotate270 => [v, 1.0 - u],
            Self::Normal | Self::Unknown => [u, v],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Photometric {
    Cfa,
    LinearRaw,
    BlackIsZero,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawMetadata {
    pub make: String,
    pub model: String,
    pub width: u32,
    pub height: u32,
    pub components_per_pixel: u8,
    pub bits_per_sample: u8,
    pub photometric: Photometric,
    pub cfa: Option<CfaPattern>,
    pub black_level: LevelGrid,
    pub white_level: WhiteLevel,
    pub white_balance: [f32; 4],
    pub xyz_to_camera: [[f32; 3]; 4],
    pub active_area: Option<Rect>,
    pub crop_area: Option<Rect>,
    pub orientation: Orientation,
}

impl RawMetadata {
    pub fn effective_crop(&self) -> Rect {
        self.crop_area
            .filter(|rect| rect.fits_within(self.width, self.height) && !rect.is_empty())
            .or_else(|| {
                self.active_area
                    .filter(|rect| rect.fits_within(self.width, self.height) && !rect.is_empty())
            })
            .unwrap_or_else(|| Rect::full(self.width, self.height))
    }

    pub fn display_dimensions(&self) -> (u32, u32) {
        let crop = self.effective_crop();
        if self.orientation.swaps_dimensions() {
            (crop.height, crop.width)
        } else {
            (crop.width, crop.height)
        }
    }

    pub fn validate(&self) -> Result<(), FrameError> {
        if self.width == 0 || self.height == 0 || self.components_per_pixel == 0 {
            return Err(FrameError::EmptyFrame);
        }
        if self.bits_per_sample == 0 || self.bits_per_sample > 16 {
            return Err(FrameError::UnsupportedBitDepth(self.bits_per_sample));
        }
        self.black_level.validate()?;
        if let Some(cfa) = &self.cfa {
            cfa.validate()?;
        }
        if self.photometric == Photometric::Cfa && (self.components_per_pixel != 1 || self.cfa.is_none()) {
            return Err(FrameError::InconsistentCfaComponents);
        }
        for (name, rect) in [("active area", self.active_area), ("crop area", self.crop_area)] {
            if rect.is_some_and(|value| !value.fits_within(self.width, self.height)) {
                return Err(FrameError::InvalidRectangle(name));
            }
        }
        if self.white_balance.iter().any(|value| !value.is_finite())
            || self
                .xyz_to_camera
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
        {
            return Err(FrameError::NonFiniteMetadata("color calibration"));
        }
        if self.white_level.0.is_empty()
            || self
                .white_level
                .0
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(FrameError::NonFiniteMetadata("white level"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DecodedMosaic {
    pub metadata: RawMetadata,
    /// Shared ownership keeps handoff to the UI/GPU/cache O(1). Keeping the
    /// decoder's `Vec` allocation intact avoids a full-frame copy that an
    /// `Arc<[u16]>` conversion would otherwise require.
    pub pixels: Arc<Vec<u16>>,
}

impl DecodedMosaic {
    pub fn new(metadata: RawMetadata, pixels: Arc<Vec<u16>>) -> Result<Self, FrameError> {
        metadata.validate()?;
        let expected = usize::try_from(metadata.width)
            .ok()
            .and_then(|width| {
                usize::try_from(metadata.height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(usize::from(metadata.components_per_pixel)))
            .ok_or(FrameError::DimensionOverflow)?;
        if pixels.len() != expected {
            return Err(FrameError::InvalidPixelCount {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self { metadata, pixels })
    }

    pub fn byte_len(&self) -> usize {
        self.pixels.len() * size_of::<u16>()
    }

    /// Produce a compact RGBA8 thumbnail using deterministic nearest sampling.
    /// This is intentionally CPU-only and allocation-bounded; callers can run
    /// it on the decode pool and upload the result as a small GPU texture.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn thumbnail_rgba8(&self, max_dimension: u32) -> Vec<u8> {
        let max_dimension = max_dimension.max(1);
        let source_long_edge = self.metadata.width.max(self.metadata.height);
        let (out_w, out_h) = if source_long_edge <= max_dimension {
            (self.metadata.width, self.metadata.height)
        } else {
            let scaled = |dimension: u32| {
                u32::try_from(
                    u64::from(dimension)
                        .saturating_mul(u64::from(max_dimension))
                        .div_ceil(u64::from(source_long_edge)),
                )
                .unwrap_or(max_dimension)
                .max(1)
            };
            (scaled(self.metadata.width), scaled(self.metadata.height))
        };
        let Some(output_bytes) = usize::try_from(out_w)
            .ok()
            .and_then(|width| {
                usize::try_from(out_h)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4))
        else {
            return Vec::new();
        };
        let mut out = vec![0_u8; output_bytes];
        let white = self
            .metadata
            .white_level
            .0
            .first()
            .copied()
            .unwrap_or(f32::from(u16::MAX))
            .max(1.0);
        let channels = usize::from(self.metadata.components_per_pixel);
        let source_width = usize::try_from(self.metadata.width).unwrap_or(usize::MAX);
        let output_width = usize::try_from(out_w).unwrap_or(1);
        for oy in 0..out_h {
            let y0 = u64::from(oy).saturating_mul(u64::from(self.metadata.height)) / u64::from(out_h);
            for ox in 0..out_w {
                let x0 = u64::from(ox).saturating_mul(u64::from(self.metadata.width)) / u64::from(out_w);
                let src = usize::try_from(y0)
                    .unwrap_or(usize::MAX)
                    .saturating_mul(source_width)
                    .saturating_add(usize::try_from(x0).unwrap_or(usize::MAX))
                    .saturating_mul(channels);
                let value = f32::from(self.pixels.get(src).copied().unwrap_or(0)) / white;
                let byte = (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                let dst = usize::try_from(oy)
                    .unwrap_or(usize::MAX)
                    .saturating_mul(output_width)
                    .saturating_add(usize::try_from(ox).unwrap_or(usize::MAX))
                    .saturating_mul(4);
                out[dst..dst + 4].copy_from_slice(&[byte, byte, byte, 255]);
            }
        }
        out
    }
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum FrameError {
    #[error("frame dimensions or allocation size overflow")]
    DimensionOverflow,
    #[error("frame dimensions must be non-zero")]
    EmptyFrame,
    #[error("unsupported RAW bit depth: {0}")]
    UnsupportedBitDepth(u8),
    #[error("CFA photometric data must have one component and a CFA pattern")]
    InconsistentCfaComponents,
    #[error("invalid CFA cell count: expected {expected}, got {actual}")]
    InvalidCfaLength { expected: usize, actual: usize },
    #[error("unsupported CFA layout {width}x{height}; the GPU fast path currently requires 2x2 RGB Bayer")]
    UnsupportedCfa { width: u8, height: u8 },
    #[error("invalid black-level grid: expected {expected}, got {actual}")]
    InvalidLevelGrid { expected: usize, actual: usize },
    #[error("non-finite {0} metadata")]
    NonFiniteMetadata(&'static str),
    #[error("{0} lies outside the decoded image")]
    InvalidRectangle(&'static str),
    #[error("invalid pixel count: expected {expected}, got {actual}")]
    InvalidPixelCount { expected: usize, actual: usize },
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        CfaColor, CfaPattern, DecodedMosaic, FrameError, LevelGrid, Orientation, Photometric, RawMetadata,
        Rect, WhiteLevel,
    };

    #[test]
    fn recognizes_all_four_bayer_phases() {
        for cells in [
            vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
            vec![CfaColor::Blue, CfaColor::Green, CfaColor::Green, CfaColor::Red],
            vec![CfaColor::Green, CfaColor::Red, CfaColor::Blue, CfaColor::Green],
            vec![CfaColor::Green, CfaColor::Blue, CfaColor::Red, CfaColor::Green],
        ] {
            assert!(
                CfaPattern {
                    width: 2,
                    height: 2,
                    cells,
                }
                .bayer_quad()
                .is_ok()
            );
        }
    }

    #[test]
    fn rejects_malformed_cfa_and_level_grid_before_indexing() {
        let empty = CfaPattern {
            width: 0,
            height: 2,
            cells: Vec::new(),
        };
        assert!(matches!(
            empty.validate(),
            Err(FrameError::InvalidCfaLength { .. })
        ));
        let wrong_length = CfaPattern {
            width: 2,
            height: 2,
            cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Blue],
        };
        assert!(matches!(
            wrong_length.validate(),
            Err(FrameError::InvalidCfaLength { .. })
        ));

        let non_finite = LevelGrid {
            width: 1,
            height: 1,
            components: 1,
            values: vec![f32::NAN],
        };
        assert!(matches!(
            non_finite.validate(),
            Err(FrameError::NonFiniteMetadata("black level"))
        ));
    }

    #[test]
    fn level_grid_repeats_in_sensor_coordinates() {
        let grid = LevelGrid {
            width: 2,
            height: 2,
            components: 1,
            values: vec![10.0, 20.0, 30.0, 40.0],
        };
        assert_eq!(grid.at(2, 3, 0), Some(20.0));
        assert_eq!(grid.at(3, 2, 0), Some(30.0));
    }

    #[test]
    fn only_quarter_turns_swap_dimensions() {
        assert!(Orientation::Rotate90.swaps_dimensions());
        assert!(Orientation::Transpose.swaps_dimensions());
        assert!(!Orientation::Rotate180.swaps_dimensions());
        assert!(!Orientation::HorizontalFlip.swaps_dimensions());
    }

    #[test]
    fn orientation_mapping_matches_exif_corner_convention() {
        let uv = [0.2, 0.8];
        let expected = [
            ([0.2, 0.8], Orientation::Normal),
            ([0.8, 0.8], Orientation::HorizontalFlip),
            ([0.8, 0.2], Orientation::Rotate180),
            ([0.2, 0.2], Orientation::VerticalFlip),
            ([0.8, 0.2], Orientation::Transpose),
            ([0.2, 0.2], Orientation::Rotate90),
            ([0.2, 0.8], Orientation::Transverse),
            ([0.8, 0.8], Orientation::Rotate270),
        ];
        for (want, orientation) in expected {
            let got = orientation.map_display_uv(uv);
            assert!((got[0] - want[0]).abs() < 1e-5);
            assert!((got[1] - want[1]).abs() < 1e-5);
        }
    }

    #[test]
    fn orientation_maps_stay_in_normalized_domain() {
        let grid =
            (0_u16..=16).flat_map(|y| (0_u16..=16).map(move |x| [f32::from(x) / 16.0, f32::from(y) / 16.0]));
        for orientation in [
            Orientation::Normal,
            Orientation::HorizontalFlip,
            Orientation::Rotate180,
            Orientation::VerticalFlip,
            Orientation::Transpose,
            Orientation::Rotate90,
            Orientation::Transverse,
            Orientation::Rotate270,
        ] {
            for uv in grid.clone() {
                let mapped = orientation.map_display_uv(uv);
                assert!(mapped.iter().all(|value| (0.0..=1.0).contains(value)));
            }
        }
    }

    fn valid_metadata() -> RawMetadata {
        RawMetadata {
            make: "test".into(),
            model: "synthetic".into(),
            width: 2,
            height: 2,
            components_per_pixel: 1,
            bits_per_sample: 14,
            photometric: Photometric::Cfa,
            cfa: Some(CfaPattern {
                width: 2,
                height: 2,
                cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
            }),
            black_level: LevelGrid {
                width: 1,
                height: 1,
                components: 1,
                values: vec![0.0],
            },
            white_level: WhiteLevel(vec![16_383.0]),
            white_balance: [1.0; 4],
            xyz_to_camera: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]],
            active_area: None,
            crop_area: None,
            orientation: Orientation::Normal,
        }
    }

    #[test]
    fn decoded_mosaic_rejects_pixel_count_and_invalid_metadata() {
        let metadata = valid_metadata();
        let error = DecodedMosaic::new(metadata.clone(), Arc::new(vec![1_u16, 2, 3])).unwrap_err();
        assert!(matches!(
            error,
            FrameError::InvalidPixelCount {
                expected: 4,
                actual: 3
            }
        ));

        let mut invalid_crop = metadata;
        invalid_crop.crop_area = Some(Rect::new(1, 1, 2, 2));
        let error = DecodedMosaic::new(invalid_crop, Arc::new(vec![1_u16, 2, 3, 4])).unwrap_err();
        assert!(matches!(error, FrameError::InvalidRectangle("crop area")));
    }

    #[test]
    fn decoded_mosaic_preserves_the_decoder_pixel_allocation() {
        let pixels = vec![1_u16, 2, 3, 4];
        let allocation = pixels.as_ptr();
        let mosaic = DecodedMosaic::new(valid_metadata(), Arc::new(pixels)).unwrap();
        assert_eq!(mosaic.pixels.as_ptr(), allocation);
    }

    #[test]
    fn decoded_mosaic_rejects_extreme_dimensions_without_allocating() {
        let mut metadata = valid_metadata();
        metadata.width = u32::MAX;
        metadata.height = 2;
        // The empty pixel slice is deliberate: dimensions are attacker input,
        // so validation must fail without attempting to materialize them.
        let result = DecodedMosaic::new(metadata, Arc::new(Vec::new()));
        assert!(matches!(
            result,
            Err(FrameError::InvalidPixelCount { .. } | FrameError::DimensionOverflow)
        ));
    }

    #[test]
    fn thumbnail_is_bounded_and_rgba() {
        let mut metadata = valid_metadata();
        metadata.width = 4;
        metadata.height = 2;
        let mosaic = DecodedMosaic::new(
            metadata,
            Arc::new(vec![0_u16, 100, 200, 400, 800, 1000, 2000, 4000]),
        )
        .unwrap();
        let thumb = mosaic.thumbnail_rgba8(2);
        assert_eq!(thumb.len(), 8);
        assert!(thumb.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn metadata_rejects_non_finite_calibration_and_white_level() {
        let mut metadata = valid_metadata();
        metadata.white_balance[2] = f32::INFINITY;
        assert!(matches!(
            metadata.validate(),
            Err(FrameError::NonFiniteMetadata("color calibration"))
        ));

        let mut metadata = valid_metadata();
        metadata.white_level = WhiteLevel(vec![f32::NAN]);
        assert!(matches!(
            metadata.validate(),
            Err(FrameError::NonFiniteMetadata("white level"))
        ));
    }
}
