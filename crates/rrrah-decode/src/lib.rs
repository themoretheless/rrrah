//! Native RAW decoding and domain adaptation.
//!
//! The production paths are clean-room Canon EOS R8 CR3 and TIFF/DNG decoders.
//! They read the full sensor mosaic and never substitute an embedded JPEG.
#![allow(clippy::missing_errors_doc, clippy::cast_precision_loss)]

mod optio_s4;
pub use optio_s4::{
    OPTIO_S4_SENSOR_SIZE, OptioS4SensorError, read_optio_s4_sensor_with_budget, unpack_optio_s4_sensor,
};

mod bay;
mod jpeg_cmyk;
pub use bay::{
    BayReadError, BaySensorError, BaySensorLayout, read_bay_sensor_with_budget, unpack_bay_sensor,
};

mod ciff;
mod crw;
mod crw_entropy;
mod xcf;
pub use crw::{NativeCrwDecoder, decode_crw_10d, decode_crw_10d_with_tables};
pub use xcf::XcfCompositionAttributes;
pub use xcf::XcfLayerAttributes;
pub use xcf::XcfLayerGroup;
pub use xcf::composite_xcf_normal_premultiplied_rgba32f;
pub use xcf::composite_xcf_normal_rgba8;
pub use xcf::composite_xcf_normal_rgba32f;
pub use xcf::decode_xcf_samples;
pub use xcf::decode_xcf_tile;
pub use xcf::expand_xcf_legacy_u8_pixels;
pub use xcf::prepare_xcf_legacy_rgba8;
pub use xcf::xcf_icc_profile;
pub use xcf::xcf_palette;
pub use xcf::{XcfChannel, parse_xcf_channel};
pub use xcf::{XcfDecodedLayer, decode_xcf_layer};
pub use xcf::{XcfFlattenedImage, flatten_xcf_legacy_normal};
pub use xcf::{XcfHeader, XcfHeaderError, XcfProperties, XcfProperty, parse_xcf_header, xcf_properties};
pub use xcf::{XcfLayer, parse_xcf_layer};
pub use xcf::{XcfLayerNode, parse_xcf_layer_tree};
pub use xcf::{XcfLevel, parse_xcf_hierarchy};
pub use xcf::{XcfObjectTables, XcfOffsets, parse_xcf_object_tables};
pub use xcf::{XcfPixelError, decode_xcf_level};

mod off_swap;
pub use off_swap::{OffSwapError, off_swap_payload_len, read_off_swap_payload, write_off_swap_payload};
mod off;
pub use off::{OffColor, OffDecodeError, OffEncoding, OffFace, OffMesh, decode_off};
mod ply_swap;
pub use ply_swap::{PlySwapError, ply_swap_payload_len, read_ply_swap_payload, write_ply_swap_payload};
mod ply;
pub use ply::{
    PlyDecodeError, PlyElement, PlyEncoding, PlyMesh, PlyProperty, PlyScalars, PlyType, PlyValues, decode_ply,
};
mod model;
pub use model::{DecodedModel, ModelBuffer, ModelDecodeError, decode_model, is_supported_model_path};
mod obj_swap;
pub use obj_swap::{ObjSwapError, obj_swap_payload_len, read_obj_swap_payload, write_obj_swap_payload};
mod obj;
pub use obj::{ObjCorner, ObjDecodeError, ObjFace, ObjMesh, ObjTriangle, decode_obj};
mod stl_swap;
pub use stl_swap::{StlSwapError, read_stl_swap_payload, stl_swap_payload_len, write_stl_swap_payload};
mod stl;
pub use stl::{StlDecodeError, StlFacet, StlMesh, decode_stl};

mod eip;
mod x3f;
pub use eip::{
    EipEntry, EipError, EipManifest, EipRaw, EipSensor, decode_eip_file_sensor, decode_eip_sensor,
    inspect_eip, read_eip_raw, read_eip_raw_with_budget,
};
pub use x3f::{
    X3fCamf, X3fCamfEntry, X3fCamfMatrix, X3fColorTransform, X3fEntry, X3fError, X3fImageInfo, X3fInventory,
    X3fLegacyChannels, X3fLegacyHuffman, X3fNoiseCurve, X3fProperties, crop_x3f_channels, decode_x3f_legacy,
    decode_x3f_legacy_auto, inspect_x3f, linearize_x3f_highlights, linearize_x3f_highlights_from_camf,
    repair_x3f_bad_pixels, repair_x3f_bad_pixels_from_camf, rotate_x3f_linear, sharpen_x3f_red,
    smooth_x3f_black_rows, smooth_x3f_chroma, smooth_x3f_hues, smooth_x3f_hues_wide, transform_x3f_pixels,
    visit_x3f_camf, x3f_block_enabled, x3f_chroma_guide, x3f_color_transform_auto, x3f_illuminant_matrix,
    x3f_linear_output, x3f_linear_raster, x3f_neutral_response, x3f_noise_curves_auto, x3f_wb_correction,
};

mod animation;
mod aseprite;
pub use aseprite::{AsepriteImage, AsepriteTag, decode_aseprite};
mod jpegls;
mod jpegxr;
mod sample;
pub use jpegls::{JpegLsImage, decode_jpegls};
mod jng;
mod webp_animation;
pub use webp_animation::{WebpImage, decode_webp_animation};
mod gif_animation;
pub use gif_animation::{GifImage, decode_gif};
mod basis;
mod emf;
mod mng;
mod pdf;
mod pict;
mod sti;
mod wmf;
pub use mng::{MngImage, decode_mng};
mod apng;
mod astc;
pub use apng::{ApngImage, decode_apng};
mod avif_color;
mod bounded_io;
mod camtiff;
pub use camtiff::{decode_mef_zd_sensor, decode_mef_zd_sensor_file};
pub use camtiff::erf::{EpsonMakerMetadata, read_erf_maker_metadata};
mod mrw;
pub use mrw::{MrwLayout, NativeMrwDecoder};
mod cineon;
mod cr3;
mod cursor;
mod dcx;
mod dds;
mod dicom;
#[doc(hidden)] // Exposed only for criterion micro-benchmarks (`bench_support`); not public API.
pub mod dng;
mod dng_backend;
mod dpx;
mod exr_color;
mod fits;
mod heif;
mod iff;
mod image_router;
mod jp2;
mod jxl;
mod kra;
mod ktx;
mod ktx2;
mod macpaint;
mod mrc;
mod native_backend;
mod native_router;
mod nrrd;
mod ora;
mod pcx;
mod pfm;
mod pix;
mod pkm;
mod png_color;
mod psd;
mod pvr;
mod raster;
mod raster_color;
#[cfg(test)]
mod raster_corpus;
mod rla;
mod rpf;
mod scientific;
mod sgi;
mod sniff;
mod softimage;
mod sun;
mod svg;
mod wad;
mod wal;
mod xbm;
mod xpm;

/// SHA-256 digest of the resolved workspace lockfile, shared by every native
/// backend's semantic recipe. Must stay in sync with
/// `scripts/native-cr3-semantic-lock.sha256`.
pub(crate) const WORKSPACE_LOCK_DIGEST: [u8; 32] = [
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

pub trait RawDecoder: Send + Sync {
    fn mosaic_recipe(&self, request: &DecodeRequest) -> Result<MosaicRecipeManifest, DecodeError>;
    fn decode(&self, request: &DecodeRequest) -> Result<DecodeOutput, DecodeError>;
}

#[derive(Debug, Clone)]
pub struct DecodeRequest {
    pub path: PathBuf,
    pub image_index: usize,
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
    /// unspecified encoded samples are never overridden. Does not apply a transfer curve.
    pub assume_untagged_linear_srgb: bool,
    pub cancellation: Option<GenerationToken>,
    /// Optional admission for supported native decoder output allocations.
    pub memory_budget: Option<rrrah_core::MemoryBudget>,
}

impl DecodeRequest {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            image_index: 0,
            rla_alpha_mode: None,
            rla_float_byte_order: None,
            rla_color_space: None,
            pq_reference_white_nits: None,
            hlg_display: None,
            assume_untagged_srgb: false,
            assume_untagged_linear_srgb: false,
            cancellation: None,
            memory_budget: None,
        }
    }

    pub(crate) fn qualify_linear_color(
        &self,
        color: rrrah_core::RasterColorSpace,
    ) -> rrrah_core::RasterColorSpace {
        if self.assume_untagged_linear_srgb && color == rrrah_core::RasterColorSpace::LinearRgbUnspecified {
            rrrah_core::RasterColorSpace::LinearSrgb
        } else {
            color
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

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };

    use super::{DecodeError, DecodeRequest, GenerationToken};

    #[test]
    fn generation_token_cancels_stale_work() {
        let generation = Arc::new(AtomicU64::new(7));
        let token = GenerationToken::new(Arc::clone(&generation), 7);
        assert!(!token.is_cancelled());
        generation.store(8, Ordering::Release);
        assert!(token.is_cancelled());
    }
    #[test]
    fn combined_generation_tokens_cancel_from_either_owner_and_deduplicate() {
        for changed in 0..3 {
            let owners: Vec<_> = (0..3).map(|_| Arc::new(AtomicU64::new(1))).collect();
            let tokens: Vec<_> = owners
                .iter()
                .map(|owner| GenerationToken::new(owner.clone(), 1))
                .collect();
            let mut combined = tokens[0].combine(&tokens[1]).combine(&tokens[2]);
            for _ in 0..100 {
                combined = combined.combine(&combined);
            }
            assert_eq!(combined.additional.len(), 2);
            assert!(!combined.is_cancelled());
            let retained = combined.clone();
            owners[changed].store(2, Ordering::Release);
            assert!(combined.is_cancelled());
            assert!(retained.is_cancelled());
            assert!(tokens[changed].is_cancelled());
        }
        let owner = Arc::new(AtomicU64::new(2));
        let current = GenerationToken::new(owner.clone(), 2);
        let obsolete = GenerationToken::new(owner, 1);
        assert!(current.combine(&obsolete).is_cancelled());
    }

    #[test]
    fn stale_request_is_rejected_without_io() {
        let generation = Arc::new(AtomicU64::new(12));
        let request = DecodeRequest {
            rla_alpha_mode: None,
            rla_float_byte_order: None,
            rla_color_space: None,
            pq_reference_white_nits: None,
            hlg_display: None,
            path: "does-not-exist.CR3".into(),
            image_index: 0,
            assume_untagged_srgb: false,
            assume_untagged_linear_srgb: false,
            cancellation: Some(GenerationToken::new(generation, 11)),
            memory_budget: None,
        };
        assert!(matches!(request.check_cancelled(), Err(DecodeError::Cancelled)));
    }
}

pub use image_router::{
    DecodedImage, ImageSourceKind, decode_image, decode_image_file, image_source_kind,
    is_supported_image_path, is_supported_raw_path,
};
pub use mrc::{MrcImage, decode_mrc};
pub use nrrd::{NrrdImage, decode_nrrd};
pub use raster::{RasterDecodeError, decode_raster, decode_raster_file};
pub use raster_color::{
    RasterColorError, prepare_raster_for_display, prepare_raster_for_display_with_budget,
    prepare_raster_for_display_with_budget_and_cancel,
};
pub use rla::{
    RlaAlphaMode, RlaColorSpace, RlaFloatByteOrder, decode_rla_float_with_interpretation,
    decode_rla_with_interpretation,
};
pub use rpf::{
    RPF_GBUFFER_SAMPLE_BYTES, RpfAspectError, RpfDecodeLimits, RpfDecodedImage, RpfDecodedLayers,
    RpfDisplayError, RpfFileError, RpfFileView, RpfHeader, RpfImageAllocationPlan, RpfImageDecodeError,
    RpfImageSnapshot, RpfInspectError, RpfInspection, RpfLayerCoordinateError, RpfLayerDecodeError,
    RpfLayerError, RpfLayerRecords, RpfNameError, RpfNodeNames, RpfPlanError, RpfPlaneError,
    RpfRasterInterpretation, RpfRasterReadError, RpfReadError, RpfRenderInfo, RpfRenderInfoError,
    RpfRowError, RpfRowView, RpfSnapshotError, RpfWindow, decode_rpf_byte_planes, decode_rpf_color_channel,
    decode_rpf_file_raster_with_budget, decode_rpf_file_with_budget, inspect_rpf, inspect_rpf_file,
    inspect_rpf_layer_records, inspect_rpf_node_names, inspect_rpf_render_info, inspect_rpf_row,
};
pub use scientific::{ScalarWindow, decode_raster_with_window};
pub use wal::{decode_wal_with_palette, wal_palette_path};

pub use fits::{FitsImage, FitsSamples, decode_fits};

pub use pvr::{PvrImage, PvrMetadata, decode_pvr};

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
mod linear_assumption_tests {
    use super::DecodeRequest;
    use rrrah_core::RasterColorSpace;
    #[test]
    fn explicit_linear_assumption_only_qualifies_unspecified_linear_primaries() {
        let mut request = DecodeRequest::new("unused.pfm");
        assert!(!request.assume_untagged_linear_srgb);
        request.assume_untagged_srgb = true;
        assert_eq!(
            request.qualify_linear_color(RasterColorSpace::LinearRgbUnspecified),
            RasterColorSpace::LinearRgbUnspecified
        );
        request.assume_untagged_linear_srgb = true;
        assert_eq!(
            request.qualify_linear_color(RasterColorSpace::LinearRgbUnspecified),
            RasterColorSpace::LinearSrgb
        );
        for color in [
            RasterColorSpace::Srgb,
            RasterColorSpace::AssumedSrgb,
            RasterColorSpace::LinearSrgb,
            RasterColorSpace::Unspecified,
            RasterColorSpace::Icc(vec![1, 2, 3]),
        ] {
            assert_eq!(request.qualify_linear_color(color.clone()), color);
        }
    }
}

/// Identity of the immutable system-font snapshot used by SVG text rendering.
///
/// # Errors
/// Returns an error if a discovered font cannot be retained.
pub fn svg_font_resource_identity() -> Result<[u8; 32], RasterDecodeError> {
    svg::font_identity()
}

/// Identity of the currently available HEVC decoder inventory, ordered by priority.
/// This is capability/version identity, not a digest of native library bytes.
///
/// # Errors
/// Returns initialization errors or an excessive decoder inventory error.
pub fn hevc_decoder_resource_identity() -> Result<[u8; 32], RasterDecodeError> {
    heif::decoder_identity()
}

/// External files consumed by native raster dispatch for this request/header.
/// Extension hints for otherwise recognized raster data do not add WAL dependencies.
pub fn raster_external_dependencies(request: &DecodeRequest, header: &[u8]) -> Vec<PathBuf> {
    if image::guess_format(header).is_err() {
        wal::wal_palette_path(&request.path).into_iter().collect()
    } else {
        Vec::new()
    }
}

/// Native VC-5 framing and managed sensor reconstruction; DNG admission is separate.
pub mod vc5;

#[cfg(test)]
mod gpr_opcode_tests {
    #[test]
    #[ignore = "requires pinned official GoPro sample corpus"]
    fn official_gpr_corrections_are_not_silently_omitted() {
        let root = std::path::PathBuf::from(std::env::var("RRRAH_GPR_CORPUS").unwrap());
        for name in ["HERO9", "HERO5", "HERO6", "HERO7", "Fusion-back", "Fusion-front"] {
            let budget = rrrah_core::MemoryBudget::new(16 * 1024 * 1024);
            let mut request = super::DecodeRequest::new(root.join(format!("{name}.GPR")));
            request.memory_budget = Some(budget.clone());
            let lists = super::raw_development_opcodes(&request).unwrap();
            assert!(lists.list1.is_empty());
            if name.starts_with("Fusion") {
                assert_eq!(lists.list3.len(), 1);
            } else {
                assert!(!lists.list2.is_empty());
            }
            eprintln!(
                "{name}: {} / {} / {} opcodes",
                lists.list1.len(),
                lists.list2.len(),
                lists.list3.len()
            );
            assert_eq!(budget.used(), 0);
        }
    }
}

#[cfg(test)]
mod opcode_geometry_serde_compatibility_tests {
    #[test]
    fn old_lists_remain_square_and_new_aspect_roundtrips() {
        let legacy = r#"{"list1":[],"list2":[],"list3":[]}"#;
        let old: rrrah_core::develop::OpcodeLists = serde_json::from_str(legacy).unwrap();
        assert_eq!(old.pixel_aspect, None);
        assert_eq!(serde_json::to_string(&old).unwrap(), legacy);
        let mut asymmetric = old;
        asymmetric.pixel_aspect = Some(1.5);
        let bytes = serde_json::to_vec(&asymmetric).unwrap();
        let restored: rrrah_core::develop::OpcodeLists = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored, asymmetric);
        assert_eq!(restored.pixel_aspect, Some(1.5));
    }
}

#[cfg(test)]
mod pdf_patch_tests;

#[cfg(test)]
mod pdf_device_default_tests;

mod eps;
pub use eps::{EpsInspectError, EpsSource, inspect_eps_source};

mod eps_tokens;
pub use eps_tokens::{EpsToken, EpsTokenError, EpsTokens};

mod eps_strings;
pub use eps_strings::{EpsStringError, decode_eps_string};

mod eps_numbers;
pub use eps_numbers::{EpsNumber, EpsNumberError, parse_eps_number};

mod eps_program;
pub use eps_program::{EpsCompileError, EpsCompileLimits, EpsInstruction, EpsProgram, compile_eps_program};

mod eps_vm;
pub use eps_vm::{EpsEvaluation, EpsOperator, EpsValue, EpsVmError, EpsVmLimit, EpsVmLimits, evaluate_eps_program};

mod eps_graphics;
pub use eps_graphics::{EpsDeviceColor, EpsGraphics, EpsGraphicsError, EpsGraphicsLimit, EpsGraphicsLimits, EpsMatrix, EpsPaint, EpsPaintKind, EpsPathSegment, EpsPoint, EpsStrokeStyle, EpsVectorScene};

pub use eps_vm::{EpsVectorExecution, evaluate_eps_vectors};
