//! Vector and metafile document decoders and rasterizers (EPS, PDF, WMF, EMF, PICT, SVG).
#![allow(
    clippy::pedantic,
    clippy::chunks_exact_to_as_chunks,
    clippy::collapsible_if,
    clippy::missing_errors_doc,
    clippy::cast_precision_loss,
    clippy::manual_is_multiple_of,
    clippy::manual_checked_ops,
    clippy::too_many_arguments,
    clippy::needless_range_loop,
    clippy::collapsible_match
)]

pub use resvg::usvg::roxmltree;
pub use rrrah_raw::{DecodeError, DecodeRequest, GenerationToken, bounded_io};

pub mod emf;
pub mod eps;
pub mod eps_bounds;
pub mod eps_document;
pub mod eps_fill;
pub mod eps_flatten;
pub mod eps_graphics;
pub mod eps_numbers;
pub mod eps_program;
pub mod eps_raster;
pub mod eps_strings;
pub mod eps_stroke;
pub mod eps_stroke_prepare;
pub mod eps_tokens;
pub mod eps_vm;
pub mod pdf;
#[cfg(test)]
mod pdf_device_default_tests;
#[cfg(test)]
mod pdf_patch_tests;
pub mod pict;
pub mod png_color;
pub mod svg;
pub mod wmf;

pub use eps::{EpsInspectError, EpsSource, inspect_eps_source};
pub use eps_bounds::{EpsBounds, EpsBoundsError, inspect_eps_bounds};
pub use eps_document::{EpsDocumentError, EpsDocumentLimits, decode_eps_document, decode_eps_file_document};
pub use eps_fill::{EpsFillError, EpsFillLimits, fill_eps_path};
pub use eps_flatten::{EpsFlattenError, EpsFlattenLimits, flatten_eps_path};
pub use eps_graphics::{
    EpsDeviceColor, EpsGraphics, EpsGraphicsError, EpsGraphicsLimit, EpsGraphicsLimits, EpsMatrix, EpsPaint,
    EpsPaintKind, EpsPathSegment, EpsPoint, EpsStrokeStyle, EpsVectorScene,
};
pub use eps_numbers::{EpsNumber, EpsNumberError, parse_eps_number};
pub use eps_program::{EpsCompileError, EpsCompileLimits, EpsInstruction, EpsProgram, compile_eps_program};
pub use eps_raster::{
    EpsRasterColorPolicy, EpsRasterError, EpsRasterLimits, rasterize_eps_fills, rasterize_eps_scene,
};
pub use eps_strings::{EpsStringError, decode_eps_string};
pub use eps_stroke::{EpsStrokeError, outline_eps_join, outline_eps_polyline, outline_eps_segment};
pub use eps_stroke_prepare::{
    EpsPreparedStroke, EpsStrokeAssemblyLimits, EpsStrokePrepareError, outline_eps_prepared_stroke,
    prepare_eps_stroke,
};
pub use eps_tokens::{EpsToken, EpsTokenError, EpsTokens};
pub use eps_vm::{
    EpsEvaluation, EpsOperator, EpsValue, EpsVectorExecution, EpsVmError, EpsVmLimit, EpsVmLimits,
    evaluate_eps_program, evaluate_eps_vectors,
};

pub mod raster {
    pub const MAX_RASTER_BYTES: u64 = 512 * 1024 * 1024;
    pub use super::VectorDecodeError as RasterDecodeError;
}

#[derive(Debug, thiserror::Error)]
pub enum VectorDecodeError {
    #[error(transparent)]
    Eps(#[from] EpsDocumentError),
    #[error("invalid or unsupported PICT: {0}")]
    InvalidPict(String),
    #[error("invalid or unsupported PDF: {0}")]
    InvalidPdf(&'static str),
    #[error("invalid or unsupported EMF: {0}")]
    InvalidEmf(&'static str),
    #[error("invalid or unsupported WMF: {0}")]
    InvalidWmf(&'static str),
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error(transparent)]
    Codec(#[from] image::ImageError),
    #[error(transparent)]
    Frame(#[from] rrrah_core::RasterError),
    #[error("raster output exceeds the {}-byte limit", raster::MAX_RASTER_BYTES)]
    OutputTooLarge,
    #[error("invalid or unsupported SVG: {0}")]
    InvalidSvg(String),
    #[error("invalid PNG color metadata: {0}")]
    InvalidPngColor(&'static str),
    #[error("invalid PNG physical pixel dimensions")]
    InvalidPngPhysicalDimensions,
}

pub type RasterDecodeError = VectorDecodeError;

/// Identity of the immutable system-font snapshot used by SVG text rendering.
pub fn svg_font_resource_identity() -> Result<[u8; 32], VectorDecodeError> {
    svg::font_identity()
}
