//! Production adapter for the clean-room TIFF/DNG decoder.

use std::{sync::Arc, time::Instant};

use rrrah_core::{
    CfaColor, CfaPattern, DECODE_CROP_AS_METADATA, DECODE_FULL_SENSOR_RAW, DECODE_IMAGE_INDEX_IN_KEY,
    DECODE_INTEGER_U16, DECODE_SENSOR_COORDINATES, DecodedMosaic, DngColorMatrix, LevelGrid,
    MosaicRecipeManifest, Orientation, Photometric, RawMetadata, Rect, WhiteLevel, select_dng_xyz_to_camera,
};

use crate::{
    AdaptTimings, DecodeError, DecodeOutput, DecodeRequest, DecodeTimings, DngDecodeTimings, RawDecoder,
    bounded_io::read_managed,
    dng::{self, DngError, DngImage},
};

pub const NATIVE_DNG_BACKEND_ID: u32 = 3;

const NATIVE_DECODE_FLAGS: u32 = DECODE_FULL_SENSOR_RAW
    | DECODE_INTEGER_U16
    | DECODE_SENSOR_COORDINATES
    | DECODE_CROP_AS_METADATA
    | DECODE_IMAGE_INDEX_IN_KEY;

/// Semantic contract for the first native DNG decoder.
///
/// The dependency digest is the same resolved-workspace lock digest used by
/// the CR3 backend. The distinct backend ID prevents the two pixel contracts
/// from ever sharing cache entries.
pub const NATIVE_DNG_MOSAIC_CONTRACT_1: MosaicRecipeManifest = MosaicRecipeManifest::new(
    NATIVE_DNG_BACKEND_ID,
    3,
    1,
    1,
    NATIVE_DECODE_FLAGS,
    crate::WORKSPACE_LOCK_DIGEST,
);

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDngDecoder;

impl NativeDngDecoder {
    /// Decode an owned or borrowed source without opening a filesystem path.
    /// Source ownership remains alive through borrowed DNG metadata adaptation.
    pub(crate) fn decode_source<S: std::ops::Deref<Target = [u8]>>(
        &self,
        request: &DecodeRequest,
        data: S,
        source_open: std::time::Duration,
        total_started: Instant,
    ) -> Result<DecodeOutput, DecodeError> {
        request.check_cancelled()?;
        if request.image_index != 0 {
            return Err(DecodeError::UnsupportedImageIndex {
                index: request.image_index,
            });
        }
        let decoder_select_started = Instant::now();
        let image = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dng::parse(&data)))
            .map_err(|_| DecodeError::DecoderPanicked)?
            .map_err(|error| map_dng_error(&error))?;
        let decoder_select = decoder_select_started.elapsed();
        request.check_cancelled()?;

        let cancelled = || {
            request
                .cancellation
                .as_ref()
                .is_some_and(crate::GenerationToken::is_cancelled)
        };
        let output_bytes = u64::try_from(image.sample_count)
            .map_err(|_| DecodeError::DimensionOverflow)?
            .checked_mul(2)
            .ok_or(DecodeError::DimensionOverflow)?;
        let raw_image_started = Instant::now();
        let (pixels, pixel_timings): (rrrah_core::PixelBuffer<u16>, dng::DngPixelTimings) =
            if image.compression == dng::Compression::Vc5 {
                let pixels = image.decode_vc5(request)?;
                let elapsed = raw_image_started.elapsed();
                (
                    pixels.into(),
                    dng::DngPixelTimings {
                        pixel_unpack: elapsed,
                        linearization: Default::default(),
                        total: elapsed,
                    },
                )
            } else {
                let reservation = request
                    .memory_budget
                    .as_ref()
                    .map(|budget| budget.try_reserve(output_bytes))
                    .transpose()?;
                let decoded =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| image.decode_u16(&cancelled)))
                        .map_err(|_| DecodeError::DecoderPanicked)?
                        .map_err(|error| map_dng_error(&error))?;
                let pixels = match reservation {
                    Some(reservation) => reservation.try_adopt(decoded.pixels)?.into(),
                    None => Arc::new(decoded.pixels).into(),
                };
                (pixels, decoded.timings)
            };
        let raw_image = raw_image_started.elapsed();
        request.check_cancelled()?;
        let dng_timings = DngDecodeTimings {
            tiff_header: image.parse_timings.tiff_header,
            ifd_walk: image.parse_timings.ifd_walk,
            raw_ifd_select: image.parse_timings.raw_ifd_select,
            metadata: image.parse_timings.metadata,
            storage_plan: image.parse_timings.storage_plan,
            pixel_unpack: pixel_timings.pixel_unpack,
            linearization: pixel_timings.linearization,
        };
        let (mosaic, adapt) = adapt_dng(&image, pixels)?;
        let adapt_metadata = adapt.total;
        let raw_decode = decoder_select.saturating_add(raw_image);
        request.check_cancelled()?;

        Ok(DecodeOutput {
            mosaic,
            timings: DecodeTimings {
                source_open,
                decoder_select,
                raw_image,
                raw_decode,
                native: None,
                dng: Some(dng_timings),
                adapt,
                adapt_metadata,
                total: total_started.elapsed(),
            },
        })
    }
}

impl RawDecoder for NativeDngDecoder {
    fn mosaic_recipe(&self, _request: &DecodeRequest) -> Result<MosaicRecipeManifest, DecodeError> {
        Ok(NATIVE_DNG_MOSAIC_CONTRACT_1)
    }

    fn decode(&self, request: &DecodeRequest) -> Result<DecodeOutput, DecodeError> {
        let total_started = Instant::now();
        request.check_cancelled()?;
        if request.image_index != 0 {
            return Err(DecodeError::UnsupportedImageIndex {
                index: request.image_index,
            });
        }

        let source_started = Instant::now();
        let data = read_managed(request)?;
        let source_open = source_started.elapsed();
        request.check_cancelled()?;

        self.decode_source(request, data, source_open, total_started)
    }
}

fn map_dng_error(error: &DngError) -> DecodeError {
    if matches!(error, DngError::Cancelled { .. }) {
        DecodeError::Cancelled
    } else {
        DecodeError::NativeDng(error.to_string())
    }
}

fn adapt_dng(
    image: &DngImage<'_>,
    pixels: rrrah_core::PixelBuffer<u16>,
) -> Result<(DecodedMosaic, AdaptTimings), DecodeError> {
    let total_started = Instant::now();

    let layout_started = Instant::now();
    let mut cfa = CfaPattern {
        width: u8::try_from(image.metadata.cfa.columns)
            .map_err(|_| dng_adapt_error("CFA width exceeds 255"))?,
        height: u8::try_from(image.metadata.cfa.rows)
            .map_err(|_| dng_adapt_error("CFA height exceeds 255"))?,
        cells: image
            .metadata
            .cfa
            .cells
            .iter()
            .copied()
            .map(map_cfa_color)
            .collect(),
    };
    cfa.validate()?;
    if cfa.bayer_quad().is_err()
        && (cfa.width != 6
            || cfa.height != 6
            || cfa.cells.iter().any(|c| *c as u8 > 2)
            || [CfaColor::Red, CfaColor::Green, CfaColor::Blue]
                .into_iter()
                .zip([8, 20, 8])
                .any(|(c, n)| cfa.cells.iter().filter(|p| **p == c).count() != n))
    {
        return Err(dng_adapt_error(
            "quality display requires RGB Bayer or 6x6 X-Trans",
        ));
    }
    // DNG CFA and black grids are anchored at ActiveArea's top-left. Runtime
    // metadata is anchored at the immutable full sensor's (0,0).
    cfa.cells = sensor_grid(
        &cfa.cells,
        usize::from(cfa.width),
        usize::from(cfa.height),
        image.metadata.active_area,
    );
    let rgb_plane_indices = rgb_plane_indices(&image.metadata.cfa.plane_colors)?;
    let layout_cfa = layout_started.elapsed();

    let levels_started = Instant::now();
    let mut black_level = LevelGrid {
        width: u8::try_from(image.metadata.black_level.repeat_columns)
            .map_err(|_| dng_adapt_error("black-level grid width exceeds 255"))?,
        height: u8::try_from(image.metadata.black_level.repeat_rows)
            .map_err(|_| dng_adapt_error("black-level grid height exceeds 255"))?,
        components: 1,
        values: image
            .metadata
            .black_level
            .values
            .iter()
            .copied()
            .map(finite_f32)
            .collect::<Result<Vec<_>, _>>()?,
    };
    black_level.values = sensor_grid(
        &black_level.values,
        usize::from(black_level.width),
        usize::from(black_level.height),
        image.metadata.active_area,
    );
    let white_level = WhiteLevel(
        image
            .metadata
            .white_level
            .iter()
            .copied()
            .map(f32::from)
            .collect(),
    );
    let levels = levels_started.elapsed();

    let color_started = Instant::now();
    let white_balance = white_balance(image, rgb_plane_indices)?;
    let xyz_to_camera = xyz_to_camera(image, rgb_plane_indices)?;
    let color = color_started.elapsed();

    let geometry_started = Instant::now();
    let active = image.metadata.active_area;
    let active_area = Some(Rect::new(
        active.left,
        active.top,
        active.width(),
        active.height(),
    ));
    let crop = image.metadata.crop;
    let crop_area = Some(Rect::new(
        exact_u32(crop.origin_x, "DefaultCropOrigin x")?,
        exact_u32(crop.origin_y, "DefaultCropOrigin y")?,
        exact_u32(crop.width, "DefaultCropSize width")?,
        exact_u32(crop.height, "DefaultCropSize height")?,
    ));
    let orientation = map_orientation(image.metadata.orientation);
    let geometry = geometry_started.elapsed();

    let finalize_started = Instant::now();
    let metadata = RawMetadata {
        make: image.metadata.make.clone(),
        model: image.metadata.model.clone(),
        width: image.width,
        height: image.height,
        components_per_pixel: 1,
        bits_per_sample: image.output_bits_per_sample,
        photometric: Photometric::Cfa,
        cfa: Some(cfa),
        black_level,
        white_level,
        white_balance,
        xyz_to_camera,
        active_area,
        crop_area,
        orientation,
    };
    let mosaic = DecodedMosaic::new(metadata, pixels)?;
    let finalize = finalize_started.elapsed();

    Ok((
        mosaic,
        AdaptTimings {
            layout_cfa,
            levels,
            color,
            geometry,
            finalize,
            total: total_started.elapsed(),
        },
    ))
}

fn sensor_grid<T: Copy>(values: &[T], w: usize, h: usize, active: dng::Rect) -> Vec<T> {
    (0..w * h)
        .map(|i| {
            let x = (i % w + w - active.left as usize % w) % w;
            let y = (i / w + h - active.top as usize % h) % h;
            values[y * w + x]
        })
        .collect()
}

fn rgb_plane_indices(colors: &[dng::CfaColor]) -> Result<[usize; 3], DecodeError> {
    let mut indices = [None; 3];
    for (index, color) in colors.iter().copied().enumerate() {
        let slot = match color {
            dng::CfaColor::Red => 0,
            dng::CfaColor::Green => 1,
            dng::CfaColor::Blue => 2,
            _ => {
                return Err(dng_adapt_error(
                    "the display pipeline currently supports only RGB CFA planes",
                ));
            }
        };
        if indices[slot].replace(index).is_some() {
            return Err(dng_adapt_error("DNG CFA plane colors contain duplicates"));
        }
    }
    match indices {
        [Some(red), Some(green), Some(blue)] if colors.len() == 3 => Ok([red, green, blue]),
        _ => Err(dng_adapt_error(
            "DNG CFA plane colors must contain exactly red, green, and blue",
        )),
    }
}

fn white_balance(image: &DngImage<'_>, indices: [usize; 3]) -> Result<[f32; 4], DecodeError> {
    let Some(neutral) = image.metadata.as_shot_neutral.as_deref() else {
        return Err(dng_adapt_error(
            "missing AsShotNeutral; refusing neutral white-balance placeholder",
        ));
    };
    let mut gains = [0.0_f32; 3];
    for (destination, source) in indices.into_iter().enumerate() {
        gains[destination] = finite_f32(1.0 / neutral[source])?;
    }
    let green = gains[1];
    if green <= 0.0 {
        return Err(dng_adapt_error(
            "AsShotNeutral produced a non-positive green gain",
        ));
    }
    for gain in &mut gains {
        *gain /= green;
    }
    Ok([gains[0], gains[1], gains[2], gains[1]])
}

fn xyz_to_camera(image: &DngImage<'_>, indices: [usize; 3]) -> Result<[[f32; 3]; 4], DecodeError> {
    let metadata = &image.metadata;
    let Some(matrix) = select_xyz_to_camera_d65(
        metadata.color_matrix_1.as_deref(),
        metadata.calibration_illuminant_1,
        metadata.color_matrix_2.as_deref(),
        metadata.calibration_illuminant_2,
    ) else {
        return crate::camtiff::color::profile(&metadata.make, &metadata.model)
            .ok_or_else(|| dng_adapt_error("no DNG color matrix or calibrated camera profile"));
    };
    let mut result = [[0.0_f32; 3]; 4];
    for (destination, source) in indices.into_iter().enumerate() {
        for column in 0..3 {
            result[destination][column] = finite_f32(matrix[source][column])?;
        }
    }
    if rrrah_core::camera_to_linear_srgb(result).is_none() {
        return Err(dng_adapt_error("invalid DNG camera color matrix"));
    }
    Ok(result)
}

/// Picks the D65-referenced `XYZ -> camera` matrix from the DNG calibration
/// pair. A `ColorMatrix2`/`CalibrationIlluminant2 = D65` pair is used
/// verbatim; a matrix calibrated for another known illuminant is
/// Bradford-adapted to D65 in f64; without illuminant information the legacy
/// verbatim `ColorMatrix1` behavior is kept.
fn select_xyz_to_camera_d65(
    color_matrix_1: Option<&[f64]>,
    calibration_illuminant_1: Option<u16>,
    color_matrix_2: Option<&[f64]>,
    calibration_illuminant_2: Option<u16>,
) -> Option<[[f64; 3]; 3]> {
    let candidate = |matrix: Option<&[f64]>, illuminant: Option<u16>| {
        matrix.filter(|flat| flat.len() == 9).map(|flat| DngColorMatrix {
            xyz_to_camera: [
                [flat[0], flat[1], flat[2]],
                [flat[3], flat[4], flat[5]],
                [flat[6], flat[7], flat[8]],
            ],
            illuminant,
        })
    };
    select_dng_xyz_to_camera(
        candidate(color_matrix_1, calibration_illuminant_1),
        candidate(color_matrix_2, calibration_illuminant_2),
    )
}

const fn map_cfa_color(color: dng::CfaColor) -> CfaColor {
    match color {
        dng::CfaColor::Red => CfaColor::Red,
        dng::CfaColor::Green => CfaColor::Green,
        dng::CfaColor::Blue => CfaColor::Blue,
        dng::CfaColor::Cyan => CfaColor::Cyan,
        dng::CfaColor::Magenta => CfaColor::Magenta,
        dng::CfaColor::Yellow => CfaColor::Yellow,
        dng::CfaColor::White => CfaColor::White,
    }
}

const fn map_orientation(orientation: dng::Orientation) -> Orientation {
    match orientation {
        dng::Orientation::Normal => Orientation::Normal,
        dng::Orientation::HorizontalFlip => Orientation::HorizontalFlip,
        dng::Orientation::Rotate180 => Orientation::Rotate180,
        dng::Orientation::VerticalFlip => Orientation::VerticalFlip,
        dng::Orientation::Transpose => Orientation::Transpose,
        dng::Orientation::Rotate90 => Orientation::Rotate90,
        dng::Orientation::Transverse => Orientation::Transverse,
        dng::Orientation::Rotate270 => Orientation::Rotate270,
    }
}

fn exact_u32(value: f64, field: &'static str) -> Result<u32, DecodeError> {
    if !value.is_finite() || value < 0.0 || value > f64::from(u32::MAX) || value.fract() != 0.0 {
        return Err(dng_adapt_error(format!(
            "{field}={value} cannot be represented by the integer display geometry"
        )));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(value as u32)
}

fn finite_f32(value: f64) -> Result<f32, DecodeError> {
    #[allow(clippy::cast_possible_truncation)]
    let converted = value as f32;
    if converted.is_finite() {
        Ok(converted)
    } else {
        Err(dng_adapt_error("DNG metadata is outside finite f32 range"))
    }
}

fn dng_adapt_error(message: impl Into<String>) -> DecodeError {
    DecodeError::NativeDng(message.into())
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };

    use super::*;
    use crate::GenerationToken;

    fn color_fixture(with_wb: bool, with_matrix: bool) -> Vec<u8> {
        let mut bytes = vec![0; 520];
        bytes[..8].copy_from_slice(b"II*\0\x08\0\0\0");
        let mut entries = vec![
            (256_u16, 4_u16, 1_u32, 2_u32),
            (257, 4, 1, 2),
            (258, 3, 1, 16),
            (259, 3, 1, 1),
            (262, 3, 1, 32803),
            (273, 4, 1, 512),
            (274, 3, 1, 1),
            (277, 3, 1, 1),
            (278, 4, 1, 2),
            (279, 4, 1, 8),
            (33421, 3, 2, 0x0002_0002),
            (33422, 1, 4, 0x0201_0100),
            (50706, 1, 4, 0x0000_0401),
            (50708, 2, 5, 300),
        ];
        bytes[300..305].copy_from_slice(b"TEST\0");
        if with_matrix {
            entries.push((50721, 10, 9, 320));
            for (i, value) in [6000_i32, -1000, -1000, -5000, 15000, 1000, -1000, 2000, 6000]
                .iter()
                .enumerate()
            {
                bytes[320 + i * 8..324 + i * 8].copy_from_slice(&value.to_le_bytes());
                bytes[324 + i * 8..328 + i * 8].copy_from_slice(&10000_i32.to_le_bytes());
            }
        }
        if with_wb {
            entries.push((50728, 5, 3, 400));
            for (i, (n, d)) in [(1_u32, 2_u32), (1, 1), (1, 4)].iter().enumerate() {
                bytes[400 + i * 8..404 + i * 8].copy_from_slice(&n.to_le_bytes());
                bytes[404 + i * 8..408 + i * 8].copy_from_slice(&d.to_le_bytes());
            }
        }
        entries.sort_unstable_by_key(|e| e.0);
        bytes[8..10].copy_from_slice(&u16::try_from(entries.len()).unwrap().to_le_bytes());
        for (i, (tag, kind, count, value)) in entries.iter().enumerate() {
            let at = 10 + i * 12;
            bytes[at..at + 2].copy_from_slice(&tag.to_le_bytes());
            bytes[at + 2..at + 4].copy_from_slice(&kind.to_le_bytes());
            bytes[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
            bytes[at + 8..at + 12].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
    #[test]
    fn dng_managed_source_and_output_share_budget_and_retain_owners() {
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
                "rrrah-dng-budget-{}-{}.dng",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        let mut bytes = color_fixture(true, true);
        bytes[512..520].copy_from_slice(&[1, 0, 2, 0, 3, 0, 4, 0]);
        std::fs::write(&fixture.0, &bytes).unwrap();
        let mut request = DecodeRequest::new(&fixture.0);
        let reference = NativeDngDecoder.decode(&request).unwrap();
        let budget = rrrah_core::MemoryBudget::new(bytes.len() as u64 + 8);
        request.memory_budget = Some(budget.clone());
        let decoded = NativeDngDecoder.decode(&request).unwrap();
        assert_eq!(decoded.mosaic.pixels, reference.mosaic.pixels);
        assert_eq!(decoded.mosaic.metadata, reference.mosaic.metadata);
        assert!(decoded.mosaic.pixels.is_managed());
        assert_eq!(budget.peak(), bytes.len() as u64 + 8);
        assert_eq!(budget.used(), 8);
        let owner = decoded.mosaic.clone();
        drop(decoded);
        assert!(matches!(
            NativeDngDecoder.decode(&request),
            Err(DecodeError::Memory(_))
        ));
        assert_eq!(budget.used(), 8);
        drop(owner);
        assert_eq!(budget.used(), 0);
        request.memory_budget = Some(rrrah_core::MemoryBudget::new(bytes.len() as u64));
        assert!(matches!(
            NativeDngDecoder.decode(&request),
            Err(DecodeError::Memory(_))
        ));
        assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
    }

    #[test]
    fn dng_color_metadata_is_required_and_measured_wb_is_preserved() {
        for (wb, matrix) in [(true, true), (false, true), (true, false)] {
            let bytes = color_fixture(wb, matrix);
            let image = dng::parse(&bytes).unwrap();
            assert_eq!(white_balance(&image, [0, 1, 2]).is_ok(), wb);
            assert_eq!(xyz_to_camera(&image, [0, 1, 2]).is_ok(), matrix);
            if wb {
                assert_eq!(
                    white_balance(&image, [0, 1, 2]).unwrap().map(f32::to_bits),
                    [2.0_f32, 1.0, 4.0, 1.0].map(f32::to_bits)
                );
            }
        }
    }

    #[test]
    fn dng_cfa_and_black_grid_are_anchored_to_full_sensor() {
        let active = dng::Rect {
            top: 1,
            left: 1,
            bottom: 2,
            right: 2,
        };
        assert_eq!(sensor_grid(&[0, 1, 2, 3], 2, 2, active), vec![3, 2, 1, 0]);
    }

    #[test]
    fn recipe_is_distinct_and_complete() {
        assert_eq!(
            NativeDngDecoder
                .mosaic_recipe(&DecodeRequest::new("fixture.DNG"))
                .unwrap(),
            NATIVE_DNG_MOSAIC_CONTRACT_1
        );
        assert_eq!(
            NATIVE_DNG_MOSAIC_CONTRACT_1.decoder_backend_id(),
            NATIVE_DNG_BACKEND_ID
        );
        assert_ne!(
            NATIVE_DNG_MOSAIC_CONTRACT_1,
            crate::NATIVE_EOS_R8_MOSAIC_CONTRACT_1
        );
        assert_eq!(
            &NATIVE_DNG_MOSAIC_CONTRACT_1.canonical_bytes()[28..60],
            &crate::NATIVE_EOS_R8_MOSAIC_CONTRACT_1.canonical_bytes()[28..60]
        );
    }

    #[test]
    fn stale_request_is_rejected_before_io() {
        let generation = Arc::new(AtomicU64::new(2));
        let mut request = DecodeRequest::new("does-not-exist.DNG");
        request.cancellation = Some(GenerationToken::new(Arc::clone(&generation), 1));
        assert!(matches!(
            NativeDngDecoder.decode(&request),
            Err(DecodeError::Cancelled)
        ));
        generation.store(3, Ordering::Release);
    }

    #[test]
    fn nonzero_image_index_is_rejected_before_io() {
        let mut request = DecodeRequest::new("does-not-exist.DNG");
        request.image_index = 1;
        assert!(matches!(
            NativeDngDecoder.decode(&request),
            Err(DecodeError::UnsupportedImageIndex { index: 1 })
        ));
    }

    #[test]
    fn exact_geometry_rejects_fractional_values() {
        assert_eq!(exact_u32(42.0, "test").unwrap(), 42);
        assert!(exact_u32(0.5, "test").is_err());
        assert!(exact_u32(f64::NAN, "test").is_err());
    }

    /// Flat `ColorMatrix` layout used by the selection tests below.
    fn flat(matrix: [[f64; 3]; 3]) -> Vec<f64> {
        matrix.into_iter().flatten().collect()
    }

    #[test]
    // Verbatim bit-exact reuse of the D65 matrix is exactly what is asserted.
    #[allow(clippy::float_cmp)]
    fn d65_color_matrix_2_wins_verbatim_over_illuminant_a_matrix_1() {
        let cm_a = flat([[0.6, -0.1, -0.1], [-0.8, 1.6, 0.2], [-0.2, 0.4, 0.6]]);
        let cm_d65 = flat([[1.0, -0.2, -0.1], [-0.5, 1.4, 0.1], [-0.1, 0.1, 0.8]]);
        let selected = select_xyz_to_camera_d65(Some(&cm_a), Some(17), Some(&cm_d65), Some(21)).unwrap();
        for (selected, verbatim) in selected.into_iter().flatten().zip(cm_d65) {
            assert_eq!(selected, verbatim);
        }
    }

    #[test]
    fn lone_illuminant_a_matrix_is_bradford_adapted() {
        let cm_a = flat([[0.6, -0.1, -0.1], [-0.8, 1.6, 0.2], [-0.2, 0.4, 0.6]]);
        let selected = select_xyz_to_camera_d65(Some(&cm_a), Some(17), None, None).unwrap();
        // Adapted output must differ from the raw matrix (a real correction)
        // and must match the shared core selection path exactly.
        let shifted = selected
            .into_iter()
            .flatten()
            .zip(cm_a.iter().copied())
            .any(|(adapted, raw)| (adapted - raw).abs() > 1.0e-3);
        assert!(shifted, "adaptation should visibly change the matrix");
        let core = select_dng_xyz_to_camera(
            Some(DngColorMatrix {
                xyz_to_camera: [[0.6, -0.1, -0.1], [-0.8, 1.6, 0.2], [-0.2, 0.4, 0.6]],
                illuminant: Some(17),
            }),
            None,
        )
        .unwrap();
        assert_eq!(selected, core);
    }

    #[test]
    // Verbatim bit-exact reuse of the untagged matrix is exactly what is asserted.
    #[allow(clippy::float_cmp)]
    fn missing_or_untagged_matrices_keep_legacy_behavior() {
        // No matrices at all: identity fallback upstream (`None` here).
        assert_eq!(select_xyz_to_camera_d65(None, None, None, None), None);
        // ColorMatrix1 without illuminant tags: verbatim, as before.
        let cm = flat([[1.0, -0.2, -0.1], [-0.5, 1.4, 0.1], [-0.1, 0.1, 0.8]]);
        let selected = select_xyz_to_camera_d65(Some(&cm), None, None, None).unwrap();
        for (selected, verbatim) in selected.into_iter().flatten().zip(&cm) {
            assert_eq!(selected, *verbatim);
        }
        // A malformed row count is ignored rather than misparsed.
        let short = vec![1.0_f64; 6];
        assert_eq!(select_xyz_to_camera_d65(Some(&short), Some(17), None, None), None);
    }
}

#[cfg(test)]
mod gpr_admission_tests {
    use super::*;

    #[test]
    #[ignore = "requires pinned HERO9 source and independent full Bayer oracle"]
    fn hero9_native_dng_matches_sensor_oracle_and_releases_budget() {
        let source = std::env::var("RRRAH_GPR_SOURCE").expect("RRRAH_GPR_SOURCE");
        let oracle = std::fs::read(std::env::var("RRRAH_GPR_SENSOR_ORACLE").expect("RRRAH_GPR_SENSOR_ORACLE")).unwrap();
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = DecodeRequest::new(&source);
        request.memory_budget = Some(budget.clone());
        let decoded = crate::NativeRawDecoder.decode(&request).unwrap();
        assert_eq!(decoded.mosaic.metadata.bits_per_sample, 14);
        assert_eq!(decoded.mosaic.pixels.len() * 2, oracle.len());
        for (native, reference) in decoded.mosaic.pixels.iter().zip(oracle.chunks_exact(2)) {
            assert_eq!(*native, u16::from_le_bytes([reference[0], reference[1]]));
        }
        assert_eq!(budget.used(), 46_503_936);
        drop(decoded);
        assert_eq!(budget.used(), 0);
        let small = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
        request.memory_budget = Some(small.clone());
        assert!(crate::NativeRawDecoder.decode(&request).is_err());
        assert_eq!(small.used(), 0);
    }

    #[test]
    #[ignore = "requires pinned HERO9 GPR source"]
    fn damaged_gpr_refuses_without_retaining_managed_pixels() {
        let path = std::env::var("RRRAH_GPR_SOURCE").expect("RRRAH_GPR_SOURCE");
        let source = std::fs::read(&path).unwrap();
        assert_eq!(source.len(), 6_763_778);
        let ifd = u32::from_le_bytes(source[4..8].try_into().unwrap()) as usize;
        let entries = u16::from_le_bytes(source[ifd..ifd+2].try_into().unwrap()) as usize;
        let entry = |tag: u16| (0..entries).map(|i| ifd+2+i*12)
            .find(|o| u16::from_le_bytes(source[*o..*o+2].try_into().unwrap())==tag).unwrap();
        let tile_entry = entry(324);
        let tile = u32::from_le_bytes(source[tile_entry+8..tile_entry+12].try_into().unwrap()) as usize;
        let width_record = crate::vc5::Vc5Records::new(&source[tile..]).unwrap()
            .map(Result::unwrap).find(|r| r.tag==20).unwrap().offset;
        let mut wrong_width = source.clone();
        wrong_width[tile+width_record+2..tile+width_record+4].copy_from_slice(&5566u16.to_be_bytes());
        let mut invalid_white = source.clone();
        let white = entry(50717);
        assert_eq!(u16::from_le_bytes(source[white+2..white+4].try_into().unwrap()),3);
        invalid_white[white+8..white+10].copy_from_slice(&8191u16.to_le_bytes());
        let mut bad_signature = source.clone();
        bad_signature[tile..tile+4].copy_from_slice(b"BAD!");
        let mut truncated = source.clone();
        truncated.pop();
        for (data, expected) in [
            (wrong_width, "dimensions disagree"),
            (invalid_white, "Compression 9"),
            (bad_signature, "VC-5"),
            (truncated, ""),
        ] {
            let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
            let mut request = DecodeRequest::new(&path);
            request.memory_budget = Some(budget.clone());
            let error = NativeDngDecoder.decode_source(&request, data, Default::default(), Instant::now()).unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
            assert_eq!(budget.used(), 0);
            assert_eq!(budget.peak(), 0, "invalid framing must precede sensor allocation");
        }
    }
}
