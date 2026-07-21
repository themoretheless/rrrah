use std::{
    fmt,
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    mem::size_of,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use rrrah_core::{DECODE_CACHE_ABI, DecodedMosaic, FrameError, RawMetadata};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

const MAGIC: &[u8; 8] = b"RRRAHRC1";
const MAX_HEADER_BYTES: usize = 1 << 20;
const MAX_CACHED_PIXELS: u64 = 250_000_000;
const SAMPLE_BYTES: u64 = 64 * 1024;

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
        hasher.update(&DECODE_CACHE_ABI.to_le_bytes());
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

#[derive(Debug, Clone)]
pub struct DiskMosaicCache {
    root: PathBuf,
}

impl DiskMosaicCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn load(&self, key: CacheKey) -> Result<Option<CacheLoad>, CacheError> {
        let started = Instant::now();
        let path = self.path_for(key);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(CacheError::Io { path, source }),
        };
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
        if header.schema != DECODE_CACHE_ABI || header.key != key {
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
        let mut payload = vec![0_u8; payload_bytes];
        reader.read_exact(&mut payload).map_err(|source| CacheError::Io {
            path: path.clone(),
            source,
        })?;
        if *blake3::hash(&payload).as_bytes() != header.payload_blake3 {
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
        let pixels: Vec<u16> = payload
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect();
        let mosaic = DecodedMosaic::new(header.metadata, Arc::from(pixels.into_boxed_slice()))?;
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

        let mut payload = Vec::with_capacity(mosaic.byte_len());
        for &sample in mosaic.pixels.iter() {
            payload.extend_from_slice(&sample.to_le_bytes());
        }
        let header = CacheHeader {
            schema: DECODE_CACHE_ABI,
            key,
            pixel_count,
            payload_blake3: *blake3::hash(&payload).as_bytes(),
            metadata: mosaic.metadata.clone(),
        };
        let header_bytes = serde_json::to_vec(&header)?;
        if header_bytes.len() > MAX_HEADER_BYTES {
            return Err(CacheError::Corrupt("serialized header is too large"));
        }

        let mut temporary = NamedTempFile::new_in(parent).map_err(|source| CacheError::Io {
            path: parent.to_owned(),
            source,
        })?;
        {
            let mut writer = BufWriter::new(&mut temporary);
            writer.write_all(MAGIC)?;
            writer.write_all(&(header_bytes.len() as u32).to_le_bytes())?;
            writer.write_all(&header_bytes)?;
            writer.write_all(&payload)?;
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
        Ok(path)
    }

    fn path_for(&self, key: CacheKey) -> PathBuf {
        let hex = key.to_hex();
        self.root.join(&hex[..2]).join(format!("{hex}.rrc"))
    }
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
    #[error("decoded frame has {pixels} pixels, exceeding cache limit {max}")]
    FrameTooLarge { pixels: u64, max: u64 },
    #[error("corrupt cache entry: {0}")]
    Corrupt(&'static str),
    #[error("invalid cached frame: {0}")]
    InvalidFrame(#[from] FrameError),
    #[error("cache header serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("cache I/O failed: {0}")]
    PlainIo(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rrrah_core::{
        CfaColor, CfaPattern, DecodedMosaic, LevelGrid, Orientation, Photometric, RawMetadata, WhiteLevel,
    };
    use tempfile::tempdir;

    use super::{
        CacheKey, DiskMosaicCache, MAGIC, MAX_CACHED_PIXELS, MAX_HEADER_BYTES, SourceFingerprint,
        checked_payload_bytes,
    };

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
        DecodedMosaic::new(metadata, Arc::from([512_u16, 1024, 2048, 4096])).unwrap()
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
    fn trailing_bytes_are_rejected_even_when_payload_checksum_matches() {
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
            Err(super::CacheError::Corrupt("trailing bytes"))
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
