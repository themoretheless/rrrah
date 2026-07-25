//! Native RAW decoding and domain adaptation.
//!
//! The production paths are clean-room Canon EOS R8 CR3 and TIFF/DNG decoders.
//! They read the full sensor mosaic and never substitute an embedded JPEG.
#![allow(clippy::missing_errors_doc, clippy::cast_precision_loss)]

mod bounded_io;
mod camtiff;
mod cr3;
#[doc(hidden)] // Exposed only for criterion micro-benchmarks (`bench_support`); not public API.
pub mod dng;
mod dng_backend;
mod native_backend;
mod native_router;
mod sniff;

/// SHA-256 digest of the resolved workspace lockfile, shared by every native
/// backend's semantic recipe. Must stay in sync with
/// `scripts/native-cr3-semantic-lock.sha256`.
pub(crate) const WORKSPACE_LOCK_DIGEST: [u8; 32] = [
    0xab, 0x83, 0x34, 0x86, 0x28, 0xb8, 0xa9, 0x5b, 0x42, 0x58, 0x3e, 0x96, 0xbe, 0xd2, 0x2a, 0xa5, 0xba,
    0x94, 0x4c, 0xda, 0xb5, 0x4f, 0xb7, 0x45, 0xbe, 0xb0, 0xa6, 0xa0, 0x98, 0xea, 0x37, 0x70,
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
        "unsupported RAW format for {path}; expected .cr3, .cr2, .nef, .arw, .orf, .pef, .rw2, .raf, .dng, .tif, or .tiff"
    )]
    UnsupportedFormat { path: PathBuf },
    #[error("native RAW backends support only image index 0, got {index}")]
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
