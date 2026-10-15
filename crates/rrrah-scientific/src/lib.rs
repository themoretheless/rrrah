//! Scientific and medical imaging array decoders (FITS, NRRD, MRC2014, DICOM Part 10).
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

pub mod dicom;
pub mod fits;
pub mod mrc;
pub mod nrrd;
pub mod sample;
pub mod scientific;

pub use fits::{FitsImage, FitsSamples, decode_fits};
pub use mrc::{MrcImage, decode_mrc};
pub use nrrd::{NrrdImage, decode_nrrd};
pub use scientific::{ScalarWindow, decode_raster_with_window};

pub mod raster {
    pub const MAX_RASTER_BYTES: u64 = 512 * 1024 * 1024;
    pub use super::ScientificDecodeError as RasterDecodeError;
}

#[derive(Debug, thiserror::Error)]
pub enum ScientificDecodeError {
    #[error(transparent)]
    Source(#[from] DecodeError),
    #[error(transparent)]
    Frame(#[from] rrrah_core::RasterError),
    #[error("raster output exceeds the {}-byte limit", raster::MAX_RASTER_BYTES)]
    OutputTooLarge,
    #[error("invalid or unsupported NRRD: {0}")]
    InvalidNrrd(&'static str),
    #[error("invalid or unsupported MRC: {0}")]
    InvalidMrc(&'static str),
    #[error("invalid or unsupported FITS: {0}")]
    InvalidFits(&'static str),
    #[error("invalid or unsupported DICOM: {0}")]
    InvalidDicom(&'static str),
    #[error("scalar window requires finite increasing bounds with a finite range")]
    InvalidScalarWindow,
    #[error("scalar window requires a supported scientific array (NRRD, MRC or FITS)")]
    ScalarWindowRequiresScientificArray,
}

pub type RasterDecodeError = ScientificDecodeError;
