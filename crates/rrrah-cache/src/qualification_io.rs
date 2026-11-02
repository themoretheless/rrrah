//! Opt-in delayed/short codec I/O over the real swap store's verified streams.
//! This does not simulate source-file reads, seek/fsync latency or blocked OS calls.
use std::io::{self, Read, Write};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub delay: Duration,
    pub chunk_bytes: usize,
}
impl Config {
    pub fn from_environment() -> io::Result<Option<Self>> {
        let read = |name| match std::env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(error) => Err(io::Error::new(io::ErrorKind::InvalidInput, error)),
        };
        let delay = read("RRRAH_QUALIFY_SWAP_DELAY_MS")?;
        let chunk = read("RRRAH_QUALIFY_SWAP_CHUNK_BYTES")?;
        Self::parse(delay.as_deref(), chunk.as_deref())
    }
    fn parse(delay: Option<&str>, chunk: Option<&str>) -> io::Result<Option<Self>> {
        let invalid = || {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "qualification swap I/O requires delay_ms=0..1000 and chunk_bytes=1..65536 together",
            )
        };
        match (delay, chunk) {
            (None, None) => Ok(None),
            (Some(delay), Some(chunk)) => {
                let delay = delay.parse::<u64>().map_err(|_| invalid())?;
                let chunk_bytes = chunk.parse::<usize>().map_err(|_| invalid())?;
                if delay > 1000 || !(1..=65536).contains(&chunk_bytes) {
                    return Err(invalid());
                }
                Ok(Some(Self {
                    delay: Duration::from_millis(delay),
                    chunk_bytes,
                }))
            }
            _ => Err(invalid()),
        }
    }
}
pub(crate) struct Io<'a, T: ?Sized> {
    inner: &'a mut T,
    config: Option<Config>,
}
impl<'a, T: ?Sized> Io<'a, T> {
    pub(crate) fn new(inner: &'a mut T, config: Option<Config>) -> Self {
        Self { inner, config }
    }
    fn length(&self, length: usize) -> usize {
        if length == 0 {
            return 0;
        }
        if let Some(config) = self.config {
            if !config.delay.is_zero() {
                std::thread::sleep(config.delay);
            }
            length.min(config.chunk_bytes)
        } else {
            length
        }
    }
}
impl<T: Read + ?Sized> Read for Io<'_, T> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let length = self.length(bytes.len());
        self.inner.read(&mut bytes[..length])
    }
}
impl<T: Write + ?Sized> Write for Io<'_, T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = self.length(bytes.len());
        self.inner.write(&bytes[..length])
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_incomplete_invalid_and_unbounded_configuration() {
        assert_eq!(Config::parse(None, None).unwrap(), None);
        for (delay, chunk) in [
            (Some("1"), None),
            (None, Some("8")),
            (Some("-1"), Some("8")),
            (Some("1001"), Some("8")),
            (Some("1"), Some("0")),
            (Some("1"), Some("65537")),
            (Some("NaN"), Some("8")),
        ] {
            assert!(Config::parse(delay, chunk).is_err());
        }
    }
    #[test]
    fn short_reads_and_writes_preserve_every_byte_and_propagate_io_errors() {
        struct Recorder {
            bytes: Vec<u8>,
            requests: Vec<usize>,
        }
        impl Write for Recorder {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.requests.push(bytes.len());
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let config = Config::parse(Some("0"), Some("3")).unwrap();
        let mut output = Recorder {
            bytes: Vec::new(),
            requests: Vec::new(),
        };
        Io::new(&mut output, config).write_all(b"abcdefgh").unwrap();
        assert_eq!(output.bytes, b"abcdefgh");
        assert_eq!(output.requests, [3, 3, 2]);
        let mut input = std::io::Cursor::new(output.bytes);
        let mut bytes = [0; 8];
        Io::new(&mut input, config).read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"abcdefgh");
        assert_eq!(Io::new(&mut input, config).read(&mut []).unwrap(), 0);
        assert!(Io::new(&mut input, config).read_exact(&mut [0; 1]).is_err());
        struct Fails;
        impl Read for Fails {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::ErrorKind::PermissionDenied.into())
            }
        }
        impl Write for Fails {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::PermissionDenied.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::ErrorKind::PermissionDenied.into())
            }
        }
        assert_eq!(
            Io::new(&mut Fails, config).read(&mut [0; 1]).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            Io::new(&mut Fails, config).write(b"x").unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            Io::new(&mut Fails, config).flush().unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    }
}
