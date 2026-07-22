use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    mem::{size_of, size_of_val},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use rrrah_core::{DecodedMosaic, FrameError, LEGACY_V2_CACHE_ABI, RawMetadata};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

const MAGIC: &[u8; 8] = b"RRRAHRC1";
const MAX_HEADER_BYTES: usize = 1 << 20;
const MAX_CACHED_PIXELS: u64 = 250_000_000;
const SAMPLE_BYTES: u64 = 64 * 1024;
const PAYLOAD_BUFFER_BYTES: usize = 16 * 1024;
const WRITE_LOCK_FILE: &str = ".rrrah-cache-write.lock";

/// Default ceiling for decoded mosaics. Writes prune the oldest complete
/// entries before publication, so speculative prefetch cannot grow without
/// bound and consume the volume hosting the user's cache directory.
pub const DEFAULT_MAX_DISK_CACHE_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFingerprint {
    pub file_size: u64,
    pub modified_ns: u128,
    pub sampled_blake3: [u8; 32],
}

impl SourceFingerprint {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, CacheError> {
        let path = path.as_ref();
        let metadata = fs::metadata(path).map_err(|source| CacheError::Io {
            path: path.to_owned(),
            source,
        })?;
        if !metadata.is_file() {
            return Err(CacheError::NotAFile(path.to_owned()));
        }
        let file_size = metadata.len();
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_nanos());
        let mut file = File::open(path).map_err(|source| CacheError::Io {
            path: path.to_owned(),
            source,
        })?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"rrrah/source-sample/v1\0");
        hasher.update(&file_size.to_le_bytes());
        for offset in sample_offsets(file_size) {
            file.seek(SeekFrom::Start(offset))
                .map_err(|source| CacheError::Io {
                    path: path.to_owned(),
                    source,
                })?;
            hasher.update(&offset.to_le_bytes());
            let remaining = file_size.saturating_sub(offset).min(SAMPLE_BYTES);
            let sample_len = usize::try_from(remaining).map_err(|_| CacheError::SizeOverflow)?;
            let mut sample = vec![0_u8; sample_len];
            file.read_exact(&mut sample).map_err(|source| CacheError::Io {
                path: path.to_owned(),
                source,
            })?;
            hasher.update(&sample);
        }
        Ok(Self {
            file_size,
            modified_ns,
            sampled_blake3: *hasher.finalize().as_bytes(),
        })
    }
}

fn sample_offsets(file_size: u64) -> Vec<u64> {
    if file_size <= SAMPLE_BYTES {
        return vec![0];
    }
    let last = file_size - SAMPLE_BYTES;
    let middle = file_size.saturating_sub(SAMPLE_BYTES) / 2;
    let mut offsets = vec![0, middle, last];
    offsets.sort_unstable();
    offsets.dedup();
    offsets
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey([u8; 32]);

impl CacheKey {
    pub fn for_mosaic(source: &SourceFingerprint, image_index: usize) -> Self {
        let mut hasher = blake3::Hasher::new();
        // Bump the namespace whenever decoded metadata or pixel semantics change.
        // v2 invalidates mosaics written before the RAW colour-metadata fix.
        hasher.update(b"rrrah/decoded-mosaic/v2\0");
        hasher.update(&LEGACY_V2_CACHE_ABI.to_le_bytes());
        hasher.update(&source.file_size.to_le_bytes());
        hasher.update(&source.modified_ns.to_le_bytes());
        hasher.update(&source.sampled_blake3);
        hasher.update(&image_index.to_le_bytes());
        Self(*hasher.finalize().as_bytes())
    }

    pub fn to_hex(self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

impl fmt::Debug for CacheKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("CacheKey").field(&self.to_hex()).finish()
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

#[derive(Debug, Clone)]
pub struct CacheLoad {
    pub mosaic: DecodedMosaic,
    pub elapsed: Duration,
}

/// A point-in-time measurement of complete decoded-mosaic cache entries.
/// Temporary files and the cache lock are deliberately excluded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiskCacheUsage {
    pub resident_bytes: u64,
    pub entries: u64,
}

#[derive(Debug, Clone)]
pub struct DiskMosaicCache {
    root: PathBuf,
    max_bytes: u64,
}

impl DiskMosaicCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_max_bytes(root, DEFAULT_MAX_DISK_CACHE_BYTES)
    }

    pub fn with_max_bytes(root: impl Into<PathBuf>, max_bytes: u64) -> Self {
        Self {
            root: root.into(),
            max_bytes,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    /// Cheap cache-presence probe used by speculative prefetch. Validation is
    /// still performed by `load`; a corrupt entry therefore only postpones
    /// the fallback decode until the image becomes foreground work.
    pub fn contains(&self, key: CacheKey) -> bool {
        fs::metadata(self.path_for(key)).is_ok_and(|metadata| metadata.is_file())
    }

    /// Measure complete cache entries. This walks the two-level cache tree and
    /// must therefore run on a cache worker, never in a render/event callback.
    pub fn usage(&self) -> Result<DiskCacheUsage, CacheError> {
        let shards = match fs::read_dir(&self.root) {
            Ok(shards) => shards,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(DiskCacheUsage::default());
            }
            Err(source) => {
                return Err(CacheError::Io {
                    path: self.root.clone(),
                    source,
                });
            }
        };
        let mut usage = DiskCacheUsage::default();
        for shard in shards {
            let shard = shard.map_err(|source| CacheError::Io {
                path: self.root.clone(),
                source,
            })?;
            let shard_path = shard.path();
            let shard_type = shard.file_type().map_err(|source| CacheError::Io {
                path: shard_path.clone(),
                source,
            })?;
            if !shard_type.is_dir() {
                continue;
            }
            for entry in fs::read_dir(&shard_path).map_err(|source| CacheError::Io {
                path: shard_path.clone(),
                source,
            })? {
                let entry = entry.map_err(|source| CacheError::Io {
                    path: shard_path.clone(),
                    source,
                })?;
                let entry_path = entry.path();
                if entry_path.extension().and_then(|value| value.to_str()) != Some("rrc") {
                    continue;
                }
                let entry_type = entry.file_type().map_err(|source| CacheError::Io {
                    path: entry_path.clone(),
                    source,
                })?;
                if !entry_type.is_file() {
                    continue;
                }
                let metadata = entry.metadata().map_err(|source| CacheError::Io {
                    path: entry_path,
                    source,
                })?;
                usage.resident_bytes = usage
                    .resident_bytes
                    .checked_add(metadata.len())
                    .ok_or(CacheError::SizeOverflow)?;
                usage.entries = usage.entries.checked_add(1).ok_or(CacheError::SizeOverflow)?;
            }
        }
        Ok(usage)
    }

    pub fn load(&self, key: CacheKey) -> Result<Option<CacheLoad>, CacheError> {
        let started = Instant::now();
        let path = self.path_for(key);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(CacheError::Io { path, source }),
        };
        let file_bytes = file
            .metadata()
            .map_err(|source| CacheError::Io {
                path: path.clone(),
                source,
            })?
            .len();
        let mut reader = BufReader::new(file);
        let mut magic = [0_u8; 8];
        reader.read_exact(&mut magic).map_err(|source| CacheError::Io {
            path: path.clone(),
            source,
        })?;
        if &magic != MAGIC {
            return Err(CacheError::Corrupt("bad magic"));
        }
        let header_len = read_u32(&mut reader, &path)? as usize;
        if header_len > MAX_HEADER_BYTES {
            return Err(CacheError::Corrupt("header is too large"));
        }
        let mut header_bytes = vec![0_u8; header_len];
        reader
            .read_exact(&mut header_bytes)
            .map_err(|source| CacheError::Io {
                path: path.clone(),
                source,
            })?;
        let header: CacheHeader = serde_json::from_slice(&header_bytes)?;
        if header.schema != LEGACY_V2_CACHE_ABI || header.key != key {
            return Ok(None);
        }
        header.metadata.validate()?;
        let expected_pixels = u64::from(header.metadata.width)
            .checked_mul(u64::from(header.metadata.height))
            .and_then(|value| value.checked_mul(u64::from(header.metadata.components_per_pixel)))
            .ok_or(CacheError::SizeOverflow)?;
        if header.pixel_count != expected_pixels || header.pixel_count > MAX_CACHED_PIXELS {
            return Err(CacheError::Corrupt(
                "pixel count is inconsistent or exceeds cache limit",
            ));
        }
        let payload_bytes = checked_payload_bytes(header.pixel_count)?;
        let expected_file_bytes = 12_u64
            .checked_add(u64::try_from(header_len).map_err(|_| CacheError::SizeOverflow)?)
            .and_then(|bytes| bytes.checked_add(u64::try_from(payload_bytes).ok()?))
            .ok_or(CacheError::SizeOverflow)?;
        if file_bytes != expected_file_bytes {
            return Err(CacheError::Corrupt("file length does not match header"));
        }
        let pixel_count = usize::try_from(header.pixel_count).map_err(|_| CacheError::SizeOverflow)?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(pixel_count)
            .map_err(|_| CacheError::AllocationFailed {
                bytes: u64::try_from(payload_bytes).unwrap_or(u64::MAX),
            })?;
        let mut payload_hasher = blake3::Hasher::new();
        let mut buffer = [0_u8; PAYLOAD_BUFFER_BYTES];
        let mut remaining = payload_bytes;
        while remaining != 0 {
            let chunk_len = remaining.min(buffer.len());
            let chunk = &mut buffer[..chunk_len];
            reader.read_exact(chunk).map_err(|source| CacheError::Io {
                path: path.clone(),
                source,
            })?;
            payload_hasher.update(chunk);
            pixels.extend(
                chunk
                    .chunks_exact(2)
                    .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]])),
            );
            remaining -= chunk_len;
        }
        if payload_hasher.finalize().as_bytes() != &header.payload_blake3 {
            return Err(CacheError::Corrupt("payload checksum mismatch"));
        }
        let mut trailing = [0_u8; 1];
        if reader.read(&mut trailing).map_err(|source| CacheError::Io {
            path: path.clone(),
            source,
        })? != 0
        {
            return Err(CacheError::Corrupt("trailing bytes"));
        }
        let mosaic = DecodedMosaic::new(header.metadata, Arc::new(pixels))?;
        Ok(Some(CacheLoad {
            mosaic,
            elapsed: started.elapsed(),
        }))
    }

    pub fn store(&self, key: CacheKey, mosaic: &DecodedMosaic) -> Result<PathBuf, CacheError> {
        // Never let an untrusted/accidentally oversized decoded frame turn a
        // cache write into an unbounded allocation. `load` enforces the same
        // limit before allocating its payload, so this keeps both sides of
        // the persistence boundary symmetric.
        let pixel_count = u64::try_from(mosaic.pixels.len()).map_err(|_| CacheError::SizeOverflow)?;
        ensure_cache_size(pixel_count)?;
        let path = self.path_for(key);
        let parent = path
            .parent()
            .ok_or(CacheError::Corrupt("cache path has no parent"))?;
        fs::create_dir_all(parent).map_err(|source| CacheError::Io {
            path: parent.to_owned(),
            source,
        })?;

        let header = CacheHeader {
            schema: LEGACY_V2_CACHE_ABI,
            key,
            pixel_count,
            payload_blake3: hash_pixels(&mosaic.pixels),
            metadata: mosaic.metadata.clone(),
        };
        let header_bytes = serde_json::to_vec(&header)?;
        if header_bytes.len() > MAX_HEADER_BYTES {
            return Err(CacheError::Corrupt("serialized header is too large"));
        }

        let entry_bytes = 8_u64
            .checked_add(4)
            .and_then(|bytes| bytes.checked_add(u64::try_from(header_bytes.len()).ok()?))
            .and_then(|bytes| bytes.checked_add(pixel_count.checked_mul(2)?))
            .ok_or(CacheError::SizeOverflow)?;

        // Capacity accounting and final rename are serialized across cache
        // instances/processes. Readers need no lock because publication is an
        // atomic same-directory rename and can only expose an old or new
        // complete entry.
        let write_lock = self.lock_writes()?;
        self.prune_for_write(&path, entry_bytes)?;

        let mut temporary = NamedTempFile::new_in(parent).map_err(|source| CacheError::Io {
            path: parent.to_owned(),
            source,
        })?;
        {
            let mut writer = BufWriter::new(&mut temporary);
            writer.write_all(MAGIC)?;
            writer.write_all(&(header_bytes.len() as u32).to_le_bytes())?;
            writer.write_all(&header_bytes)?;
            write_pixels(&mut writer, &mosaic.pixels)?;
            writer.flush()?;
        }
        temporary.as_file().sync_data()?;
        temporary.persist(&path).map_err(|error| CacheError::Io {
            path: path.clone(),
            source: error.error,
        })?;
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
        drop(write_lock);
        Ok(path)
    }

    fn lock_writes(&self) -> Result<File, CacheError> {
        fs::create_dir_all(&self.root).map_err(|source| CacheError::Io {
            path: self.root.clone(),
            source,
        })?;
        let lock_path = self.root.join(WRITE_LOCK_FILE);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| CacheError::Io {
                path: lock_path.clone(),
                source,
            })?;
        lock.lock().map_err(|source| CacheError::Io {
            path: lock_path,
            source,
        })?;
        Ok(lock)
    }

    fn prune_for_write(&self, destination: &Path, incoming_bytes: u64) -> Result<(), CacheError> {
        if incoming_bytes > self.max_bytes {
            return Err(CacheError::DiskBudgetExceeded {
                incoming: incoming_bytes,
                limit: self.max_bytes,
            });
        }

        let mut resident = 0_u64;
        let mut candidates = Vec::new();
        for shard in fs::read_dir(&self.root).map_err(|source| CacheError::Io {
            path: self.root.clone(),
            source,
        })? {
            let shard = shard.map_err(|source| CacheError::Io {
                path: self.root.clone(),
                source,
            })?;
            let shard_path = shard.path();
            let shard_type = shard.file_type().map_err(|source| CacheError::Io {
                path: shard_path.clone(),
                source,
            })?;
            if !shard_type.is_dir() {
                continue;
            }
            for entry in fs::read_dir(&shard_path).map_err(|source| CacheError::Io {
                path: shard_path.clone(),
                source,
            })? {
                let entry = entry.map_err(|source| CacheError::Io {
                    path: shard_path.clone(),
                    source,
                })?;
                let entry_path = entry.path();
                if entry_path == destination
                    || entry_path.extension().and_then(|value| value.to_str()) != Some("rrc")
                {
                    continue;
                }
                let metadata = entry.metadata().map_err(|source| CacheError::Io {
                    path: entry_path.clone(),
                    source,
                })?;
                if !metadata.is_file() {
                    continue;
                }
                resident = resident
                    .checked_add(metadata.len())
                    .ok_or(CacheError::SizeOverflow)?;
                let age = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map_or(0, |duration| duration.as_nanos());
                candidates.push((age, entry_path, metadata.len()));
            }
        }

        candidates.sort_unstable_by_key(|(age, _, _)| *age);
        for (_, candidate, bytes) in candidates {
            if resident.saturating_add(incoming_bytes) <= self.max_bytes {
                break;
            }
            match fs::remove_file(&candidate) {
                Ok(()) => resident = resident.saturating_sub(bytes),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    resident = resident.saturating_sub(bytes);
                }
                Err(source) => {
                    return Err(CacheError::Io {
                        path: candidate,
                        source,
                    });
                }
            }
        }
        if resident.saturating_add(incoming_bytes) > self.max_bytes {
            return Err(CacheError::DiskBudgetExceeded {
                incoming: incoming_bytes,
                limit: self.max_bytes,
            });
        }
        Ok(())
    }

    fn path_for(&self, key: CacheKey) -> PathBuf {
        let hex = key.to_hex();
        self.root.join(&hex[..2]).join(format!("{hex}.rrc"))
    }
}

fn hash_pixels(pixels: &[u16]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; PAYLOAD_BUFFER_BYTES];
    for samples in pixels.chunks(PAYLOAD_BUFFER_BYTES / size_of::<u16>()) {
        let bytes = &mut buffer[..size_of_val(samples)];
        for (sample, output) in samples.iter().zip(bytes.chunks_exact_mut(2)) {
            output.copy_from_slice(&sample.to_le_bytes());
        }
        hasher.update(bytes);
    }
    *hasher.finalize().as_bytes()
}

fn write_pixels(writer: &mut impl Write, pixels: &[u16]) -> Result<(), std::io::Error> {
    let mut buffer = [0_u8; PAYLOAD_BUFFER_BYTES];
    for samples in pixels.chunks(PAYLOAD_BUFFER_BYTES / size_of::<u16>()) {
        let bytes = &mut buffer[..size_of_val(samples)];
        for (sample, output) in samples.iter().zip(bytes.chunks_exact_mut(2)) {
            output.copy_from_slice(&sample.to_le_bytes());
        }
        writer.write_all(bytes)?;
    }
    Ok(())
}

fn read_u32(reader: &mut impl Read, path: &Path) -> Result<u32, CacheError> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes).map_err(|source| CacheError::Io {
        path: path.to_owned(),
        source,
    })?;
    Ok(u32::from_le_bytes(bytes))
}

fn ensure_cache_size(pixel_count: u64) -> Result<(), CacheError> {
    if pixel_count > MAX_CACHED_PIXELS {
        return Err(CacheError::FrameTooLarge {
            pixels: pixel_count,
            max: MAX_CACHED_PIXELS,
        });
    }
    Ok(())
}

/// Converts a validated pixel count into the byte count used by the on-disk
/// payload. The count is read from an untrusted cache header and must never
/// flow directly into a vector allocation without this checked conversion.
fn checked_payload_bytes(pixel_count: u64) -> Result<usize, CacheError> {
    ensure_cache_size(pixel_count)?;
    let sample_bytes = u64::try_from(size_of::<u16>()).map_err(|_| CacheError::SizeOverflow)?;
    pixel_count
        .checked_mul(sample_bytes)
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or(CacheError::SizeOverflow)
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheHeader {
    schema: u32,
    key: CacheKey,
    pixel_count: u64,
    payload_blake3: [u8; 32],
    metadata: RawMetadata,
}

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("cache I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cache source is not a regular file: {0}")]
    NotAFile(PathBuf),
    #[error("cache size arithmetic overflow")]
    SizeOverflow,
    #[error("cannot reserve {bytes} bytes for cached pixels")]
    AllocationFailed { bytes: u64 },
    #[error("decoded frame has {pixels} pixels, exceeding cache limit {max}")]
    FrameTooLarge { pixels: u64, max: u64 },
    #[error("cache entry needs {incoming} bytes, exceeding disk-cache budget {limit}")]
    DiskBudgetExceeded { incoming: u64, limit: u64 },
    #[error("corrupt cache entry: {0}")]
    Corrupt(&'static str),
    #[error("invalid cached frame: {0}")]
    InvalidFrame(#[from] FrameError),
    #[error("cache header serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("cache I/O failed: {0}")]
    PlainIo(#[from] std::io::Error),
}

impl CacheError {
    /// Whether speculative work should stop for the current batch instead of
    /// retrying every neighbour and repeatedly pressuring the same volume.
    pub fn is_disk_pressure(&self) -> bool {
        match self {
            Self::DiskBudgetExceeded { .. } => true,
            Self::Io { source, .. } | Self::PlainIo(source) => matches!(
                source.kind(),
                std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded
            ),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
    };

    use rrrah_core::{
        CfaColor, CfaPattern, DecodedMosaic, LevelGrid, Orientation, Photometric, RawMetadata, WhiteLevel,
    };
    use tempfile::tempdir;

    use super::{
        CacheError, CacheKey, DiskMosaicCache, MAGIC, MAX_CACHED_PIXELS, MAX_HEADER_BYTES, SourceFingerprint,
        checked_payload_bytes, hash_pixels, write_pixels,
    };

    fn test_key(tag: u8) -> CacheKey {
        CacheKey::for_mosaic(
            &SourceFingerprint {
                file_size: u64::from(tag),
                modified_ns: u128::from(tag),
                sampled_blake3: [tag; 32],
            },
            0,
        )
    }

    fn legacy_fixture_key() -> CacheKey {
        CacheKey([
            38, 104, 79, 209, 38, 253, 207, 63, 193, 226, 239, 21, 158, 73, 161, 253, 43, 127, 41, 112, 248,
            223, 43, 33, 164, 183, 147, 27, 168, 219, 219, 70,
        ])
    }

    fn legacy_fixture_bytes() -> Vec<u8> {
        let hex = include_str!("../tests/fixtures/legacy_v2_mosaic.hex")
            .trim()
            .as_bytes();
        assert_eq!(hex.len() % 2, 0);
        hex.chunks_exact(2)
            .map(|pair| {
                let nibble = |byte| match byte {
                    b'0'..=b'9' => byte - b'0',
                    b'a'..=b'f' => byte - b'a' + 10,
                    _ => panic!("invalid checked-in fixture hex"),
                };
                (nibble(pair[0]) << 4) | nibble(pair[1])
            })
            .collect()
    }

    fn test_mosaic() -> DecodedMosaic {
        let metadata = RawMetadata {
            make: "Test".into(),
            model: "Synthetic".into(),
            width: 2,
            height: 2,
            components_per_pixel: 1,
            bits_per_sample: 14,
            photometric: Photometric::Cfa,
            cfa: Some(CfaPattern {
                width: 2,
                height: 2,
                cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
            }),
            black_level: LevelGrid {
                width: 1,
                height: 1,
                components: 1,
                values: vec![512.0],
            },
            white_level: WhiteLevel(vec![16_383.0]),
            white_balance: [2.0, 1.0, 1.5, 1.0],
            xyz_to_camera: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]],
            active_area: None,
            crop_area: None,
            orientation: Orientation::Normal,
        };
        DecodedMosaic::new(metadata, Arc::new(vec![512_u16, 1024, 2048, 4096])).unwrap()
    }

    #[test]
    fn disk_cache_round_trip() {
        let directory = tempdir().unwrap();
        let cache = DiskMosaicCache::new(directory.path());
        let fingerprint = SourceFingerprint {
            file_size: 42,
            modified_ns: 7,
            sampled_blake3: [3; 32],
        };
        let key = CacheKey::for_mosaic(&fingerprint, 0);
        let original = test_mosaic();
        cache.store(key, &original).unwrap();
        let loaded = cache.load(key).unwrap().expect("cache hit");
        assert_eq!(&*loaded.mosaic.pixels, &*original.pixels);
        assert_eq!(loaded.mosaic.metadata, original.metadata);
    }

    #[test]
    fn checked_in_legacy_v2_object_remains_byte_exact_and_readable() {
        let key = legacy_fixture_key();
        let fixture = legacy_fixture_bytes();

        let read_directory = tempdir().unwrap();
        let read_cache = DiskMosaicCache::new(read_directory.path());
        let fixture_path = read_cache.path_for(key);
        std::fs::create_dir_all(fixture_path.parent().unwrap()).unwrap();
        std::fs::write(&fixture_path, &fixture).unwrap();
        let loaded = read_cache.load(key).unwrap().expect("frozen V2 hit");
        let expected = test_mosaic();
        assert_eq!(loaded.mosaic.metadata, expected.metadata);
        assert_eq!(loaded.mosaic.pixels, expected.pixels);

        let write_directory = tempdir().unwrap();
        let write_cache = DiskMosaicCache::new(write_directory.path());
        let written_path = write_cache.store(key, &expected).unwrap();
        assert_eq!(std::fs::read(written_path).unwrap(), fixture);
    }

    #[test]
    fn usage_counts_published_entries_and_ignores_other_files() {
        let directory = tempdir().unwrap();
        let cache = DiskMosaicCache::new(directory.path());
        assert_eq!(cache.usage().unwrap(), super::DiskCacheUsage::default());

        let path = cache.store(test_key(9), &test_mosaic()).unwrap();
        std::fs::write(directory.path().join("unrelated.rrc"), b"not in a shard").unwrap();
        std::fs::write(path.parent().unwrap().join("partial.tmp"), b"temporary").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&path, path.parent().unwrap().join("alias.rrc")).unwrap();

        let usage = cache.usage().unwrap();
        assert_eq!(usage.entries, 1);
        assert_eq!(usage.resident_bytes, std::fs::metadata(path).unwrap().len());
    }

    #[test]
    fn streaming_payload_encoding_matches_canonical_little_endian_bytes() {
        // Cross the internal 64 KiB boundary and leave a partial final chunk.
        let pixels = (0_u32..40_003)
            .map(|value| (value ^ (value >> 11)) as u16)
            .collect::<Vec<_>>();
        let canonical = pixels
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        let mut streamed = Vec::new();
        write_pixels(&mut streamed, &pixels).unwrap();
        assert_eq!(streamed, canonical);
        assert_eq!(hash_pixels(&pixels), *blake3::hash(&canonical).as_bytes());
    }

    #[test]
    fn disk_budget_prunes_an_old_complete_entry_before_publish() {
        let directory = tempdir().unwrap();
        let first_key = test_key(1);
        let second_key = test_key(2);
        let unbounded_for_setup = DiskMosaicCache::with_max_bytes(directory.path(), u64::MAX);
        let first_path = unbounded_for_setup.store(first_key, &test_mosaic()).unwrap();
        let first_bytes = std::fs::metadata(first_path).unwrap().len();
        let sizing_directory = tempdir().unwrap();
        let second_path = DiskMosaicCache::with_max_bytes(sizing_directory.path(), u64::MAX)
            .store(second_key, &test_mosaic())
            .unwrap();
        let second_bytes = std::fs::metadata(second_path).unwrap().len();
        let one_entry_budget = first_bytes.max(second_bytes);

        let bounded = DiskMosaicCache::with_max_bytes(directory.path(), one_entry_budget);
        bounded.store(second_key, &test_mosaic()).unwrap();
        assert!(!bounded.contains(first_key));
        assert!(bounded.contains(second_key));
        assert!(bounded.load(second_key).unwrap().is_some());
    }

    #[test]
    fn oversized_entry_is_rejected_without_evicting_resident_data() {
        let directory = tempdir().unwrap();
        let resident_key = test_key(3);
        let rejected_key = test_key(4);
        let setup = DiskMosaicCache::with_max_bytes(directory.path(), u64::MAX);
        let resident_path = setup.store(resident_key, &test_mosaic()).unwrap();
        let entry_bytes = std::fs::metadata(resident_path).unwrap().len();

        let bounded = DiskMosaicCache::with_max_bytes(directory.path(), entry_bytes - 1);
        assert!(matches!(
            bounded.store(rejected_key, &test_mosaic()),
            Err(CacheError::DiskBudgetExceeded { .. })
        ));
        assert!(bounded.contains(resident_key));
        assert!(bounded.load(resident_key).unwrap().is_some());
        assert!(!bounded.contains(rejected_key));
    }

    #[test]
    fn disk_pressure_errors_are_classified_for_prefetch_backoff() {
        let budget = CacheError::DiskBudgetExceeded {
            incoming: 2,
            limit: 1,
        };
        let full = CacheError::PlainIo(std::io::Error::from(std::io::ErrorKind::StorageFull));
        let quota = CacheError::PlainIo(std::io::Error::from(std::io::ErrorKind::QuotaExceeded));
        let unrelated = CacheError::PlainIo(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert!(budget.is_disk_pressure());
        assert!(full.is_disk_pressure());
        assert!(quota.is_disk_pressure());
        assert!(!unrelated.is_disk_pressure());
    }

    #[test]
    fn concurrent_replacement_never_exposes_a_partial_entry() {
        let directory = tempdir().unwrap();
        let cache = DiskMosaicCache::new(directory.path());
        let key = test_key(5);
        let first = test_mosaic();
        let mut second = first.clone();
        second.pixels = Arc::new(vec![513_u16, 1025, 2049, 4097]);
        cache.store(key, &first).unwrap();

        let writers_alive = Arc::new(AtomicUsize::new(2));
        let writers = [first.clone(), second.clone()].map(|mosaic| {
            let cache = cache.clone();
            let writers_alive = Arc::clone(&writers_alive);
            thread::spawn(move || {
                for _ in 0..32 {
                    cache.store(key, &mosaic).unwrap();
                }
                writers_alive.fetch_sub(1, Ordering::Release);
            })
        });

        while writers_alive.load(Ordering::Acquire) != 0 {
            let loaded = cache.load(key).unwrap().expect("entry remains visible");
            assert!(loaded.mosaic.pixels == first.pixels || loaded.mosaic.pixels == second.pixels);
            thread::yield_now();
        }
        for writer in writers {
            writer.join().unwrap();
        }
        assert!(cache.load(key).unwrap().is_some());
    }

    #[test]
    fn corrupt_payload_is_rejected_without_panicking() {
        let directory = tempdir().unwrap();
        let cache = DiskMosaicCache::new(directory.path());
        let fingerprint = SourceFingerprint {
            file_size: 42,
            modified_ns: 7,
            sampled_blake3: [3; 32],
        };
        let key = CacheKey::for_mosaic(&fingerprint, 0);
        let original = test_mosaic();
        let path = cache.store(key, &original).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        *bytes.last_mut().unwrap() ^= 0x80;
        std::fs::write(&path, bytes).unwrap();
        assert!(matches!(cache.load(key), Err(super::CacheError::Corrupt(_))));
    }

    #[test]
    fn oversized_header_is_rejected_before_json_parse() {
        let directory = tempdir().unwrap();
        let cache = DiskMosaicCache::new(directory.path());
        let fingerprint = SourceFingerprint {
            file_size: 42,
            modified_ns: 7,
            sampled_blake3: [3; 32],
        };
        let key = CacheKey::for_mosaic(&fingerprint, 0);
        let path = cache.path_for(key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&u32::try_from(MAX_HEADER_BYTES + 1).unwrap().to_le_bytes());
        // The reader must reject based on the declared length and not attempt
        // to allocate/read this body. A single byte is enough to exercise it.
        bytes.push(b'{');
        std::fs::write(path, bytes).unwrap();
        assert!(matches!(
            cache.load(key),
            Err(super::CacheError::Corrupt("header is too large"))
        ));
    }

    #[test]
    fn oversized_payload_is_rejected_before_allocation() {
        assert!(super::ensure_cache_size(super::MAX_CACHED_PIXELS).is_ok());
        let error = super::ensure_cache_size(super::MAX_CACHED_PIXELS + 1).unwrap_err();
        assert!(matches!(error, super::CacheError::FrameTooLarge { .. }));
    }

    #[test]
    fn payload_byte_arithmetic_is_checked_before_allocation() {
        assert_eq!(checked_payload_bytes(0).unwrap(), 0);
        assert_eq!(
            checked_payload_bytes(MAX_CACHED_PIXELS).unwrap(),
            usize::try_from(MAX_CACHED_PIXELS * 2).unwrap()
        );
        assert!(matches!(
            checked_payload_bytes(u64::MAX),
            Err(super::CacheError::FrameTooLarge { .. })
        ));
    }

    #[test]
    fn malformed_header_lengths_are_bounded_and_never_panic() {
        let directory = tempdir().unwrap();
        let cache = DiskMosaicCache::new(directory.path());
        let fingerprint = SourceFingerprint {
            file_size: 42,
            modified_ns: 7,
            sampled_blake3: [3; 32],
        };
        let key = CacheKey::for_mosaic(&fingerprint, 0);
        let path = cache.path_for(key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        // Keep malformed inputs tiny. A huge declared length must be rejected
        // before allocating its body; small/truncated lengths can only produce
        // a typed I/O/JSON error.
        for declared in [0_u32, 1, (MAX_HEADER_BYTES as u32) + 1, u32::MAX] {
            let mut bytes = Vec::with_capacity(13);
            bytes.extend_from_slice(MAGIC);
            bytes.extend_from_slice(&declared.to_le_bytes());
            bytes.push(b'{');
            std::fs::write(&path, &bytes).unwrap();
            let result = std::panic::catch_unwind(|| cache.load(key));
            assert!(result.is_ok(), "cache parser panicked for {declared}");
            if declared as usize > MAX_HEADER_BYTES {
                assert!(matches!(
                    result.unwrap(),
                    Err(super::CacheError::Corrupt("header is too large"))
                ));
            } else {
                assert!(result.unwrap().is_err());
            }
        }
    }

    #[test]
    fn declared_pixel_count_is_checked_before_payload_allocation() {
        let directory = tempdir().unwrap();
        let cache = DiskMosaicCache::new(directory.path());
        let fingerprint = SourceFingerprint {
            file_size: 42,
            modified_ns: 7,
            sampled_blake3: [3; 32],
        };
        let key = CacheKey::for_mosaic(&fingerprint, 0);
        let original = test_mosaic();
        let path = cache.store(key, &original).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut header: serde_json::Value = serde_json::from_slice(&bytes[12..12 + header_len]).unwrap();
        header["pixel_count"] = serde_json::Value::from(super::MAX_CACHED_PIXELS + 1);
        let header_bytes = serde_json::to_vec(&header).unwrap();
        let mut mutated = Vec::with_capacity(12 + header_bytes.len() + bytes.len() - 12 - header_len);
        mutated.extend_from_slice(MAGIC);
        mutated.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
        mutated.extend_from_slice(&header_bytes);
        mutated.extend_from_slice(&bytes[12 + header_len..]);
        std::fs::write(path, mutated).unwrap();
        assert!(matches!(
            cache.load(key),
            Err(super::CacheError::Corrupt(
                "pixel count is inconsistent or exceeds cache limit"
            ))
        ));
    }

    #[test]
    fn truncated_large_frame_is_rejected_by_file_length_before_reserving_pixels() {
        let directory = tempdir().unwrap();
        let cache = DiskMosaicCache::new(directory.path());
        let key = test_key(11);
        let path = cache.store(key, &test_mosaic()).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut header: serde_json::Value = serde_json::from_slice(&bytes[12..12 + header_len]).unwrap();

        // These dimensions are internally valid and below MAX_CACHED_PIXELS,
        // but the tiny file cannot contain their 200 MB payload. File-length
        // validation must reject it before Vec::try_reserve_exact is reached.
        header["metadata"]["width"] = serde_json::Value::from(10_000_u32);
        header["metadata"]["height"] = serde_json::Value::from(10_000_u32);
        header["pixel_count"] = serde_json::Value::from(100_000_000_u64);
        let header_bytes = serde_json::to_vec(&header).unwrap();
        let mut mutated = Vec::with_capacity(12 + header_bytes.len() + 8);
        mutated.extend_from_slice(MAGIC);
        mutated.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
        mutated.extend_from_slice(&header_bytes);
        // Preserve only the original four-pixel payload.
        mutated.extend_from_slice(&bytes[12 + header_len..]);
        std::fs::write(path, mutated).unwrap();

        assert!(matches!(
            cache.load(key),
            Err(CacheError::Corrupt("file length does not match header"))
        ));
    }

    #[test]
    fn invalid_dimensions_are_rejected_before_payload_allocation() {
        let directory = tempdir().unwrap();
        let cache = DiskMosaicCache::new(directory.path());
        let fingerprint = SourceFingerprint {
            file_size: 42,
            modified_ns: 7,
            sampled_blake3: [3; 32],
        };
        let key = CacheKey::for_mosaic(&fingerprint, 0);
        let path = cache.store(key, &test_mosaic()).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut header: serde_json::Value = serde_json::from_slice(&bytes[12..12 + header_len]).unwrap();

        // An absurd scalar width must fail the pixel-count consistency check;
        // it must not become a Vec length or trigger a large allocation.
        header["metadata"]["width"] = serde_json::Value::from(u32::MAX);
        let header_bytes = serde_json::to_vec(&header).unwrap();
        let mut mutated = Vec::with_capacity(12 + header_bytes.len() + bytes.len() - 12 - header_len);
        mutated.extend_from_slice(MAGIC);
        mutated.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
        mutated.extend_from_slice(&header_bytes);
        mutated.extend_from_slice(&bytes[12 + header_len..]);
        std::fs::write(&path, &mutated).unwrap();
        assert!(matches!(
            cache.load(key),
            Err(super::CacheError::Corrupt(
                "pixel count is inconsistent or exceeds cache limit"
            ))
        ));

        // Zero dimensions are rejected by RawMetadata::validate before the
        // payload-size calculation, with only the tiny test frame live.
        header["metadata"]["width"] = serde_json::Value::from(0_u32);
        let header_bytes = serde_json::to_vec(&header).unwrap();
        let mut mutated = Vec::with_capacity(12 + header_bytes.len() + bytes.len() - 12 - header_len);
        mutated.extend_from_slice(MAGIC);
        mutated.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
        mutated.extend_from_slice(&header_bytes);
        mutated.extend_from_slice(&bytes[12 + header_len..]);
        std::fs::write(path, mutated).unwrap();
        assert!(matches!(
            cache.load(key),
            Err(super::CacheError::InvalidFrame(
                rrrah_core::FrameError::EmptyFrame
            ))
        ));
    }

    #[test]
    fn trailing_bytes_are_rejected_by_file_length() {
        let directory = tempdir().unwrap();
        let cache = DiskMosaicCache::new(directory.path());
        let fingerprint = SourceFingerprint {
            file_size: 42,
            modified_ns: 7,
            sampled_blake3: [3; 32],
        };
        let key = CacheKey::for_mosaic(&fingerprint, 0);
        let path = cache.store(key, &test_mosaic()).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.push(0xA5);
        std::fs::write(path, bytes).unwrap();
        assert!(matches!(
            cache.load(key),
            Err(super::CacheError::Corrupt("file length does not match header"))
        ));
    }

    #[test]
    fn source_fingerprint_is_deterministic_and_changes_when_sample_changes() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("sample.raw");
        std::fs::write(&path, b"0123456789abcdef").unwrap();
        let first = SourceFingerprint::from_path(&path).unwrap();
        assert_eq!(first, SourceFingerprint::from_path(&path).unwrap());
        std::fs::write(&path, b"0123456789ABCDEF").unwrap();
        let second = SourceFingerprint::from_path(&path).unwrap();
        assert_ne!(first.sampled_blake3, second.sampled_blake3);
        assert!(matches!(
            SourceFingerprint::from_path(directory.path()),
            Err(super::CacheError::NotAFile(_))
        ));
    }

    #[test]
    fn cache_keys_are_domain_separated_by_image_index() {
        let fingerprint = SourceFingerprint {
            file_size: 42,
            modified_ns: 7,
            sampled_blake3: [3; 32],
        };
        let first = CacheKey::for_mosaic(&fingerprint, 0);
        let second = CacheKey::for_mosaic(&fingerprint, 1);
        assert_ne!(first, second);
        assert_eq!(first.to_hex().len(), 64);
    }
}
