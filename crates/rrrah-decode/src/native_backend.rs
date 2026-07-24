//! Default clean-room Canon EOS R8 CR3 backend.

use std::{fs::File, io::Read, sync::Arc, time::Instant};

use rrrah_core::{
    CfaPattern, DECODE_CROP_AS_METADATA, DECODE_FULL_SENSOR_RAW, DECODE_IMAGE_INDEX_IN_KEY,
    DECODE_INTEGER_U16, DECODE_SENSOR_COORDINATES, DecodedMosaic, LevelGrid, MosaicRecipeManifest,
    Orientation, Photometric, RawMetadata, Rect, WhiteLevel,
};

use crate::{
    AdaptTimings, DecodeError, DecodeOutput, DecodeRequest, DecodeTimings, NativeDecodeTimings, RawDecoder,
    cr3::{
        assemble::{
            ParallelDecodeError, StreamingDecodeError, decode_four_planes, decode_four_planes_streaming,
            interleave_rggb,
        },
        crx::Iad1,
        ctmd::EosR8AsShotWhiteBalance,
        lossless::{self, LosslessError},
        metadata::EosR8Profile,
        native::{NativeFrame, parse},
    },
};

pub const NATIVE_CR3_BACKEND_ID: u32 = 2;

const NATIVE_DECODE_FLAGS: u32 = DECODE_FULL_SENSOR_RAW
    | DECODE_INTEGER_U16
    | DECODE_SENSOR_COORDINATES
    | DECODE_CROP_AS_METADATA
    | DECODE_IMAGE_INDEX_IN_KEY;
const MAX_INPUT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DEFAULT_PLANE_WORKERS: usize = 4;
const PLANE_WORKERS_ENV: &str = "RRRAH_CR3_PLANE_WORKERS";
const PLANE_WORKER_OPTIONS: [usize; 3] = [1, 2, 4];
/// CRX stores the four Bayer parities as independent entropy streams, so the
/// streaming scheduler always runs one worker per plane.
const CR3_PLANE_COUNT: usize = 4;

/// Resolves the requested CR3 plane-worker count from the environment.
///
/// `RRRAH_CR3_PLANE_WORKERS` accepts 1, 2, or 4 and exists for scheduler
/// sweeps; any missing or invalid value falls back to the default of 4,
/// matching the previously hardcoded request. Values below the plane count
/// force the bounded batch scheduler so a `1|2|4` sweep can compare the
/// batch branch against the streaming branch.
fn requested_plane_workers() -> usize {
    parse_plane_workers(std::env::var(PLANE_WORKERS_ENV).ok().as_deref())
}

fn parse_plane_workers(value: Option<&str>) -> usize {
    value
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|workers| PLANE_WORKER_OPTIONS.contains(workers))
        .unwrap_or(DEFAULT_PLANE_WORKERS)
}

/// Streaming assembly keeps one worker per plane plus the coordinating
/// caller, so it is eligible only when the request covers all four planes
/// and the machine has a spare hardware thread for the coordinator.
fn should_stream_planes(requested_workers: usize, available_workers: usize) -> bool {
    requested_workers >= CR3_PLANE_COUNT && available_workers > requested_workers
}

/// Worker count the decoder reports for the current environment, kept in
/// sync with both schedulers so regression contracts can assert it under
/// any `RRRAH_CR3_PLANE_WORKERS` setting.
#[cfg(test)]
pub(crate) fn planned_plane_worker_count() -> usize {
    let requested = requested_plane_workers();
    let available = std::thread::available_parallelism().map_or(1, usize::from);
    if should_stream_planes(requested, available) {
        CR3_PLANE_COUNT
    } else {
        requested.clamp(1, available.clamp(1, CR3_PLANE_COUNT))
    }
}

/// The small, fully owned subset of a parsed frame needed after entropy decode.
///
/// Keeping this separate from `NativeFrame` lets all borrowed parser evidence
/// and the compressed input be released immediately after streamed entropy
/// decode, before metadata adaptation and handoff.
#[derive(Debug)]
struct NativeAdaptationSummary {
    width: u32,
    height: u32,
    image_description: Iad1,
    profile: EosR8Profile,
    as_shot_white_balance: EosR8AsShotWhiteBalance,
    orientation: Orientation,
}

impl NativeAdaptationSummary {
    fn from_frame(frame: NativeFrame<'_>) -> Self {
        let NativeFrame {
            config,
            metadata,
            as_shot_white_balance,
            ..
        } = frame;
        Self {
            width: config.compression.image_width,
            height: config.compression.image_height,
            image_description: config.image_description,
            profile: metadata.profile,
            as_shot_white_balance,
            orientation: metadata.orientation,
        }
    }
}

/// Semantic contract for the first native EOS R8 CR3 decoder.
///
/// The digest conservatively covers the complete resolved workspace lockfile.
/// The backend revision covers local entropy/parser changes and must be bumped
/// whenever decoded pixels or metadata can change.
pub const NATIVE_EOS_R8_MOSAIC_CONTRACT_1: MosaicRecipeManifest = MosaicRecipeManifest::new(
    NATIVE_CR3_BACKEND_ID,
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
pub struct NativeCr3Decoder;

impl RawDecoder for NativeCr3Decoder {
    fn mosaic_recipe(&self, _request: &DecodeRequest) -> Result<MosaicRecipeManifest, DecodeError> {
        Ok(NATIVE_EOS_R8_MOSAIC_CONTRACT_1)
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
        let frame = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse(&data)))
            .map_err(|_| DecodeError::DecoderPanicked)?
            .map_err(|error| DecodeError::NativeCr3(error.to_string()))?;
        let decoder_select = decoder_select_started.elapsed();
        request.check_cancelled()?;

        let raw_image_started = Instant::now();
        let (pixels, native) = decode_sensor_pixels(&frame, request)?;
        let adaptation = NativeAdaptationSummary::from_frame(frame);
        drop(data);
        let raw_image = raw_image_started.elapsed();
        request.check_cancelled()?;

        let (mosaic, adapt) = adapt_native_frame(&adaptation, pixels)?;
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
                native: Some(native),
                dng: None,
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

fn decode_sensor_pixels(
    frame: &NativeFrame<'_>,
    request: &DecodeRequest,
) -> Result<(Vec<u16>, NativeDecodeTimings), DecodeError> {
    let sensor_width = frame.config.compression.image_width;
    let sensor_height = frame.config.compression.image_height;
    let plane_width = usize::try_from(sensor_width / 2).map_err(|_| DecodeError::DimensionOverflow)?;
    let plane_height = usize::try_from(sensor_height / 2).map_err(|_| DecodeError::DimensionOverflow)?;
    let bit_depth = frame.config.compression.n_bits;
    let plane_data = frame.planes.each_ref().map(|plane| plane.data);
    let cancelled = || {
        request
            .cancellation
            .as_ref()
            .is_some_and(crate::GenerationToken::is_cancelled)
    };

    let requested_workers = requested_plane_workers();
    let available_workers = std::thread::available_parallelism().map_or(1, usize::from);
    if should_stream_planes(requested_workers, available_workers) {
        let decode_rows = |_plane_index: usize,
                           bytes: &[u8],
                           should_cancel: &dyn Fn() -> bool,
                           emit_row: &mut dyn FnMut(usize, Vec<u16>) -> Option<Vec<u16>>|
         -> Result<(), LosslessError> {
            lossless::decode_plane_rows(
                bytes,
                plane_width,
                plane_height,
                bit_depth,
                should_cancel,
                emit_row,
            )
        };
        let batch =
            decode_four_planes_streaming(plane_data, sensor_width, sensor_height, &cancelled, &decode_rows)
                .map_err(map_streaming_error)?;
        let worker_count = u8::try_from(batch.worker_count).unwrap_or(4);
        return Ok((
            batch.pixels,
            NativeDecodeTimings {
                plane_decode: batch.plane_elapsed,
                plane_wall: batch.wall_elapsed,
                interleave: batch.interleave_elapsed,
                worker_count,
            },
        ));
    }

    let decode = |_plane_index: usize,
                  bytes: &[u8],
                  should_cancel: &dyn Fn() -> bool|
     -> Result<Vec<u16>, LosslessError> {
        lossless::decode_plane(bytes, plane_width, plane_height, bit_depth, should_cancel)
    };
    let batch =
        decode_four_planes(plane_data, requested_workers, &cancelled, &decode).map_err(map_parallel_error)?;
    let worker_count = u8::try_from(batch.worker_count).unwrap_or(4);
    let interleave_started = Instant::now();
    let plane_slices = batch.planes.each_ref().map(Vec::as_slice);
    let pixels = interleave_rggb(plane_slices, sensor_width, sensor_height)
        .map_err(|error| DecodeError::NativeCr3(error.to_string()))?;
    let interleave = interleave_started.elapsed();

    Ok((
        pixels,
        NativeDecodeTimings {
            plane_decode: batch.plane_elapsed,
            plane_wall: batch.wall_elapsed,
            interleave,
            worker_count,
        },
    ))
}

fn map_streaming_error(error: StreamingDecodeError<LosslessError>) -> DecodeError {
    match error {
        StreamingDecodeError::Parallel(error) => map_parallel_error(error),
        other => DecodeError::NativeCr3(other.to_string()),
    }
}

fn map_parallel_error(error: ParallelDecodeError<LosslessError>) -> DecodeError {
    match error {
        ParallelDecodeError::Cancelled => DecodeError::Cancelled,
        other => DecodeError::NativeCr3(other.to_string()),
    }
}

fn adapt_native_frame(
    frame: &NativeAdaptationSummary,
    pixels: Vec<u16>,
) -> Result<(DecodedMosaic, AdaptTimings), DecodeError> {
    let total_started = Instant::now();

    let layout_started = Instant::now();
    let width = frame.width;
    let height = frame.height;
    let profile = frame.profile;
    let cfa = CfaPattern {
        width: 2,
        height: 2,
        cells: profile.cfa.into_iter().collect(),
    };
    let layout_cfa = layout_started.elapsed();

    let levels_started = Instant::now();
    let black_level = LevelGrid {
        width: 2,
        height: 2,
        components: 1,
        values: profile.black_level.into_iter().collect(),
    };
    let white_level = WhiteLevel(vec![profile.white_level]);
    let levels = levels_started.elapsed();

    let color_started = Instant::now();
    let white_balance = frame.as_shot_white_balance.gains();
    let xyz_to_camera = profile.xyz_to_camera;
    let color = color_started.elapsed();

    let geometry_started = Instant::now();
    let geometry = frame
        .image_description
        .eos_r8_sensor_geometry()
        .ok_or_else(|| DecodeError::NativeCr3("unsupported EOS R8 sensor geometry".into()))?;
    let active_area = Some(Rect::new(
        geometry.active_area.x,
        geometry.active_area.y,
        geometry.active_area.width,
        geometry.active_area.height,
    ));
    let crop_area = Some(Rect::new(
        geometry.crop_area.x,
        geometry.crop_area.y,
        geometry.crop_area.width,
        geometry.crop_area.height,
    ));
    let geometry_elapsed = geometry_started.elapsed();

    let finalize_started = Instant::now();
    let metadata = RawMetadata {
        make: crate::cr3::metadata::EosR8Metadata::canonical_make().into(),
        model: crate::cr3::metadata::EosR8Metadata::canonical_model().into(),
        width,
        height,
        components_per_pixel: 1,
        bits_per_sample: profile.bits_per_sample,
        photometric: Photometric::Cfa,
        cfa: Some(cfa),
        black_level,
        white_level,
        white_balance,
        xyz_to_camera,
        active_area,
        crop_area,
        orientation: frame.orientation,
    };
    let mosaic = DecodedMosaic::new(metadata, Arc::new(pixels))?;
    let finalize = finalize_started.elapsed();
    let total = total_started.elapsed();

    Ok((
        mosaic,
        AdaptTimings {
            layout_cfa,
            levels,
            color,
            geometry: geometry_elapsed,
            finalize,
            total,
        },
    ))
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
    #[allow(clippy::format_collect)]
    fn recipe_is_distinct_and_complete() {
        assert_eq!(
            NativeCr3Decoder
                .mosaic_recipe(&DecodeRequest::new("fixture.CR3"))
                .unwrap(),
            NATIVE_EOS_R8_MOSAIC_CONTRACT_1
        );
        assert_eq!(
            NATIVE_EOS_R8_MOSAIC_CONTRACT_1.decoder_backend_id(),
            NATIVE_CR3_BACKEND_ID
        );
        let actual = NATIVE_EOS_R8_MOSAIC_CONTRACT_1.canonical_bytes()[28..60]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            actual,
            include_str!("../../../scripts/native-cr3-semantic-lock.sha256").trim()
        );
    }

    #[test]
    fn stale_request_is_rejected_before_io() {
        let generation = Arc::new(AtomicU64::new(2));
        let mut request = DecodeRequest::new("does-not-exist.CR3");
        request.cancellation = Some(GenerationToken::new(Arc::clone(&generation), 1));
        assert!(matches!(
            NativeCr3Decoder.decode(&request),
            Err(DecodeError::Cancelled)
        ));
        generation.store(3, Ordering::Release);
    }

    #[test]
    fn nonzero_image_index_is_rejected_before_io() {
        let mut request = DecodeRequest::new("does-not-exist.CR3");
        request.image_index = 1;
        assert!(matches!(
            NativeCr3Decoder.decode(&request),
            Err(DecodeError::UnsupportedImageIndex { index: 1 })
        ));
    }

    #[test]
    fn adaptation_summary_cannot_borrow_the_source_buffer() {
        fn assert_static<T: 'static>() {}

        assert_static::<NativeAdaptationSummary>();
    }

    #[test]
    fn plane_worker_knob_accepts_only_supported_values() {
        assert_eq!(parse_plane_workers(None), DEFAULT_PLANE_WORKERS);
        assert_eq!(parse_plane_workers(Some("1")), 1);
        assert_eq!(parse_plane_workers(Some("2")), 2);
        assert_eq!(parse_plane_workers(Some("4")), 4);
        for invalid in ["0", "3", "5", "8", "", "four", " 2", "4 "] {
            assert_eq!(
                parse_plane_workers(Some(invalid)),
                DEFAULT_PLANE_WORKERS,
                "{invalid:?} must fall back to the default"
            );
        }
    }

    #[test]
    fn streaming_requires_full_plane_coverage_and_a_spare_thread() {
        assert!(should_stream_planes(4, 5));
        assert!(should_stream_planes(4, 10));
        assert!(!should_stream_planes(4, 4));
        assert!(!should_stream_planes(4, 1));
        assert!(!should_stream_planes(2, 10));
        assert!(!should_stream_planes(1, 10));
        assert!(!should_stream_planes(0, 10));
    }
}
