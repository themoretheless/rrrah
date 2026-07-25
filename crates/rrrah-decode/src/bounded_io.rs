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
pub(crate) fn read_bounded(request: &DecodeRequest) -> Result<Vec<u8>, DecodeError> {
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
