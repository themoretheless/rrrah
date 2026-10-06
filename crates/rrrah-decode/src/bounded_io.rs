//! Bounded whole-file reads shared by every native RAW backend.
//!
//! Every backend materializes the complete source file before parsing, so the
//! read is capped at [`MAX_INPUT_BYTES`] and the allocation is reserved with
//! `try_reserve_exact` to fail cleanly instead of aborting on over-commit.

use std::{fs::File, io::Read};

use crate::{DecodeError, DecodeRequest};

/// Hard cap on a native RAW source. Anything larger is rejected before
/// allocation so a hostile or corrupted file cannot exhaust memory.
pub(crate) const MAX_INPUT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Reads the whole request path into memory, enforcing [`MAX_INPUT_BYTES`]
/// both on the declared length and on the bytes actually read.
fn read_legacy(request: &DecodeRequest) -> Result<Vec<u8>, DecodeError> {
    let mut file = File::open(&request.path).map_err(|source| DecodeError::Io {
        path: request.path.clone(),
        source,
    })?;
    let declared = file
        .metadata()
        .map_err(|source| DecodeError::Io {
            path: request.path.clone(),
            source,
        })?
        .len();
    if declared > MAX_INPUT_BYTES {
        return Err(DecodeError::InputTooLarge {
            path: request.path.clone(),
            actual: declared,
            limit: MAX_INPUT_BYTES,
        });
    }
    let capacity = usize::try_from(declared).map_err(|_| DecodeError::InputTooLarge {
        path: request.path.clone(),
        actual: declared,
        limit: MAX_INPUT_BYTES,
    })?;
    let mut data = Vec::new();
    data.try_reserve_exact(capacity)
        .map_err(|_| DecodeError::InputAllocation { bytes: capacity })?;
    file.by_ref()
        .take(MAX_INPUT_BYTES.saturating_add(1))
        .read_to_end(&mut data)
        .map_err(|source| DecodeError::Io {
            path: request.path.clone(),
            source,
        })?;
    let actual = u64::try_from(data.len()).unwrap_or(u64::MAX);
    if actual > MAX_INPUT_BYTES {
        return Err(DecodeError::InputTooLarge {
            path: request.path.clone(),
            actual,
            limit: MAX_INPUT_BYTES,
        });
    }
    Ok(data)
}

/// Budgeted immutable input for native decoders with managed source ownership.
/// A changed source length is rejected rather than growing outside admission.
pub(crate) fn read_bounded(request: &DecodeRequest) -> Result<SourceBuffer, DecodeError> {
    let Some(budget) = &request.memory_budget else {
        return read_legacy(request).map(SourceBuffer::Legacy);
    };
    request.check_cancelled()?;
    let io_error = |source| DecodeError::Io {
        path: request.path.clone(),
        source,
    };
    let mut file = File::open(&request.path).map_err(io_error)?;
    let declared = file.metadata().map_err(io_error)?.len();
    if declared > MAX_INPUT_BYTES {
        return Err(DecodeError::InputTooLarge {
            path: request.path.clone(),
            actual: declared,
            limit: MAX_INPUT_BYTES,
        });
    }
    let capacity = usize::try_from(declared).map_err(|_| DecodeError::DimensionOverflow)?;
    let mut data = budget.try_buffer(capacity, 0_u8)?;
    for chunk in data.chunks_mut(64 * 1024) {
        request.check_cancelled()?;
        file.read_exact(chunk).map_err(io_error)?;
    }
    request.check_cancelled()?;
    let mut extra = [0_u8; 1];
    if file.read(&mut extra).map_err(io_error)? != 0 {
        return Err(io_error(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "source grew during budgeted read",
        )));
    }
    Ok(SourceBuffer::Managed(data))
}

/// Mutable during parsing (e.g. CUR normalization), with ownership-bound admission.
pub(crate) enum SourceBuffer {
    Legacy(Vec<u8>),
    Managed(rrrah_core::MutableBuffer<u8>),
}
impl AsRef<[u8]> for SourceBuffer {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Legacy(v) => v,
            Self::Managed(v) => v,
        }
    }
}
impl AsMut<[u8]> for SourceBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        match self {
            Self::Legacy(v) => v,
            Self::Managed(v) => v,
        }
    }
}
impl std::ops::Deref for SourceBuffer {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_ref()
    }
}
/// Immutable managed input for RAW parsing.
pub(crate) fn read_managed(request: &DecodeRequest) -> Result<rrrah_core::PixelBuffer<u8>, DecodeError> {
    Ok(match read_bounded(request)? {
        SourceBuffer::Legacy(bytes) => std::sync::Arc::new(bytes).into(),
        SourceBuffer::Managed(bytes) => bytes.freeze().into(),
    })
}
