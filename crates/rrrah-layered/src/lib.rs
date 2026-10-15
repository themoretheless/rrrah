//! Multi-layer editor document decoders (GIMP XCF, Aseprite, Adobe Photoshop PSD/PSB).
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

pub use rrrah_raw::{DecodeError, DecodeRequest, GenerationToken, bounded_io};
pub use rrrah_scientific::sample;

pub mod aseprite;
pub mod psd;
pub mod xcf;

pub use aseprite::{AsepriteImage, AsepriteTag, decode_aseprite};
pub use xcf::{
    XcfChannel, XcfCompositionAttributes, XcfDecodedLayer, XcfFlattenedImage, XcfHeader, XcfHeaderError,
    XcfLayer, XcfLayerAttributes, XcfLayerGroup, XcfLayerNode, XcfLevel, XcfObjectTables, XcfOffsets,
    XcfPixelError, XcfProperties, XcfProperty, composite_xcf_normal_premultiplied_rgba32f,
    composite_xcf_normal_rgba8, composite_xcf_normal_rgba32f, decode_xcf_layer, decode_xcf_level,
    decode_xcf_samples, decode_xcf_tile, expand_xcf_legacy_u8_pixels, flatten_xcf_legacy_normal,
    parse_xcf_channel, parse_xcf_header, parse_xcf_hierarchy, parse_xcf_layer, parse_xcf_layer_tree,
    parse_xcf_object_tables, prepare_xcf_legacy_rgba8, xcf_icc_profile, xcf_palette, xcf_properties,
};

pub mod raster {
    pub const MAX_RASTER_BYTES: u64 = 512 * 1024 * 1024;
    pub use super::LayeredDecodeError as RasterDecodeError;
}

#[derive(Debug, thiserror::Error)]
pub enum LayeredDecodeError {
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error(transparent)]
    Frame(#[from] rrrah_core::RasterError),
    #[error("raster output exceeds the {}-byte limit", raster::MAX_RASTER_BYTES)]
    OutputTooLarge,
    #[error("invalid or unsupported XCF: {0}")]
    InvalidXcf(String),
    #[error("invalid or unsupported Aseprite: {0}")]
    InvalidAseprite(&'static str),
    #[error("invalid or unsupported Photoshop composition: {0}")]
    InvalidPsd(&'static str),
}

pub type RasterDecodeError = LayeredDecodeError;
