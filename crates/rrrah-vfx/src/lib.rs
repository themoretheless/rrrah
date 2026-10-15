//! Cinema, VFX, post-production, and G-buffer image decoders (RPF, RLA, DPX, Cineon, SGI, Softimage PIC, Alias PIX).
#![allow(
    clippy::pedantic,
    clippy::chunks_exact_to_as_chunks,
    clippy::collapsible_if,
    clippy::missing_errors_doc,
    clippy::cast_precision_loss,
    clippy::manual_is_multiple_of,
    clippy::manual_checked_ops,
    clippy::too_many_arguments
)]

pub use rrrah_raw::{
    DecodeError, DecodeRequest, GenerationToken, RlaAlphaMode, RlaColorSpace, RlaFloatByteOrder,
    RpfRasterInterpretation, bounded_io,
};

pub mod cineon;
pub mod dpx;
pub mod pix;
pub mod rla;
pub mod rpf;
pub mod sgi;
pub mod softimage;

pub use rla::{decode_rla_float_with_interpretation, decode_rla_with_interpretation};
pub use rpf::{
    RPF_GBUFFER_SAMPLE_BYTES, RpfAspectError, RpfDecodeLimits, RpfDecodedImage, RpfDecodedLayers,
    RpfDisplayError, RpfFileError, RpfFileView, RpfHeader, RpfImageAllocationPlan, RpfImageDecodeError,
    RpfImageSnapshot, RpfInspectError, RpfInspection, RpfLayerCoordinateError, RpfLayerDecodeError,
    RpfLayerError, RpfLayerRecords, RpfNameError, RpfNodeNames, RpfPlanError, RpfPlaneError,
    RpfRasterReadError, RpfReadError, RpfRenderInfo, RpfRenderInfoError, RpfRowError, RpfRowView,
    RpfSnapshotError, RpfWindow, decode_rpf_byte_planes, decode_rpf_color_channel,
    decode_rpf_file_raster_with_budget, decode_rpf_file_with_budget, inspect_rpf, inspect_rpf_file,
    inspect_rpf_layer_records, inspect_rpf_node_names, inspect_rpf_render_info, inspect_rpf_row,
};

pub mod raster {
    pub const MAX_RASTER_BYTES: u64 = 512 * 1024 * 1024;
    pub use super::VfxDecodeError as RasterDecodeError;
}

#[derive(Debug, thiserror::Error)]
pub enum VfxDecodeError {
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error(transparent)]
    Frame(#[from] rrrah_core::RasterError),
    #[error("raster output exceeds the {}-byte limit", raster::MAX_RASTER_BYTES)]
    OutputTooLarge,
    #[error("RLA matte requires explicit alpha/color interpretation; use decode_rla_with_interpretation")]
    RlaInterpretationRequired,
    #[error("invalid or unsupported Wavefront RLA: {0}")]
    InvalidRla(&'static str),
    #[error("invalid or unsupported Softimage PIC: {0}")]
    InvalidSoftimage(&'static str),
    #[error("invalid or unsupported Cineon: {0}")]
    InvalidCineon(&'static str),
    #[error("invalid or unsupported DPX: {0}")]
    InvalidDpx(&'static str),
    #[error("invalid Alias PIX: {0}")]
    InvalidPix(&'static str),
    #[error("invalid or unsupported SGI image: {0}")]
    InvalidSgi(&'static str),
    #[error("RPF requires explicit channel, alpha and color interpretation")]
    RpfInterpretationRequired,
    #[error("RPF raster import failed: {0:?}")]
    Rpf(RpfRasterReadError),
}

pub type RasterDecodeError = VfxDecodeError;
