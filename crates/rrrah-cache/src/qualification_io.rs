//! Opt-in delayed/short codec I/O over the real swap store's verified streams.
//! This does not simulate source-file reads, seek/fsync latency or blocked OS calls.
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
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
/// Process-wide qualification counters. Observe after all tested cache workers
/// have joined; live snapshots can span concurrent operations. No reset API.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub read_calls: u64,
    pub read_bytes: u64,
    pub max_read_request: u64,
    pub write_calls: u64,
    pub write_bytes: u64,
    pub max_write_request: u64,
    pub completed_spills: u64,
    pub completed_restores: u64,
}
struct Direction {
    calls: AtomicU64,
    bytes: AtomicU64,
    maximum: AtomicU64,
}
impl Direction {
    const fn new() -> Self {
        Self {
            calls: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            maximum: AtomicU64::new(0),
        }
    }
    fn record(&self, requested: usize, result: &io::Result<usize>) {
        if requested == 0 {
            return;
        }
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.maximum.fetch_max(requested as u64, Ordering::Relaxed);
        if let Ok(count) = result {
            self.bytes.fetch_add(*count as u64, Ordering::Relaxed);
        }
    }
}
struct Counters {
    read: Direction,
    write: Direction,
    spills: AtomicU64,
    restores: AtomicU64,
}
impl Counters {
    const fn new() -> Self {
        Self {
            read: Direction::new(),
            write: Direction::new(),
            spills: AtomicU64::new(0),
            restores: AtomicU64::new(0),
        }
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            read_calls: self.read.calls.load(Ordering::Relaxed),
            read_bytes: self.read.bytes.load(Ordering::Relaxed),
            max_read_request: self.read.maximum.load(Ordering::Relaxed),
            write_calls: self.write.calls.load(Ordering::Relaxed),
            write_bytes: self.write.bytes.load(Ordering::Relaxed),
            max_write_request: self.write.maximum.load(Ordering::Relaxed),
            completed_spills: self.spills.load(Ordering::Relaxed),
            completed_restores: self.restores.load(Ordering::Relaxed),
        }
    }
}
static COUNTERS: Counters = Counters::new();
pub fn snapshot() -> Snapshot {
    COUNTERS.snapshot()
}
pub(crate) fn record_spill() {
    COUNTERS.spills.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn record_restore() {
    COUNTERS.restores.fetch_add(1, Ordering::Relaxed);
}

pub(crate) struct Io<'a, T: ?Sized> {
    inner: &'a mut T,
    config: Option<Config>,
    counters: &'a Counters,
}
impl<'a, T: ?Sized> Io<'a, T> {
    pub(crate) fn new(inner: &'a mut T, config: Option<Config>) -> Self {
        Self {
            inner,
            config,
            counters: &COUNTERS,
        }
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
        let result = self.inner.read(&mut bytes[..length]);
        if self.config.is_some() {
            self.counters.read.record(length, &result);
        }
        result
    }
}
impl<T: Write + ?Sized> Write for Io<'_, T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = self.length(bytes.len());
        let result = self.inner.write(&bytes[..length]);
        if self.config.is_some() {
            self.counters.write.record(length, &result);
        }
        result
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
        let counters = Counters::new();
        let config = Config::parse(Some("0"), Some("3")).unwrap();
        let mut output = Recorder {
            bytes: Vec::new(),
            requests: Vec::new(),
        };
        Io {
            inner: &mut output,
            config,
            counters: &counters,
        }
        .write_all(b"abcdefgh")
        .unwrap();
        assert_eq!(output.bytes, b"abcdefgh");
        assert_eq!(output.requests, [3, 3, 2]);
        let mut input = std::io::Cursor::new(output.bytes);
        let mut bytes = [0; 8];
        Io {
            inner: &mut input,
            config,
            counters: &counters,
        }
        .read_exact(&mut bytes)
        .unwrap();
        assert_eq!(&bytes, b"abcdefgh");
        assert_eq!(
            Io {
                inner: &mut input,
                config,
                counters: &counters
            }
            .read(&mut [])
            .unwrap(),
            0
        );
        assert!(
            Io {
                inner: &mut input,
                config,
                counters: &counters
            }
            .read_exact(&mut [0; 1])
            .is_err()
        );
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
            Io {
                inner: &mut Fails,
                config,
                counters: &counters
            }
            .read(&mut [0; 1])
            .unwrap_err()
            .kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            Io {
                inner: &mut Fails,
                config,
                counters: &counters
            }
            .write(b"x")
            .unwrap_err()
            .kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            Io {
                inner: &mut Fails,
                config,
                counters: &counters
            }
            .flush()
            .unwrap_err()
            .kind(),
            io::ErrorKind::PermissionDenied
        );
        let snapshot = counters.snapshot();
        assert_eq!((snapshot.read_bytes, snapshot.write_bytes), (8, 8));
        assert_eq!((snapshot.max_read_request, snapshot.max_write_request), (3, 3));
        assert_eq!((snapshot.read_calls, snapshot.write_calls), (5, 4));
        assert_eq!((snapshot.completed_spills, snapshot.completed_restores), (0, 0));
        let idle = Counters::new();
        Io {
            inner: &mut Vec::new(),
            config: None,
            counters: &idle,
        }
        .write_all(b"unconfigured")
        .unwrap();
        assert_eq!(idle.snapshot(), Snapshot::default());
    }
}
