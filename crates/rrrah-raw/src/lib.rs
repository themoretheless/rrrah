//! Native RAW decoding and domain adaptation.
//!
//! Includes clean-room Canon EOS R8 CR3, TIFF/DNG, CR2, CRW/CIFF, NEF/NRW, ARW/SR2/SRF,
//! ORF, PEF/PTX, RW2/RWL, RAF, MRW, ERF, KDC/DCR/DCS, SRW, 3FR/FFF, MOS, IIQ/EIP,
//! MEF, BAY, Optio S4, and GoPro GPR/VC-5 sensor decoders.
#![allow(
    clippy::pedantic,
    clippy::chunks_exact_to_as_chunks,
    clippy::collapsible_if,
    clippy::missing_errors_doc,
    clippy::cast_precision_loss,
    clippy::manual_is_multiple_of,
    clippy::manual_checked_ops,
    clippy::too_many_arguments,
    clippy::precedence,
    clippy::manual_contains,
    clippy::needless_range_loop,
    clippy::manual_range_patterns
)]

pub mod bounded_io;
mod optio_s4;
pub use optio_s4::{
    OPTIO_S4_SENSOR_SIZE, OptioS4SensorError, read_optio_s4_sensor_with_budget, unpack_optio_s4_sensor,
};

mod bay;
pub use bay::{
    BayReadError, BaySensorError, BaySensorLayout, read_bay_image_sensor_with_budget,
    read_bay_sensor_with_budget, unpack_bay_sensor,
};

mod ciff;
mod crw;
mod crw_entropy;
pub use crw::{NativeCrwDecoder, decode_crw_10d, decode_crw_10d_with_tables};

mod eip;
pub use eip::{
    EipEntry, EipError, EipManifest, EipRaw, EipSensor, decode_eip_file_sensor, decode_eip_sensor,
    eip_file_sensor_recipe, inspect_eip, read_eip_raw, read_eip_raw_with_budget,
};

mod camtiff;
pub use camtiff::erf::{EpsonMakerMetadata, read_erf_maker_metadata};
pub use camtiff::{decode_mef_zd_sensor, decode_mef_zd_sensor_file};
mod mrw;
pub use mrw::{MrwLayout, NativeMrwDecoder};
mod cr3;
#[doc(hidden)]
pub mod dng;
mod dng_backend;
mod native_backend;
mod native_router;
pub mod sniff;
pub mod vc5;
pub mod x3f;
pub use x3f::{
    X3fCamf, X3fCamfEntry, X3fCamfMatrix, X3fColorTransform, X3fEntry, X3fError, X3fImageInfo, X3fInventory,
    X3fLegacyChannels, X3fLegacyHuffman, X3fNoiseCurve, X3fProperties, crop_x3f_channels, decode_x3f_legacy,
    decode_x3f_legacy_auto, inspect_x3f, linearize_x3f_highlights, linearize_x3f_highlights_from_camf,
    repair_x3f_bad_pixels, repair_x3f_bad_pixels_from_camf, rotate_x3f_linear, sharpen_x3f_red,
    smooth_x3f_black_rows, smooth_x3f_chroma, smooth_x3f_hues, smooth_x3f_hues_wide, transform_x3f_pixels,
    visit_x3f_camf, x3f_block_enabled, x3f_chroma_guide, x3f_color_transform_auto, x3f_illuminant_matrix,
    x3f_linear_output, x3f_linear_raster, x3f_neutral_response, x3f_noise_curves_auto, x3f_wb_correction,
};

pub mod raster {
    /// Maximum single raster / VC-5 scratch allocation when no explicit budget is supplied (512 MiB).
    pub const MAX_RASTER_BYTES: u64 = 512 * 1024 * 1024;
}

/// SHA-256 digest of the resolved workspace lockfile, shared by every native
/// backend's semantic recipe. Must stay in sync with
/// `scripts/native-cr3-semantic-lock.sha256`.
pub const WORKSPACE_LOCK_DIGEST: [u8; 32] = [
    0xfc, 0x7f, 0xa9, 0x6e, 0x12, 0x29, 0x5d, 0x02, 0x5b, 0x2d, 0xf0, 0xd3, 0x8b, 0xcd, 0xcb, 0x1e, 0xb6,
    0x04, 0x3a, 0xfe, 0x0b, 0xeb, 0x34, 0x85, 0x6e, 0xe9, 0x20, 0x74, 0x08, 0x78, 0xb6, 0xf1,
];

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

pub use dng_backend::{NATIVE_DNG_BACKEND_ID, NATIVE_DNG_MOSAIC_CONTRACT_1, NativeDngDecoder};
pub use native_backend::{NATIVE_CR3_BACKEND_ID, NATIVE_EOS_R8_MOSAIC_CONTRACT_1, NativeCr3Decoder};
pub use native_router::NativeRawDecoder;
use rrrah_core::{DecodedMosaic, FrameError, MosaicRecipeManifest};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RlaAlphaMode {
    Straight,
    Premultiplied,
}

/// Explicit producer color interpretation; float storage alone does not declare linear light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RlaColorSpace {
    Srgb,
    LinearSrgb,
}

/// Float RLA records have producer-dependent byte order; never infer it from values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RlaFloatByteOrder {
    Little,
    Big,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RpfRasterInterpretation {
    pub rgb: [u16; 3],
    pub matte: Option<u16>,
    pub alpha_mode: RlaAlphaMode,
    pub color_space: rrrah_core::RasterColorSpace,
}

pub trait RawDecoder: Send + Sync {
    fn mosaic_recipe(&self, request: &DecodeRequest) -> Result<MosaicRecipeManifest, DecodeError>;
    fn decode(&self, request: &DecodeRequest) -> Result<DecodeOutput, DecodeError>;
}

#[derive(Debug, Clone)]
pub struct DecodeRequest {
    pub path: PathBuf,
    pub image_index: usize,
    /// Explicit RPF channel, alpha and color interpretation. No inferred defaults.
    pub rpf_interpretation: Option<RpfRasterInterpretation>,
    /// Explicit producer-declared alpha association for RLA. None refuses
    /// alpha-bearing RLA; color interpretation remains a separate policy.
    pub rla_alpha_mode: Option<RlaAlphaMode>,
    /// Explicit byte order of producer-native float RLA records. None refuses float input.
    pub rla_float_byte_order: Option<RlaFloatByteOrder>,
    /// Explicit producer color interpretation for RLA, independent of alpha/storage.
    pub rla_color_space: Option<RlaColorSpace>,
    /// Explicit reference-white luminance for PQ BT.2020 conversion.
    pub pq_reference_white_nits: Option<f32>,
    /// HLG zero-black display: reference white, peak luminance, system gamma.
    pub hlg_display: Option<(f32, f32, f32)>,
    /// Explicit conventional interpretation for untagged TIFF, PNM, TGA and RLA.
    /// Defaults to false. Embedded ICC and declared color metadata take priority;
    /// unspecified linear/HDR primaries are never overridden by this setting.
    pub assume_untagged_srgb: bool,
    /// Explicit sRGB primaries/white point for samples already declared linear
    /// but lacking primaries (e.g. PFM). Defaults to false; tagged color/ICC and
    /// unspecified integer samples are never overridden. Unprofiled float TIFF
    /// may explicitly use linear sRGB. Does not apply a transfer curve.
    pub assume_untagged_linear_srgb: bool,
    /// Explicit Adobe-style PFM storage order. Default is bottom-up.
    pub pfm_top_down: bool,
    pub cancellation: Option<GenerationToken>,
    /// Optional admission for supported native decoder output allocations.
    pub memory_budget: Option<rrrah_core::MemoryBudget>,
}

impl DecodeRequest {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            image_index: 0,
            rpf_interpretation: None,
            rla_alpha_mode: None,
            rla_float_byte_order: None,
            rla_color_space: None,
            pq_reference_white_nits: None,
            hlg_display: None,
            assume_untagged_srgb: false,
            assume_untagged_linear_srgb: false,
            pfm_top_down: false,
            cancellation: None,
            memory_budget: None,
        }
    }

    pub fn qualify_linear_color(&self, color: rrrah_core::RasterColorSpace) -> rrrah_core::RasterColorSpace {
        if self.assume_untagged_linear_srgb && color == rrrah_core::RasterColorSpace::LinearRgbUnspecified {
            rrrah_core::RasterColorSpace::LinearSrgb
        } else {
            color
        }
    }

    pub fn check_cancelled(&self) -> Result<(), DecodeError> {
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
    additional: Arc<[(Arc<AtomicU64>, u64)]>,
}

impl GenerationToken {
    pub fn new(generation: Arc<AtomicU64>, expected: u64) -> Self {
        Self {
            generation,
            expected,
            additional: Arc::from([]),
        }
    }

    /// Cancellation from either owner terminates the shared operation. Flattened
    /// conditions avoid recursive token traversal and deduplicate identical owners.
    pub fn combine(&self, other: &Self) -> Self {
        let mut additional = self.additional.to_vec();
        for condition in std::iter::once((&other.generation, other.expected)).chain(
            other
                .additional
                .iter()
                .map(|(owner, expected)| (owner, *expected)),
        ) {
            if Arc::ptr_eq(&self.generation, condition.0) && self.expected == condition.1 {
                continue;
            }
            if !additional
                .iter()
                .any(|(owner, expected)| Arc::ptr_eq(owner, condition.0) && *expected == condition.1)
            {
                additional.push((Arc::clone(condition.0), condition.1));
            }
        }
        Self {
            generation: Arc::clone(&self.generation),
            expected: self.expected,
            additional: additional.into(),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.generation.load(Ordering::Acquire) != self.expected
            || self
                .additional
                .iter()
                .any(|(owner, expected)| owner.load(Ordering::Acquire) != *expected)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AdaptTimings {
    pub layout_cfa: Duration,
    pub levels: Duration,
    pub color: Duration,
    pub geometry: Duration,
    pub finalize: Duration,
    pub total: Duration,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DecodeTimings {
    pub source_open: Duration,
    pub decoder_select: Duration,
    pub raw_image: Duration,
    /// Exactly `decoder_select + raw_image`.
    pub raw_decode: Duration,
    pub native: Option<NativeDecodeTimings>,
    pub dng: Option<DngDecodeTimings>,
    pub adapt: AdaptTimings,
    pub adapt_metadata: Duration,
    pub total: Duration,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DngDecodeTimings {
    pub tiff_header: Duration,
    pub ifd_walk: Duration,
    pub raw_ifd_select: Duration,
    pub metadata: Duration,
    pub storage_plan: Duration,
    pub pixel_unpack: Duration,
    pub linearization: Duration,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDecodeTimings {
    pub plane_decode: [Duration; 4],
    pub plane_wall: Duration,
    pub interleave: Duration,
    pub worker_count: u8,
}

#[derive(Debug, Clone)]
pub struct DecodeOutput {
    pub mosaic: DecodedMosaic,
    pub timings: DecodeTimings,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error("failed to open RAW source {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("RAW source {path} has {actual} bytes, above the native decoder limit of {limit}")]
    InputTooLarge { path: PathBuf, actual: u64, limit: u64 },
    #[error("could not allocate {bytes} bytes for the bounded RAW input")]
    InputAllocation { bytes: usize },
    #[error("native EOS R8 CR3 decoder failed: {0}")]
    NativeCr3(String),
    #[error("native DNG decoder failed: {0}")]
    NativeDng(String),
    #[error("native {format} decoder failed: {message}")]
    NativeCamera { format: &'static str, message: String },
    #[error(
        "unsupported RAW format for {path}; expected .cr3, .cr2, .nef, .nrw, .arw, .orf, .pef, .rw2, .raf, .dng, .tif, or .tiff"
    )]
    UnsupportedFormat { path: PathBuf },
    #[error("image index {index} is unsupported for this source")]
    UnsupportedImageIndex { index: usize },
    #[error("RAW decoder panicked; decode untrusted files in a sandboxed worker")]
    DecoderPanicked,
    #[error("decode was superseded by a newer open request")]
    Cancelled,
    #[error("RAW dimensions do not fit the domain representation")]
    DimensionOverflow,
    #[error("decoded frame is invalid: {0}")]
    InvalidFrame(#[from] FrameError),
}

pub fn decode_file(path: impl AsRef<Path>) -> Result<DecodeOutput, DecodeError> {
    NativeRawDecoder.decode(&DecodeRequest::new(path.as_ref()))
}

/// Retrieve development instructions independently of cached sensor samples.
pub fn raw_development_opcodes(
    request: &DecodeRequest,
) -> Result<rrrah_core::develop::OpcodeLists, DecodeError> {
    request.check_cancelled()?;
    if !request.path.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case("dng")
            || e.eq_ignore_ascii_case("gpr")
            || e.eq_ignore_ascii_case("tif")
            || e.eq_ignore_ascii_case("tiff")
    }) {
        return Ok(rrrah_core::develop::OpcodeLists::default());
    }
    let data = bounded_io::read_managed(request)?;
    dng::opcode_lists(&data).map_err(DecodeError::NativeDng)
}

#[cfg(test)]
mod api_contract_regressions {
    use super::*;

    #[test]
    fn generation_token_cancels_stale_work() {
        let owner = Arc::new(AtomicU64::new(7));
        let token = GenerationToken::new(owner.clone(), 7);
        assert!(!token.is_cancelled());
        owner.store(8, Ordering::Release);
        assert!(token.is_cancelled());
        assert!(!GenerationToken::new(owner, 8).is_cancelled());
    }

    #[test]
    fn combined_generation_tokens_cancel_from_either_owner_and_deduplicate() {
        for cancelled_owner in 0..2 {
            let owners = [Arc::new(AtomicU64::new(4)), Arc::new(AtomicU64::new(9))];
            let first = GenerationToken::new(owners[0].clone(), 4);
            let second = GenerationToken::new(owners[1].clone(), 9);
            let combined = first.combine(&second).combine(&first).combine(&second);
            assert_eq!(combined.additional.len(), 1);
            assert!(!combined.is_cancelled());
            owners[cancelled_owner].fetch_add(1, Ordering::AcqRel);
            assert!(combined.is_cancelled());
        }
        let owner = Arc::new(AtomicU64::new(4));
        let token = GenerationToken::new(owner.clone(), 4);
        assert!(token.combine(&GenerationToken::new(owner, 5)).is_cancelled());
    }

    #[test]
    fn stale_request_is_rejected_without_io() {
        let mut request = DecodeRequest::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml/absent.cr3"),
        );
        request.cancellation = Some(GenerationToken::new(Arc::new(AtomicU64::new(2)), 1));
        let budget = rrrah_core::MemoryBudget::new(1024);
        request.memory_budget = Some(budget.clone());
        assert!(matches!(
            NativeRawDecoder.decode(&request),
            Err(DecodeError::Cancelled)
        ));
        assert!(matches!(
            NativeRawDecoder.mosaic_recipe(&request),
            Err(DecodeError::Cancelled)
        ));
        assert_eq!((budget.used(), budget.peak()), (0, 0));
    }

    #[test]
    fn explicit_linear_assumption_only_qualifies_unspecified_linear_primaries() {
        use rrrah_core::RasterColorSpace as Color;
        let mut request = DecodeRequest::new("unused.pfm");
        assert_eq!(
            request.qualify_linear_color(Color::LinearRgbUnspecified),
            Color::LinearRgbUnspecified
        );
        request.assume_untagged_srgb = true;
        assert_eq!(
            request.qualify_linear_color(Color::LinearRgbUnspecified),
            Color::LinearRgbUnspecified
        );
        request.assume_untagged_linear_srgb = true;
        assert_eq!(
            request.qualify_linear_color(Color::LinearRgbUnspecified),
            Color::LinearSrgb
        );
        for color in [Color::Srgb, Color::LinearSrgb] {
            assert_eq!(request.qualify_linear_color(color.clone()), color);
        }
    }
}

#[cfg(test)]
mod opcode_geometry_serde_compatibility_tests {
    #[test]
    fn old_lists_remain_square_and_new_aspect_roundtrips() {
        use rrrah_core::develop::OpcodeLists;
        let old = r#"{"list1":[],"list2":[],"list3":[]}"#;
        let lists: OpcodeLists = serde_json::from_str(old).unwrap();
        assert_eq!(lists.pixel_aspect, None);
        assert_eq!(
            serde_json::to_value(&lists).unwrap(),
            serde_json::from_str::<serde_json::Value>(old).unwrap()
        );
        for aspect in [0.5, 1.0, 2.0] {
            let mut current = lists.clone();
            current.pixel_aspect = Some(aspect);
            let encoded = serde_json::to_string(&current).unwrap();
            assert_eq!(serde_json::from_str::<OpcodeLists>(&encoded).unwrap(), current);
        }
    }
}

#[cfg(test)]
mod gpr_opcode_tests {
    use super::*;

    #[test]
    #[ignore = "requires pinned official GoPro sample corpus via RRRAH_GPR_CORPUS"]
    fn official_gpr_corrections_are_not_silently_omitted() {
        use rrrah_core::develop::Opcode;
        let root = std::path::PathBuf::from(
            std::env::var_os("RRRAH_GPR_CORPUS").expect("official corpus directory"),
        );
        for name in ["HERO9", "Fusion-back", "Fusion-front"] {
            let budget = rrrah_core::MemoryBudget::new(32 * 1024 * 1024);
            let mut request = DecodeRequest::new(root.join(format!("{name}.GPR")));
            request.memory_budget = Some(budget.clone());
            let lists = raw_development_opcodes(&request).unwrap();
            assert!(lists.list1.is_empty());
            if name == "HERO9" {
                assert_eq!(lists.list2.len(), 4);
                assert!(lists.list2.iter().all(|op| matches!(op, Opcode::GainMap { .. })));
                assert!(lists.list3.is_empty());
            } else {
                assert!(lists.list2.is_empty());
                assert_eq!(lists.list3.len(), 1);
                assert!(matches!(lists.list3[0], Opcode::WarpRectilinear { .. }));
            }
            assert_eq!(budget.used(), 0);
        }
    }
}
