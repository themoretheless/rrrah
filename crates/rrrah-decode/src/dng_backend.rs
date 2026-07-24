//! Production adapter for the clean-room TIFF/DNG decoder.

use std::{fs::File, io::Read, sync::Arc, time::Instant};

use rrrah_core::{
    CfaColor, CfaPattern, DECODE_CROP_AS_METADATA, DECODE_FULL_SENSOR_RAW, DECODE_IMAGE_INDEX_IN_KEY,
    DECODE_INTEGER_U16, DECODE_SENSOR_COORDINATES, DecodedMosaic, DngColorMatrix, LevelGrid,
    MosaicRecipeManifest, Orientation, Photometric, RawMetadata, Rect, WhiteLevel,
    select_dng_xyz_to_camera,
};

use crate::{
    AdaptTimings, DecodeError, DecodeOutput, DecodeRequest, DecodeTimings, DngDecodeTimings, RawDecoder,
    dng::{self, DngError, DngImage},
};

pub const NATIVE_DNG_BACKEND_ID: u32 = 3;

const NATIVE_DECODE_FLAGS: u32 = DECODE_FULL_SENSOR_RAW
    | DECODE_INTEGER_U16
    | DECODE_SENSOR_COORDINATES
    | DECODE_CROP_AS_METADATA
    | DECODE_IMAGE_INDEX_IN_KEY;
const MAX_INPUT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Semantic contract for the first native DNG decoder.
///
/// The dependency digest is the same resolved-workspace lock digest used by
/// the CR3 backend. The distinct backend ID prevents the two pixel contracts
/// from ever sharing cache entries.
pub const NATIVE_DNG_MOSAIC_CONTRACT_1: MosaicRecipeManifest = MosaicRecipeManifest::new(
    NATIVE_DNG_BACKEND_ID,
    1,
    1,
    1,
    NATIVE_DECODE_FLAGS,
    [
        0xab, 0x83, 0x34, 0x86, 0x28, 0xb8, 0xa9, 0x5b, 0x42, 0x58, 0x3e, 0x96, 0xbe, 0xd2, 0x2a, 0xa5, 0xba,
        0x94, 0x4c, 0xda, 0xb5, 0x4f, 0xb7, 0x45, 0xbe, 0xb0, 0xa6, 0xa0, 0x98, 0xea, 0x37, 0x70,
    ],
);

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDngDecoder;

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
        let data = read_bounded(request)?;
        let source_open = source_started.elapsed();
        request.check_cancelled()?;

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
        let raw_image_started = Instant::now();
        let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| image.decode_u16(&cancelled)))
            .map_err(|_| DecodeError::DecoderPanicked)?
            .map_err(|error| map_dng_error(&error))?;
        let raw_image = raw_image_started.elapsed();
        request.check_cancelled()?;

        let dng_timings = DngDecodeTimings {
            tiff_header: image.parse_timings.tiff_header,
            ifd_walk: image.parse_timings.ifd_walk,
            raw_ifd_select: image.parse_timings.raw_ifd_select,
            metadata: image.parse_timings.metadata,
            storage_plan: image.parse_timings.storage_plan,
            pixel_unpack: decoded.timings.pixel_unpack,
            linearization: decoded.timings.linearization,
        };
        let (mosaic, adapt) = adapt_dng(&image, decoded.pixels)?;
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

fn read_bounded(request: &DecodeRequest) -> Result<Vec<u8>, DecodeError> {
    let mut file = File::open(&request.path).map_err(|source| DecodeError::Io {
        path: request.path.clone(),
        source,
    })?;
    let declared = file
        .metadata()
        .map_err(|source| DecodeError::Io {
            path: request.path.clone(),
            source,
        })?
        .len();
    if declared > MAX_INPUT_BYTES {
        return Err(DecodeError::InputTooLarge {
            path: request.path.clone(),
            actual: declared,
            limit: MAX_INPUT_BYTES,
        });
    }
    let capacity = usize::try_from(declared).map_err(|_| DecodeError::InputTooLarge {
        path: request.path.clone(),
        actual: declared,
        limit: MAX_INPUT_BYTES,
    })?;
    let mut data = Vec::new();
    data.try_reserve_exact(capacity)
        .map_err(|_| DecodeError::InputAllocation { bytes: capacity })?;
    file.by_ref()
        .take(MAX_INPUT_BYTES.saturating_add(1))
        .read_to_end(&mut data)
        .map_err(|source| DecodeError::Io {
            path: request.path.clone(),
            source,
        })?;
    let actual = u64::try_from(data.len()).unwrap_or(u64::MAX);
    if actual > MAX_INPUT_BYTES {
        return Err(DecodeError::InputTooLarge {
            path: request.path.clone(),
            actual,
            limit: MAX_INPUT_BYTES,
        });
    }
    Ok(data)
}

fn map_dng_error(error: &DngError) -> DecodeError {
    if matches!(error, DngError::Cancelled { .. }) {
        DecodeError::Cancelled
    } else {
        DecodeError::NativeDng(error.to_string())
    }
}

fn adapt_dng(image: &DngImage<'_>, pixels: Vec<u16>) -> Result<(DecodedMosaic, AdaptTimings), DecodeError> {
    let total_started = Instant::now();

    let layout_started = Instant::now();
    let cfa = CfaPattern {
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
    cfa.bayer_quad()
        .map_err(|_| dng_adapt_error("the display pipeline currently requires a 2x2 RGB Bayer CFA"))?;
    let rgb_plane_indices = rgb_plane_indices(&image.metadata.cfa.plane_colors)?;
    let layout_cfa = layout_started.elapsed();

    let levels_started = Instant::now();
    let black_level = LevelGrid {
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
    let mosaic = DecodedMosaic::new(metadata, Arc::new(pixels))?;
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
        return Ok([1.0; 4]);
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
        return Ok([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]]);
    };
    let mut result = [[0.0_f32; 3]; 4];
    for (destination, source) in indices.into_iter().enumerate() {
        for column in 0..3 {
            result[destination][column] = finite_f32(matrix[source][column])?;
        }
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
        matrix
            .filter(|flat| flat.len() == 9)
            .map(|flat| DngColorMatrix {
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
        let selected =
            select_xyz_to_camera_d65(Some(&cm_a), Some(17), Some(&cm_d65), Some(21)).unwrap();
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
