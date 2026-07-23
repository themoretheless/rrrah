//! Native RAW decoding and domain adaptation.
//!
//! The production path is a clean-room Canon EOS R8 CR3 decoder. It reads the
//! full sensor mosaic and never substitutes an embedded JPEG.
#![allow(clippy::missing_errors_doc, clippy::cast_precision_loss)]

mod cr3;
mod native_backend;

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

pub use native_backend::{NATIVE_CR3_BACKEND_ID, NATIVE_EOS_R8_MOSAIC_CONTRACT_1, NativeCr3Decoder};
use rrrah_core::{DecodedMosaic, FrameError, MosaicRecipeManifest};
use thiserror::Error;

pub trait RawDecoder: Send + Sync {
    fn mosaic_recipe(&self) -> MosaicRecipeManifest;
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
    pub adapt: AdaptTimings,
    pub adapt_metadata: Duration,
    pub total: Duration,
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
    #[error("native EOS R8 CR3 supports only image index 0, got {index}")]
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
    NativeCr3Decoder.decode(&DecodeRequest::new(path.as_ref()))
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
    fn stale_request_is_rejected_without_io() {
        let generation = Arc::new(AtomicU64::new(12));
        let request = DecodeRequest {
            path: "does-not-exist.CR3".into(),
            image_index: 0,
            cancellation: Some(GenerationToken::new(generation, 11)),
        };
        assert!(matches!(request.check_cancelled(), Err(DecodeError::Cancelled)));
    }
}
