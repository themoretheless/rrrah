//! Bounded whole-file reads shared by every native RAW backend.
//!
//! Every backend materializes the complete source file before parsing, so the
//! read is capped at [`MAX_INPUT_BYTES`] and the allocation is reserved with
//! `try_reserve_exact` to fail cleanly instead of aborting on over-commit.

use std::{fs::File, io::Read};

use crate::{DecodeError, DecodeRequest};

/// Retry interrupted reads while observing cancellation between every I/O call.
pub(crate) fn read_checked(
    reader: &mut impl Read,
    request: &DecodeRequest,
    output: &mut [u8],
) -> Result<usize, DecodeError> {
    loop {
        request.check_cancelled()?;
        match reader.read(output) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => {
                request.check_cancelled()?;
                return result.map_err(|source| DecodeError::Io {
                    path: request.path.clone(),
                    source,
                });
            }
        }
    }
}

pub(crate) fn read_exact_checked(
    reader: &mut impl Read,
    request: &DecodeRequest,
    mut output: &mut [u8],
) -> Result<(), DecodeError> {
    while !output.is_empty() {
        let count = read_checked(reader, request, output)?;
        if count == 0 {
            return Err(DecodeError::Io {
                path: request.path.clone(),
                source: std::io::ErrorKind::UnexpectedEof.into(),
            });
        }
        output = &mut output[count..];
    }
    request.check_cancelled()
}

/// Hard cap on a native RAW source. Anything larger is rejected before
/// allocation so a hostile or corrupted file cannot exhaust memory.
pub const MAX_INPUT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Reads the whole request path into memory, enforcing [`MAX_INPUT_BYTES`]
/// both on the declared length and on the bytes actually read.
fn read_legacy(request: &DecodeRequest) -> Result<Vec<u8>, DecodeError> {
    request.check_cancelled()?;
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
    read_legacy_stream(&mut file, request, capacity, MAX_INPUT_BYTES)
}

fn read_legacy_stream(
    reader: &mut impl Read,
    request: &DecodeRequest,
    capacity: usize,
    limit: u64,
) -> Result<Vec<u8>, DecodeError> {
    request.check_cancelled()?;
    let mut data = Vec::new();
    data.try_reserve_exact(capacity)
        .map_err(|_| DecodeError::InputAllocation { bytes: capacity })?;
    let mut scratch = [0u8; 64 * 1024];
    loop {
        let probe =
            (limit.saturating_sub(data.len() as u64).saturating_add(1)).min(scratch.len() as u64) as usize;
        let count = read_checked(reader, request, &mut scratch[..probe])?;
        if count == 0 {
            return Ok(data);
        }
        let actual = data.len() as u64 + count as u64;
        if actual > limit {
            return Err(DecodeError::InputTooLarge {
                path: request.path.clone(),
                actual,
                limit,
            });
        }
        data.try_reserve_exact(count)
            .map_err(|_| DecodeError::InputAllocation {
                bytes: actual as usize,
            })?;
        data.extend_from_slice(&scratch[..count]);
    }
}

/// Budgeted immutable input for native decoders with managed source ownership.
/// A changed source length is rejected rather than growing outside admission.
pub fn read_bounded(request: &DecodeRequest) -> Result<SourceBuffer, DecodeError> {
    let Some(budget) = &request.memory_budget else {
        return read_legacy(request).map(SourceBuffer::Legacy);
    };
    read_managed_capped(request, budget, MAX_INPUT_BYTES).map(SourceBuffer::Managed)
}

/// Format-specific source limit enforced before managed source admission.
pub fn read_managed_capped(
    request: &DecodeRequest,
    budget: &rrrah_core::MemoryBudget,
    max_bytes: u64,
) -> Result<rrrah_core::MutableBuffer<u8>, DecodeError> {
    let limit = max_bytes.min(MAX_INPUT_BYTES);
    request.check_cancelled()?;
    let io_error = |source| DecodeError::Io {
        path: request.path.clone(),
        source,
    };
    let mut file = File::open(&request.path).map_err(io_error)?;
    let declared = file.metadata().map_err(io_error)?.len();
    if declared > limit {
        return Err(DecodeError::InputTooLarge {
            path: request.path.clone(),
            actual: declared,
            limit,
        });
    }
    let capacity = usize::try_from(declared).map_err(|_| DecodeError::DimensionOverflow)?;
    let mut data = budget.try_buffer(capacity, 0_u8)?;
    for chunk in data.chunks_mut(64 * 1024) {
        request.check_cancelled()?;
        read_exact_checked(&mut file, request, chunk)?;
    }
    request.check_cancelled()?;
    let mut extra = [0_u8; 1];
    if read_checked(&mut file, request, &mut extra)? != 0 {
        return Err(io_error(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "source grew during budgeted read",
        )));
    }
    Ok(data)
}

/// Mutable during parsing (e.g. CUR normalization), with ownership-bound admission.
#[derive(Debug)]
pub enum SourceBuffer {
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
pub fn read_managed(request: &DecodeRequest) -> Result<rrrah_core::PixelBuffer<u8>, DecodeError> {
    Ok(match read_bounded(request)? {
        SourceBuffer::Legacy(bytes) => std::sync::Arc::new(bytes).into(),
        SourceBuffer::Managed(bytes) => bytes.freeze().into(),
    })
}

#[cfg(test)]
mod interrupted_read_tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    struct Reader {
        calls: usize,
        token: Arc<AtomicU64>,
        interrupt: bool,
    }
    impl Read for Reader {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            self.calls += 1;
            if self.calls == 3 {
                self.token.store(2, Ordering::Release);
            }
            assert!(self.calls <= 3, "cancelled reader retried");
            if self.interrupt {
                Err(std::io::ErrorKind::Interrupted.into())
            } else {
                out[0] = 42;
                Ok(1)
            }
        }
    }
    #[test]
    fn cancellation_interrupts_retry_and_partial_exact_reads() {
        for interrupt in [true, false] {
            let token = Arc::new(AtomicU64::new(1));
            let mut request = DecodeRequest::new("controlled.raw");
            request.cancellation = Some(crate::GenerationToken::new(token.clone(), 1));
            let mut reader = Reader {
                calls: 0,
                token,
                interrupt,
            };
            assert!(matches!(
                read_exact_checked(&mut reader, &request, &mut [0; 16]),
                Err(DecodeError::Cancelled)
            ));
            assert_eq!(reader.calls, 3);
        }
    }
    #[test]
    fn cancelled_eof_never_publishes_legacy_source() {
        struct CancelEof {
            token: Arc<AtomicU64>,
        }
        impl Read for CancelEof {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                self.token.store(2, Ordering::Release);
                Ok(0)
            }
        }
        let token = Arc::new(AtomicU64::new(1));
        let mut request = DecodeRequest::new("controlled.raw");
        request.cancellation = Some(crate::GenerationToken::new(token.clone(), 1));
        assert!(matches!(
            read_legacy_stream(&mut CancelEof { token: token.clone() }, &request, 0, 16),
            Err(DecodeError::Cancelled)
        ));
        token.store(1, Ordering::Release);
        assert!(matches!(
            read_checked(&mut CancelEof { token }, &request, &mut [0]),
            Err(DecodeError::Cancelled)
        ));
    }

    #[test]
    fn already_cancelled_public_source_read_skips_file_open_in_both_modes() {
        let missing = std::env::temp_dir().join(format!(
            "rrrah-cancelled-missing-{}-{}.raw",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(!missing.exists());
        let token = Arc::new(AtomicU64::new(2));
        let mut request = DecodeRequest::new(missing);
        request.cancellation = Some(crate::GenerationToken::new(token, 1));
        assert!(matches!(read_bounded(&request), Err(DecodeError::Cancelled)));
        let budget = rrrah_core::MemoryBudget::new(1024);
        request.memory_budget = Some(budget.clone());
        assert!(matches!(read_bounded(&request), Err(DecodeError::Cancelled)));
        assert_eq!(budget.peak(), 0);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn legacy_stream_cancels_partial_reads_and_bounds_actual_growth() {
        for interrupt in [true, false] {
            let token = Arc::new(AtomicU64::new(1));
            let mut request = DecodeRequest::new("controlled.raw");
            request.cancellation = Some(crate::GenerationToken::new(token.clone(), 1));
            let mut reader = Reader {
                calls: 0,
                token,
                interrupt,
            };
            assert!(matches!(
                read_legacy_stream(&mut reader, &request, 0, 16),
                Err(DecodeError::Cancelled)
            ));
            assert_eq!(reader.calls, 3);
        }
        let request = DecodeRequest::new("controlled.raw");
        assert_eq!(
            read_legacy_stream(&mut &b"abc"[..], &request, 0, 3).unwrap(),
            b"abc"
        );
        assert_eq!(read_legacy_stream(&mut &b""[..], &request, 0, 0).unwrap(), b"");
        assert!(matches!(
            read_legacy_stream(&mut &b"abcd"[..], &request, 1, 3),
            Err(DecodeError::InputTooLarge {
                actual: 4,
                limit: 3,
                ..
            })
        ));
        assert!(matches!(
            read_legacy_stream(&mut &b"x"[..], &request, 0, 0),
            Err(DecodeError::InputTooLarge {
                actual: 1,
                limit: 0,
                ..
            })
        ));
    }

    #[test]
    fn exact_read_handles_short_input_and_partial_success() {
        let request = DecodeRequest::new("controlled.raw");
        let mut output = [0; 3];
        read_exact_checked(&mut &b"abc"[..], &request, &mut output).unwrap();
        assert_eq!(&output, b"abc");
        assert!(
            matches!(read_exact_checked(&mut &b"ab"[..], &request, &mut output), Err(DecodeError::Io { source, .. }) if source.kind() == std::io::ErrorKind::UnexpectedEof)
        );
        assert_eq!(read_checked(&mut &b""[..], &request, &mut output).unwrap(), 0);
    }
}
