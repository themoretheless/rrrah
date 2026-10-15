//! Animated and multi-frame container decoders (APNG, WebP animation, GIF animation, JNG) and display color preparation.
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
pub use rrrah_vector::png_color;

pub mod animation;
pub mod apng;
pub mod gif_animation;
pub mod jng;
pub mod raster_color;
pub mod webp_animation;

pub use apng::{ApngImage, decode_apng};
pub use gif_animation::{GifImage, decode_gif};
pub use raster_color::{
    RasterColorError, prepare_raster_for_display, prepare_raster_for_display_with_budget,
    prepare_raster_for_display_with_budget_and_cancel,
};
pub use webp_animation::{WebpImage, decode_webp_animation};

pub mod raster {
    pub const MAX_RASTER_BYTES: u64 = 512 * 1024 * 1024;
    pub use super::AnimationDecodeError as RasterDecodeError;
}

#[derive(Debug, thiserror::Error)]
pub enum AnimationDecodeError {
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error(transparent)]
    Codec(#[from] image::ImageError),
    #[error(transparent)]
    Frame(#[from] rrrah_core::RasterError),
    #[error(transparent)]
    Color(#[from] RasterColorError),
    #[error("raster output exceeds the {}-byte limit", raster::MAX_RASTER_BYTES)]
    OutputTooLarge,
    #[error("invalid or unsupported JNG: {0}")]
    InvalidJng(&'static str),
    #[error("invalid or unsupported WebP animation: {0}")]
    InvalidWebpAnimation(&'static str),
    #[error("invalid or unsupported GIF: {0}")]
    InvalidGif(&'static str),
    #[error("invalid or unsupported APNG: {0}")]
    InvalidApng(&'static str),
    #[error("invalid PNG color metadata: {0}")]
    InvalidPngColor(&'static str),
    #[error("invalid PNG physical pixel dimensions")]
    InvalidPngPhysicalDimensions,
}

impl From<rrrah_vector::VectorDecodeError> for AnimationDecodeError {
    fn from(error: rrrah_vector::VectorDecodeError) -> Self {
        match error {
            rrrah_vector::VectorDecodeError::Source(e) => Self::Source(e),
            rrrah_vector::VectorDecodeError::Codec(e) => Self::Codec(e),
            rrrah_vector::VectorDecodeError::Frame(e) => Self::Frame(e),
            rrrah_vector::VectorDecodeError::OutputTooLarge => Self::OutputTooLarge,
            rrrah_vector::VectorDecodeError::InvalidPngColor(e) => Self::InvalidPngColor(e),
            rrrah_vector::VectorDecodeError::InvalidPngPhysicalDimensions => {
                Self::InvalidPngPhysicalDimensions
            }
            _ => Self::InvalidPngColor("unexpected vector error in PNG color metadata"),
        }
    }
}

pub type RasterDecodeError = AnimationDecodeError;
