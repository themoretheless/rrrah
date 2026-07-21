//! Decoder port and the `rawler` adapter.
//!
//! The adapter requests `Decoder::raw_image(..., dummy = false)`. It never asks
//! for `thumbnail_image`, `preview_image`, or `full_image`, which makes the
//! no-embedded-JPEG invariant explicit in code.
#![allow(clippy::missing_errors_doc, clippy::cast_precision_loss)]

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use rawler::{
    decoders::{Orientation as RawlerOrientation, RawDecodeParams},
    rawimage::{RawImage, RawImageData, RawPhotometricInterpretation},
    rawsource::RawSource,
};
use rrrah_core::{
    CfaColor, CfaPattern, DecodedMosaic, FrameError, LevelGrid, Orientation, Photometric, RawMetadata, Rect,
    WhiteLevel,
};
use thiserror::Error;

pub trait RawDecoder: Send + Sync {
    fn decode(&self, request: &DecodeRequest) -> Result<DecodeOutput, DecodeError>;
}

#[derive(Debug, Clone)]
pub struct DecodeRequest {
    pub path: PathBuf,
    pub image_index: usize,
    pub cancellation: Option<GenerationToken>,
}

impl DecodeRequest {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            image_index: 0,
            cancellation: None,
        }
    }

    fn check_cancelled(&self) -> Result<(), DecodeError> {
        if self
            .cancellation
            .as_ref()
            .is_some_and(GenerationToken::is_cancelled)
        {
            Err(DecodeError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub struct GenerationToken {
    generation: Arc<AtomicU64>,
    expected: u64,
}

impl GenerationToken {
    pub fn new(generation: Arc<AtomicU64>, expected: u64) -> Self {
        Self { generation, expected }
    }

    pub fn is_cancelled(&self) -> bool {
        self.generation.load(Ordering::Acquire) != self.expected
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DecodeTimings {
    pub source_open: Duration,
    pub raw_decode: Duration,
    pub adapt_metadata: Duration,
    pub total: Duration,
}

#[derive(Debug, Clone)]
pub struct DecodeOutput {
    pub mosaic: DecodedMosaic,
    pub timings: DecodeTimings,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RawlerDecoder;

impl RawDecoder for RawlerDecoder {
    fn decode(&self, request: &DecodeRequest) -> Result<DecodeOutput, DecodeError> {
        let total_started = Instant::now();
        request.check_cancelled()?;

        let source_started = Instant::now();
        let source = RawSource::new(&request.path).map_err(|source| DecodeError::Io {
            path: request.path.clone(),
            source,
        })?;
        let source_open = source_started.elapsed();
        request.check_cancelled()?;

        let decode_started = Instant::now();
        let image = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let decoder = rawler::get_decoder(&source).map_err(|error| error.to_string())?;
            // `false` is the critical semantic choice: decode sensor samples,
            // not a dummy image and not an embedded preview.
            decoder
                .raw_image(
                    &source,
                    &RawDecodeParams {
                        image_index: request.image_index,
                    },
                    false,
                )
                .map_err(|error| error.to_string())
        }))
        .map_err(|_| DecodeError::DecoderPanicked)?
        .map_err(DecodeError::Rawler)?;
        let raw_decode = decode_started.elapsed();
        request.check_cancelled()?;

        let adapt_started = Instant::now();
        let mosaic = adapt_rawler_image(image)?;
        let adapt_metadata = adapt_started.elapsed();
        request.check_cancelled()?;

        Ok(DecodeOutput {
            mosaic,
            timings: DecodeTimings {
                source_open,
                raw_decode,
                adapt_metadata,
                total: total_started.elapsed(),
            },
        })
    }
}

fn adapt_rawler_image(image: RawImage) -> Result<DecodedMosaic, DecodeError> {
    let width = u32::try_from(image.width).map_err(|_| DecodeError::DimensionOverflow)?;
    let height = u32::try_from(image.height).map_err(|_| DecodeError::DimensionOverflow)?;
    let components_per_pixel = u8::try_from(image.cpp).map_err(|_| DecodeError::DimensionOverflow)?;
    let bits_per_sample = u8::try_from(image.bps).map_err(|_| DecodeError::DimensionOverflow)?;

    let (photometric, cfa) = match &image.photometric {
        RawPhotometricInterpretation::Cfa(config) => {
            let cells = config.cfa.flat_pattern().into_iter().map(map_cfa_color).collect();
            (
                Photometric::Cfa,
                Some(CfaPattern {
                    width: u8::try_from(config.cfa.width).map_err(|_| DecodeError::DimensionOverflow)?,
                    height: u8::try_from(config.cfa.height).map_err(|_| DecodeError::DimensionOverflow)?,
                    cells,
                }),
            )
        }
        RawPhotometricInterpretation::LinearRaw => (Photometric::LinearRaw, None),
        RawPhotometricInterpretation::BlackIsZero => (Photometric::BlackIsZero, None),
    };

    let black_level = LevelGrid {
        width: u8::try_from(image.blacklevel.width).map_err(|_| DecodeError::DimensionOverflow)?,
        height: u8::try_from(image.blacklevel.height).map_err(|_| DecodeError::DimensionOverflow)?,
        components: u8::try_from(image.blacklevel.cpp).map_err(|_| DecodeError::DimensionOverflow)?,
        values: image
            .blacklevel
            .levels
            .iter()
            .map(rawler::formats::tiff::Rational::as_f32)
            .collect(),
    };

    let white_balance = sanitize_white_balance(image.wb_coeffs);
    let xyz_to_camera = camera_matrix(&image);

    let metadata = RawMetadata {
        make: image.clean_make,
        model: image.clean_model,
        width,
        height,
        components_per_pixel,
        bits_per_sample,
        photometric,
        cfa,
        black_level,
        white_level: WhiteLevel(image.whitelevel.0.into_iter().map(|value| value as f32).collect()),
        white_balance,
        xyz_to_camera,
        active_area: image.active_area.map(convert_rect).transpose()?,
        crop_area: image.crop_area.map(convert_rect).transpose()?,
        orientation: map_orientation(image.orientation),
    };

    let pixels: Arc<[u16]> = match image.data {
        RawImageData::Integer(data) => Arc::from(data.into_boxed_slice()),
        RawImageData::Float(_) => return Err(DecodeError::UnsupportedFloatRaw),
    };
    DecodedMosaic::new(metadata, pixels).map_err(DecodeError::InvalidFrame)
}

/// Prefer Rawler's current illuminant-tagged matrix. `xyz_to_cam` is retained
/// by Rawler for compatibility and is zero for some otherwise profiled Canon
/// cameras (including the 5DS fixture). Keep the legacy matrix only as a
/// fallback for files without a color-matrix map.
fn camera_matrix(image: &RawImage) -> [[f32; 3]; 4] {
    let values = image
        .color_matrix
        .get(&rawler::imgop::xyz::Illuminant::D65)
        .or_else(|| image.color_matrix.values().next());
    let Some(values) = values else {
        return image.xyz_to_cam;
    };
    if values.len() < 9 || values.len() % 3 != 0 {
        return image.xyz_to_cam;
    }
    let mut matrix = [[0.0_f32; 3]; 4];
    for (row, chunk) in values.chunks_exact(3).take(4).enumerate() {
        matrix[row].copy_from_slice(chunk);
    }
    matrix
}

/// Rawler represents a three-channel RGB white balance as `[R, G, B, NaN]`.
/// The fourth value is an absent second green plane, not a corrupt profile.
/// Preserve the calibrated RGB gains and make the optional slot finite for our
/// metadata/cache invariants.
fn sanitize_white_balance(mut white_balance: [f32; 4]) -> [f32; 4] {
    if white_balance[..3]
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return [1.0; 4];
    }
    if !white_balance[3].is_finite() || white_balance[3] <= 0.0 {
        white_balance[3] = white_balance[1];
    }
    white_balance
}

fn convert_rect(rect: rawler::imgop::Rect) -> Result<Rect, DecodeError> {
    Ok(Rect::new(
        u32::try_from(rect.p.x).map_err(|_| DecodeError::DimensionOverflow)?,
        u32::try_from(rect.p.y).map_err(|_| DecodeError::DimensionOverflow)?,
        u32::try_from(rect.d.w).map_err(|_| DecodeError::DimensionOverflow)?,
        u32::try_from(rect.d.h).map_err(|_| DecodeError::DimensionOverflow)?,
    ))
}

const fn map_cfa_color(value: u8) -> CfaColor {
    match value {
        0 => CfaColor::Red,
        1 => CfaColor::Green,
        2 => CfaColor::Blue,
        3 => CfaColor::Cyan,
        4 => CfaColor::Magenta,
        5 => CfaColor::Yellow,
        6 => CfaColor::White,
        _ => CfaColor::Unknown,
    }
}

const fn map_orientation(value: RawlerOrientation) -> Orientation {
    match value {
        RawlerOrientation::Normal => Orientation::Normal,
        RawlerOrientation::HorizontalFlip => Orientation::HorizontalFlip,
        RawlerOrientation::Rotate180 => Orientation::Rotate180,
        RawlerOrientation::VerticalFlip => Orientation::VerticalFlip,
        RawlerOrientation::Transpose => Orientation::Transpose,
        RawlerOrientation::Rotate90 => Orientation::Rotate90,
        RawlerOrientation::Transverse => Orientation::Transverse,
        RawlerOrientation::Rotate270 => Orientation::Rotate270,
        RawlerOrientation::Unknown => Orientation::Unknown,
    }
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("failed to open RAW source {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("RAW decoder failed: {0}")]
    Rawler(String),
    #[error("RAW decoder panicked; decode untrusted files in a sandboxed worker")]
    DecoderPanicked,
    #[error("decode was superseded by a newer open request")]
    Cancelled,
    #[error("RAW dimensions do not fit the domain representation")]
    DimensionOverflow,
    #[error("floating-point/linear DNG is not supported by the Bayer fast path yet")]
    UnsupportedFloatRaw,
    #[error("decoded frame is invalid: {0}")]
    InvalidFrame(#[from] FrameError),
}

pub fn decode_file(path: impl AsRef<Path>) -> Result<DecodeOutput, DecodeError> {
    RawlerDecoder.decode(&DecodeRequest::new(path.as_ref()))
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        path::{Path, PathBuf},
    };

    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };

    use rawler::{
        CFA,
        cfa::PlaneColor,
        decoders::Camera,
        rawimage::{BlackLevel, CFAConfig, RawImage, RawImageData, RawPhotometricInterpretation, WhiteLevel},
    };
    use rrrah_core::{CfaColor, FrameError, Photometric};

    use super::{DecodeError, DecodeRequest, GenerationToken, RawDecoder, RawlerDecoder, adapt_rawler_image};

    fn camera(cfa: &str) -> Camera {
        let mut camera = Camera::new();
        camera.make = "fixture".to_string();
        camera.model = "deterministic".to_string();
        camera.clean_make = camera.make.clone();
        camera.clean_model = camera.model.clone();
        camera.cfa = CFA::new(cfa);
        camera.plane_color = PlaneColor::new("RGB");
        camera.real_bps = 12;
        camera.xyz_to_cam = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]];
        camera
    }

    fn raw_image(
        cfa: &str,
        data: RawImageData,
        width: usize,
        height: usize,
        photometric: RawPhotometricInterpretation,
    ) -> RawImage {
        raw_image_with_camera(cfa, data, width, height, photometric, [1.0; 4], None)
    }

    fn raw_image_with_wb(
        cfa: &str,
        data: RawImageData,
        width: usize,
        height: usize,
        photometric: RawPhotometricInterpretation,
        wb: [f32; 4],
    ) -> RawImage {
        raw_image_with_camera(cfa, data, width, height, photometric, wb, None)
    }

    fn raw_image_with_camera(
        cfa: &str,
        data: RawImageData,
        width: usize,
        height: usize,
        photometric: RawPhotometricInterpretation,
        wb: [f32; 4],
        color_matrix: Option<Vec<f32>>,
    ) -> RawImage {
        let mut camera = camera(cfa);
        if let Some(matrix) = color_matrix {
            camera
                .color_matrix
                .insert(rawler::imgop::xyz::Illuminant::D65, matrix);
            camera.xyz_to_cam = [[0.0; 3]; 4];
        }
        RawImage::new_with_data(
            camera,
            data,
            width,
            height,
            1,
            wb,
            photometric,
            Some(BlackLevel::zero(1, 1, 1)),
            Some(WhiteLevel::new([4095_u32])),
            false,
        )
    }

    #[test]
    fn preserves_three_channel_wb_when_fourth_plane_is_absent() {
        let image = raw_image_with_wb(
            "RGGB",
            RawImageData::Integer(vec![1, 2, 3, 4]),
            2,
            2,
            RawPhotometricInterpretation::Cfa(CFAConfig::new(&CFA::new("RGGB"), &PlaneColor::new("RGB"))),
            [2.0, 1.0, 3.0, f32::NAN],
        );
        let output = adapt_rawler_image(image).expect("metadata adaptation");
        for (actual, expected) in output
            .metadata
            .white_balance
            .into_iter()
            .zip([2.0, 1.0, 3.0, 1.0])
        {
            assert!((actual - expected).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn prefers_tagged_color_matrix_over_legacy_zero_matrix() {
        let image = raw_image_with_camera(
            "RGGB",
            RawImageData::Integer(vec![1, 2, 3, 4]),
            2,
            2,
            RawPhotometricInterpretation::Cfa(CFAConfig::new(&CFA::new("RGGB"), &PlaneColor::new("RGB"))),
            [1.0; 4],
            Some(vec![0.8, 0.1, 0.05, 0.1, 0.9, 0.05, 0.05, 0.1, 0.85]),
        );
        let output = adapt_rawler_image(image).expect("metadata adaptation");
        for (actual, expected) in output.metadata.xyz_to_camera[0].into_iter().zip([0.8, 0.1, 0.05]) {
            assert!((actual - expected).abs() < f32::EPSILON);
        }
        for (actual, expected) in output.metadata.xyz_to_camera[2]
            .into_iter()
            .zip([0.05, 0.1, 0.85])
        {
            assert!((actual - expected).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn generation_token_cancels_stale_work() {
        let generation = Arc::new(AtomicU64::new(7));
        let token = GenerationToken::new(generation.clone(), 7);
        assert!(!token.is_cancelled());
        generation.store(8, Ordering::Release);
        assert!(token.is_cancelled());
    }

    #[test]
    fn decode_request_rejects_stale_generation_before_io() {
        let generation = Arc::new(AtomicU64::new(12));
        let request = DecodeRequest {
            path: "does-not-exist.CR2".into(),
            image_index: 0,
            cancellation: Some(GenerationToken::new(generation.clone(), 11)),
        };
        assert!(matches!(request.check_cancelled(), Err(DecodeError::Cancelled)));
        assert!(matches!(
            RawlerDecoder.decode(&request),
            Err(DecodeError::Cancelled)
        ));
        // A current generation remains admissible; this check exercises the
        // same atomic ordering without opening a file or allocating pixels.
        let current = DecodeRequest {
            path: "does-not-exist.CR2".into(),
            image_index: 0,
            cancellation: Some(GenerationToken::new(generation, 12)),
        };
        assert!(current.check_cancelled().is_ok());
    }

    #[test]
    fn cancellation_remains_observable_after_multiple_generation_updates() {
        let generation = Arc::new(AtomicU64::new(31));
        let token = GenerationToken::new(generation.clone(), 31);
        generation.store(32, Ordering::Release);
        assert!(token.is_cancelled());
        generation.store(33, Ordering::Release);
        assert!(token.is_cancelled());
        generation.store(34, Ordering::Release);
        assert!(token.is_cancelled());
    }

    #[test]
    fn float_raw_is_rejected_without_quantizing_or_using_preview() {
        let image = raw_image(
            "RGGB",
            RawImageData::Float(vec![0.0, 0.25, 0.5, 1.0]),
            2,
            2,
            RawPhotometricInterpretation::Cfa(CFAConfig::new(&CFA::new("RGGB"), &PlaneColor::new("RGB"))),
        );
        assert!(matches!(
            adapt_rawler_image(image),
            Err(DecodeError::UnsupportedFloatRaw)
        ));
    }

    #[test]
    fn unsupported_cfa_is_preserved_and_rejected_by_fast_path_validation() {
        let image = raw_image(
            "RRGG",
            RawImageData::Integer(vec![1, 2, 3, 4]),
            2,
            2,
            RawPhotometricInterpretation::Cfa(CFAConfig::new(&CFA::new("RRGG"), &PlaneColor::new("RGB"))),
        );
        let output = adapt_rawler_image(image).expect("adapter must not rewrite CFA metadata");
        let cfa = output.metadata.cfa.expect("CFA metadata");
        assert_eq!(
            cfa.cells,
            vec![CfaColor::Red, CfaColor::Red, CfaColor::Green, CfaColor::Green]
        );
        assert!(matches!(
            cfa.bayer_quad(),
            Err(FrameError::UnsupportedCfa { width: 2, height: 2 })
        ));
    }

    #[test]
    fn non_cfa_photometric_is_explicit_and_never_substituted_with_jpeg() {
        let image = raw_image(
            "RGGB",
            RawImageData::Integer(vec![1, 2, 3, 4]),
            2,
            2,
            RawPhotometricInterpretation::BlackIsZero,
        );
        let output = adapt_rawler_image(image).expect("metadata adaptation");
        assert_eq!(output.metadata.photometric, Photometric::BlackIsZero);
        assert!(output.metadata.cfa.is_none());
    }

    fn configured_fixture(var: &str) -> Option<PathBuf> {
        let path = env::var_os(var).map(PathBuf::from)?;
        assert!(path.is_file(), "{var} points to a non-file: {}", path.display());
        Some(path)
    }

    fn assert_full_raw_fixture(path: &Path, expected_extension: &str) {
        assert_eq!(
            path.extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase),
            Some(expected_extension.to_string()),
            "fixture extension is part of the format contract"
        );
        let output = RawlerDecoder
            .decode(&DecodeRequest::new(path))
            .unwrap_or_else(|error| panic!("{expected_extension} fixture failed: {error}"));
        assert!(output.mosaic.metadata.width > 0);
        assert!(output.mosaic.metadata.height > 0);
        assert!(!output.mosaic.pixels.is_empty());
        assert_eq!(
            output.mosaic.pixels.len(),
            usize::try_from(output.mosaic.metadata.width).unwrap()
                * usize::try_from(output.mosaic.metadata.height).unwrap()
                * usize::from(output.mosaic.metadata.components_per_pixel)
        );
    }

    #[test]
    fn configured_cr2_fixture_decodes_sensor_samples() {
        let Some(path) = configured_fixture("RRRAH_CR2_FIXTURE") else {
            eprintln!("RRRAH_CR2_FIXTURE not set; skipping licensed CR2 corpus test");
            return;
        };
        assert_full_raw_fixture(&path, "cr2");
    }

    #[test]
    fn configured_dng_fixture_decodes_sensor_samples() {
        let Some(path) = configured_fixture("RRRAH_DNG_FIXTURE") else {
            eprintln!("RRRAH_DNG_FIXTURE not set; skipping licensed DNG corpus test");
            return;
        };
        assert_full_raw_fixture(&path, "dng");
    }

    #[test]
    fn configured_float_dng_fixture_has_an_explicit_typed_result() {
        let Some(path) = configured_fixture("RRRAH_DNG_FLOAT_FIXTURE") else {
            eprintln!("RRRAH_DNG_FLOAT_FIXTURE not set; skipping float DNG negative test");
            return;
        };
        assert_eq!(
            path.extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase),
            Some("dng".to_string())
        );
        let result = RawlerDecoder.decode(&DecodeRequest::new(&path));
        assert!(
            matches!(result, Err(DecodeError::UnsupportedFloatRaw)),
            "float DNG must be rejected explicitly until the linear-float path is implemented: {result:?}"
        );
    }
}
