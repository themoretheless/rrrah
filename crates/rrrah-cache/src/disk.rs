use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    mem::{size_of, size_of_val},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use rrrah_core::{DecodedMosaic, FrameError, LEGACY_V2_CACHE_ABI, MosaicRecipeManifest, RawMetadata};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

const MAGIC: &[u8; 8] = b"RRRAHRC1";
const MAX_HEADER_BYTES: usize = 1 << 20;
const MAX_CACHED_PIXELS: u64 = 250_000_000;
const SAMPLE_BYTES: u64 = 64 * 1024;
const PAYLOAD_BUFFER_BYTES: usize = 16 * 1024;
const WRITE_LOCK_FILE: &str = ".rrrah-cache-write.lock";
const RECIPE_AWARE_MOSAIC_KEY_NAMESPACE_V1: &[u8] = b"rrrah/decoded-mosaic-recipe/v1\0";

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
    /// Derives the frozen legacy V2 key. New producers should use
    /// [`Self::for_mosaic_recipe`] so decoder semantics participate in cache
    /// identity.
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

    /// Derives a recipe-aware key for the transitional sampled-fingerprint
    /// disk cache.
    ///
    /// This is intentionally a distinct namespace from both legacy V2 keys
    /// and the full-content [`crate::MosaicKey`] protocol. The complete
    /// canonical recipe manifest participates directly in the transcript, so
    /// independent decoder backends and contract revisions cannot reuse one
    /// another's decoded mosaics.
    ///
    /// # Panics
    ///
    /// Panics if `usize` is wider than 64 bits and `image_index` does not fit
    /// the protocol's fixed-width `u64` field.
    pub fn for_mosaic_recipe(
        source: &SourceFingerprint,
        image_index: usize,
        recipe: MosaicRecipeManifest,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(RECIPE_AWARE_MOSAIC_KEY_NAMESPACE_V1);
        hasher.update(&source.file_size.to_le_bytes());
        hasher.update(&source.modified_ns.to_le_bytes());
        hasher.update(&source.sampled_blake3);
        hasher.update(
            &u64::try_from(image_index)
                .expect("supported target image index must fit in u64")
                .to_le_bytes(),
        );
        hasher.update(&recipe.canonical_bytes());
        Self(*hasher.finalize().as_bytes())
    }

    pub fn to_hex(self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// Deterministic key material for unit tests that only need distinct keys.
    #[cfg(test)]
    pub(crate) fn from_bytes_for_test(byte: u8) -> Self {
        Self([byte; 32])
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
    max_entries: Option<usize>,
    ttl: Option<Duration>,
}

impl DiskMosaicCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_max_bytes(root, DEFAULT_MAX_DISK_CACHE_BYTES)
    }

    pub fn with_max_bytes(root: impl Into<PathBuf>, max_bytes: u64) -> Self {
        Self {
            root: root.into(),
            max_bytes,
            max_entries: None,
            ttl: None,
        }
    }

    /// Sets an independent object-count limit. Zero rejects every store.
    pub fn with_max_entries(mut self, max_entries: Option<usize>) -> Self {
        self.max_entries = max_entries;
        self
    }

    /// Lifetime since the cache file's last write; reads do not renew it.
    pub fn with_ttl(mut self, ttl: Option<Duration>) -> Self {
        self.ttl = ttl;
        self
    }

    fn expired(&self, metadata: &fs::Metadata, now: SystemTime) -> bool {
        self.ttl.is_some_and(|ttl| {
            metadata
                .modified()
                .ok()
                .and_then(|written| now.duration_since(written).ok())
                .is_none_or(|age| age >= ttl)
        })
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
        fs::metadata(self.path_for(key))
            .is_ok_and(|metadata| metadata.is_file() && !self.expired(&metadata, SystemTime::now()))
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
        self.load_impl(key, None, || false)
    }

    /// Loads pixels under a shared budget, reserving before allocation.
    pub fn load_with_budget(
        &self,
        key: CacheKey,
        budget: &rrrah_memory::MemoryBudget,
    ) -> Result<Option<CacheLoad>, CacheError> {
        self.load_impl(key, Some(budget), || false)
    }

    /// Loads with block-boundary cancellation and optional shared pixel accounting.
    pub fn load_with_cancel(
        &self,
        key: CacheKey,
        budget: Option<&rrrah_memory::MemoryBudget>,
        cancelled: impl FnMut() -> bool,
    ) -> Result<Option<CacheLoad>, CacheError> {
        self.load_impl(key, budget, cancelled)
    }

    fn load_impl(
        &self,
        key: CacheKey,
        budget: Option<&rrrah_memory::MemoryBudget>,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Option<CacheLoad>, CacheError> {
        if cancelled() {
            return Err(CacheError::Cancelled);
        }
        let started = Instant::now();
        let path = self.path_for(key);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(CacheError::Io { path, source }),
        };
        let metadata = file.metadata().map_err(|source| CacheError::Io {
            path: path.clone(),
            source,
        })?;
        if self.expired(&metadata, SystemTime::now()) {
            return Ok(None);
        }
        let file_bytes = metadata.len();
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
        if cancelled() {
            return Err(CacheError::Cancelled);
        }
        let pixel_count = usize::try_from(header.pixel_count).map_err(|_| CacheError::SizeOverflow)?;
        let mut payload_hasher = blake3::Hasher::new();
        let pixels: rrrah_memory::PixelBuffer<u16> = if let Some(budget) = budget {
            let mut pixels = budget.try_buffer(pixel_count, 0_u16)?;
            for samples in pixels.chunks_mut(PAYLOAD_BUFFER_BYTES / 2) {
                if cancelled() {
                    return Err(CacheError::Cancelled);
                }
                let bytes = bytemuck::cast_slice_mut(samples);
                reader.read_exact(bytes).map_err(|source| CacheError::Io {
                    path: path.clone(),
                    source,
                })?;
                payload_hasher.update(bytes);
                #[cfg(target_endian = "big")]
                for sample in samples {
                    *sample = u16::from_le(*sample);
                }
            }
            pixels.freeze().into()
        } else {
            let mut pixels = Vec::new();
            pixels
                .try_reserve_exact(pixel_count)
                .map_err(|_| CacheError::AllocationFailed {
                    bytes: u64::try_from(payload_bytes).unwrap_or(u64::MAX),
                })?;
            let mut buffer = [0_u8; PAYLOAD_BUFFER_BYTES];
            let mut remaining = payload_bytes;
            while remaining != 0 {
                if cancelled() {
                    return Err(CacheError::Cancelled);
                }
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
            Arc::new(pixels).into()
        };
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
        if cancelled() {
            return Err(CacheError::Cancelled);
        }
        let mosaic = DecodedMosaic::new(header.metadata, pixels)?;
        Ok(Some(CacheLoad {
            mosaic,
            elapsed: started.elapsed(),
        }))
    }

    pub fn store(&self, key: CacheKey, mosaic: &DecodedMosaic) -> Result<PathBuf, CacheError> {
        self.store_with_cancel(key, mosaic, || false)
    }

    pub fn store_with_cancel(
        &self,
        key: CacheKey,
        mosaic: &DecodedMosaic,
        cancelled: impl FnMut() -> bool,
    ) -> Result<PathBuf, CacheError> {
        self.store_with_cancel_preserving(key, mosaic, &[], cancelled)
    }

    /// Protect higher-priority residents from eviction by this write.
    /// Admission and pruning share the publication lock. This protection is
    /// scoped to this operation; unrelated writers may still evict the keys.
    pub fn store_with_cancel_preserving(
        &self,
        key: CacheKey,
        mosaic: &DecodedMosaic,
        protected: &[CacheKey],
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<PathBuf, CacheError> {
        if cancelled() {
            return Err(CacheError::Cancelled);
        }
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
            payload_blake3: hash_pixels_with_cancel(&mosaic.pixels, &mut cancelled)?,
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
        if cancelled() {
            return Err(CacheError::Cancelled);
        }
        let write_lock = self.lock_writes(&mut cancelled)?;
        if cancelled() {
            return Err(CacheError::Cancelled);
        }
        let evictions = self.plan_write_evictions(&path, entry_bytes, protected)?;

        let mut temporary = NamedTempFile::new_in(parent).map_err(|source| CacheError::Io {
            path: parent.to_owned(),
            source,
        })?;
        {
            let mut writer = BufWriter::new(&mut temporary);
            writer.write_all(MAGIC)?;
            writer.write_all(&(header_bytes.len() as u32).to_le_bytes())?;
            writer.write_all(&header_bytes)?;
            for samples in mosaic.pixels.chunks(PAYLOAD_BUFFER_BYTES / 2) {
                if cancelled() {
                    return Err(CacheError::Cancelled);
                }
                write_pixels(&mut writer, samples)?;
            }
            writer.flush()?;
        }
        if cancelled() {
            return Err(CacheError::Cancelled);
        }
        temporary.as_file().sync_data()?;
        if cancelled() {
            return Err(CacheError::Cancelled);
        }
        // Commit starts after the final cancellation check. Until now both
        // live and expired residents remain untouched. Keep eviction and
        // publication together under the writer lock without cancellation gaps.
        for victim in evictions {
            match fs::remove_file(&victim) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => return Err(CacheError::Io { path: victim, source }),
            }
        }
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

    fn lock_writes(&self, mut cancelled: impl FnMut() -> bool) -> Result<File, CacheError> {
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
        loop {
            if cancelled() {
                return Err(CacheError::Cancelled);
            }
            match lock.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                Err(std::fs::TryLockError::Error(source)) => {
                    return Err(CacheError::Io {
                        path: lock_path,
                        source,
                    });
                }
            }
        }
        Ok(lock)
    }

    fn plan_write_evictions(
        &self,
        destination: &Path,
        incoming_bytes: u64,
        protected: &[CacheKey],
    ) -> Result<Vec<PathBuf>, CacheError> {
        let mut evictions = Vec::new();
        if self.max_entries == Some(0) {
            return Err(CacheError::DiskCountExceeded { limit: 0 });
        }
        if incoming_bytes > self.max_bytes {
            return Err(CacheError::DiskBudgetExceeded {
                incoming: incoming_bytes,
                limit: self.max_bytes,
            });
        }

        let now = SystemTime::now();
        let mut resident = 0_u64;
        let protected_paths: std::collections::HashSet<_> =
            protected.iter().map(|key| self.path_for(*key)).collect();
        let mut protected_bytes = 0_u64;
        let mut protected_count = 0_usize;
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
                if self.expired(&metadata, now) {
                    evictions.push(entry_path);
                    continue;
                }
                resident = resident
                    .checked_add(metadata.len())
                    .ok_or(CacheError::SizeOverflow)?;
                if protected_paths.contains(&entry_path) {
                    protected_bytes = protected_bytes
                        .checked_add(metadata.len())
                        .ok_or(CacheError::SizeOverflow)?;
                    protected_count += 1;
                    continue;
                }
                let age = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map_or(0, |duration| duration.as_nanos());
                candidates.push((age, entry_path, metadata.len()));
            }
        }

        // Reject an impossible admission before deleting any live candidate.
        let required = protected_bytes
            .checked_add(incoming_bytes)
            .ok_or(CacheError::SizeOverflow)?;
        if required > self.max_bytes {
            return Err(CacheError::DiskBudgetExceeded {
                incoming: required,
                limit: self.max_bytes,
            });
        }
        if let Some(limit) = self.max_entries {
            if protected_count >= limit {
                return Err(CacheError::DiskCountExceeded { limit });
            }
        }
        let mut remaining_entries = candidates.len() + protected_count;
        candidates.sort_unstable_by_key(|(age, _, _)| *age);
        for (_, candidate, bytes) in candidates {
            if resident.saturating_add(incoming_bytes) <= self.max_bytes
                && self.max_entries.is_none_or(|limit| remaining_entries < limit)
            {
                break;
            }
            evictions.push(candidate);
            resident = resident.saturating_sub(bytes);
            remaining_entries = remaining_entries.saturating_sub(1);
        }
        if resident.saturating_add(incoming_bytes) > self.max_bytes {
            return Err(CacheError::DiskBudgetExceeded {
                incoming: incoming_bytes,
                limit: self.max_bytes,
            });
        }
        if let Some(limit) = self.max_entries {
            if remaining_entries >= limit {
                return Err(CacheError::DiskCountExceeded { limit });
            }
        }
        Ok(evictions)
    }

    fn path_for(&self, key: CacheKey) -> PathBuf {
        let hex = key.to_hex();
        self.root.join(&hex[..2]).join(format!("{hex}.rrc"))
    }
}

#[cfg(test)]
fn hash_pixels(pixels: &[u16]) -> [u8; 32] {
    hash_pixels_with_cancel(pixels, &mut || false).unwrap()
}

fn hash_pixels_with_cancel(
    pixels: &[u16],
    cancelled: &mut impl FnMut() -> bool,
) -> Result<[u8; 32], CacheError> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; PAYLOAD_BUFFER_BYTES];
    for samples in pixels.chunks(PAYLOAD_BUFFER_BYTES / size_of::<u16>()) {
        if cancelled() {
            return Err(CacheError::Cancelled);
        }
        let bytes = &mut buffer[..size_of_val(samples)];
        for (sample, output) in samples.iter().zip(bytes.chunks_exact_mut(2)) {
            output.copy_from_slice(&sample.to_le_bytes());
        }
        hasher.update(bytes);
    }
    Ok(*hasher.finalize().as_bytes())
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
    #[error("disk cache entry count exceeds limit {limit}")]
    DiskCountExceeded { limit: usize },
    #[error("cache read cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] rrrah_memory::BufferError),
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
        CfaColor, CfaPattern, DecodedMosaic, KNOWN_MOSAIC_DECODE_FLAGS, LevelGrid, MosaicRecipeManifest,
        Orientation, Photometric, RawMetadata, WhiteLevel,
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
    fn protected_keys_do_not_override_ttl_expiration() {
        let directory = tempdir().unwrap();
        let protected = test_key(51);
        let incoming = test_key(52);
        let cache = DiskMosaicCache::new(directory.path())
            .with_max_entries(Some(1))
            .with_ttl(Some(std::time::Duration::from_secs(10)));
        let path = cache.store(protected, &test_mosaic()).unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
            .unwrap();
        cache
            .store_with_cancel_preserving(incoming, &test_mosaic(), &[protected], || false)
            .unwrap();
        assert!(!path.exists());
        assert!(cache.load(incoming).unwrap().is_some());
        assert_eq!(cache.usage().unwrap().entries, 1);
    }

    #[test]
    fn cancelled_protected_write_keeps_residents_and_releases_write_lock() {
        let directory = tempdir().unwrap();
        let protected = test_key(61);
        let incoming = test_key(62);
        let cache = DiskMosaicCache::new(directory.path()).with_max_entries(Some(1));
        cache.store(protected, &test_mosaic()).unwrap();
        // Cover cancellation before hashing and at the pre-publication lock
        // boundaries without relying on worker timing.
        for cancel_at in 1..=4 {
            let mut calls = 0;
            assert!(matches!(
                cache.store_with_cancel_preserving(incoming, &test_mosaic(), &[protected], || {
                    calls += 1;
                    calls >= cancel_at
                }),
                Err(CacheError::Cancelled)
            ));
            assert!(cache.load(protected).unwrap().is_some());
            assert!(!cache.contains(incoming));
            assert_eq!(cache.usage().unwrap().entries, 1);
            drop(cache.lock_writes(|| false).unwrap());
        }
    }

    #[test]
    fn protected_admission_rejects_without_evicting_any_live_entry() {
        for count_limited in [false, true] {
            let directory = tempdir().unwrap();
            let setup = DiskMosaicCache::with_max_bytes(directory.path(), u64::MAX);
            let protected = test_key(31);
            let other = test_key(32);
            let incoming = test_key(33);
            let path = setup.store(protected, &test_mosaic()).unwrap();
            let bytes = std::fs::metadata(path).unwrap().len();
            setup.store(other, &test_mosaic()).unwrap();
            let bounded = if count_limited {
                setup.with_max_entries(Some(1))
            } else {
                DiskMosaicCache::with_max_bytes(directory.path(), bytes)
            };
            let error = bounded
                .store_with_cancel_preserving(
                    incoming,
                    &test_mosaic(),
                    &[protected, protected, test_key(99)],
                    || false,
                )
                .unwrap_err();
            if count_limited {
                assert!(matches!(error, CacheError::DiskCountExceeded { .. }));
            } else {
                assert!(matches!(error, CacheError::DiskBudgetExceeded { .. }));
            }
            assert!(bounded.load(protected).unwrap().is_some());
            assert!(bounded.load(other).unwrap().is_some());
            assert!(!bounded.contains(incoming));
        }
    }

    #[test]
    fn protected_admission_evicts_only_unprotected_residents_and_allows_replacement() {
        let directory = tempdir().unwrap();
        let setup = DiskMosaicCache::with_max_bytes(directory.path(), u64::MAX);
        let protected = test_key(41);
        let other = test_key(42);
        let incoming = test_key(43);
        setup.store(protected, &test_mosaic()).unwrap();
        setup.store(other, &test_mosaic()).unwrap();
        let bounded = setup.with_max_entries(Some(2));
        bounded
            .store_with_cancel_preserving(incoming, &test_mosaic(), &[protected], || false)
            .unwrap();
        assert!(bounded.load(protected).unwrap().is_some());
        assert!(bounded.load(incoming).unwrap().is_some());
        assert!(!bounded.contains(other));
        bounded
            .store_with_cancel_preserving(protected, &test_mosaic(), &[protected, incoming], || false)
            .unwrap();
        assert_eq!(bounded.usage().unwrap().entries, 2);
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
        second.pixels = Arc::new(vec![513_u16, 1025, 2049, 4097]).into();
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

    #[test]
    fn recipe_aware_keys_separate_independent_backends() {
        let fingerprint = SourceFingerprint {
            file_size: 42,
            modified_ns: 7,
            sampled_blake3: [3; 32],
        };
        let backend_a = MosaicRecipeManifest::new(1, 1, 1, 1, KNOWN_MOSAIC_DECODE_FLAGS, [0x5a; 32]);
        let backend_b = MosaicRecipeManifest::new(2, 1, 1, 1, KNOWN_MOSAIC_DECODE_FLAGS, [0x5a; 32]);

        let first_key = CacheKey::for_mosaic_recipe(&fingerprint, 0, backend_a);
        let second_key = CacheKey::for_mosaic_recipe(&fingerprint, 0, backend_b);
        assert_ne!(first_key, second_key);
        assert_ne!(first_key, CacheKey::for_mosaic(&fingerprint, 0));
        assert_eq!(
            first_key.to_hex(),
            "703333e41325180a910b63e8abf2f19d03bb44ec9889661a3399a6fab9588c47"
        );

        let mut expected = blake3::Hasher::new();
        expected.update(super::RECIPE_AWARE_MOSAIC_KEY_NAMESPACE_V1);
        expected.update(&fingerprint.file_size.to_le_bytes());
        expected.update(&fingerprint.modified_ns.to_le_bytes());
        expected.update(&fingerprint.sampled_blake3);
        expected.update(&0_u64.to_le_bytes());
        expected.update(&backend_a.canonical_bytes());
        assert_eq!(first_key.0, *expected.finalize().as_bytes());
    }

    #[test]
    fn legacy_v2_key_derivation_remains_frozen() {
        let fingerprint = SourceFingerprint {
            file_size: 42,
            modified_ns: 7,
            sampled_blake3: [3; 32],
        };

        assert_eq!(
            CacheKey::for_mosaic(&fingerprint, 0).to_hex(),
            "73bc0c7f827b24f32dca2194ca5b186f7eba42a435bd5da33d8dc894e3ae7d88"
        );
    }
    #[test]
    fn budgeted_disk_reads_preserve_owners_and_release_corrupt_allocations() {
        let parent = tempdir().unwrap();
        let cache = DiskMosaicCache::new(parent.path());
        let key = test_key(91);
        let source = test_mosaic();
        let path = cache.store(key, &source).unwrap();
        let bytes = source.byte_len() as u64;
        let shared = rrrah_memory::MemoryBudget::new(bytes);
        let budget = shared.child(bytes * 2);
        let hit = cache.load_with_budget(key, &budget).unwrap().unwrap();
        assert!(hit.mosaic.pixels.is_managed());
        assert_eq!(hit.mosaic.pixels, source.pixels);
        assert_eq!(hit.mosaic.metadata, source.metadata);
        let owner = hit.mosaic.clone();
        drop(hit);
        assert_eq!(budget.used(), bytes);
        assert_eq!(shared.used(), bytes);
        assert!(shared.child(bytes).try_reserve(1).is_err());
        assert!(matches!(
            cache.load_with_budget(key, &budget),
            Err(CacheError::Memory(_))
        ));
        drop(owner);
        assert_eq!(budget.used(), 0);
        assert_eq!(shared.used(), 0);
        drop(cache.load_with_budget(key, &budget).unwrap().unwrap());
        assert_eq!(budget.used(), 0);
        assert_eq!(shared.used(), 0);
        let mut encoded = std::fs::read(&path).unwrap();
        *encoded.last_mut().unwrap() ^= 1;
        std::fs::write(&path, encoded).unwrap();
        assert!(matches!(
            cache.load_with_budget(key, &budget),
            Err(CacheError::Corrupt(_))
        ));
        assert_eq!(budget.used(), 0);
        assert_eq!(shared.used(), 0);
    }
    #[test]
    fn cancelled_disk_reads_do_not_allocate_or_publish_and_allow_retry() {
        let parent = tempdir().unwrap();
        let cache = DiskMosaicCache::new(parent.path());
        let key = test_key(92);
        let original = test_mosaic();
        cache.store(key, &original).unwrap();
        let bytes = original.byte_len() as u64;
        let budget = rrrah_memory::MemoryBudget::new(bytes);
        assert!(matches!(
            cache.load_with_cancel(key, Some(&budget), || true),
            Err(CacheError::Cancelled)
        ));
        assert_eq!(budget.peak(), 0);
        for cancel_at in [3, 4] {
            let mut checks = 0;
            assert!(matches!(
                cache.load_with_cancel(key, Some(&budget), || {
                    checks += 1;
                    checks >= cancel_at
                }),
                Err(CacheError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
        assert_eq!(budget.peak(), bytes);
        let hit = cache
            .load_with_cancel(key, Some(&budget), || false)
            .unwrap()
            .unwrap();
        assert_eq!(hit.mosaic.pixels, original.pixels);
        drop(hit);
        assert_eq!(budget.used(), 0);
        assert!(matches!(
            cache.load_with_cancel(key, None, || true),
            Err(CacheError::Cancelled)
        ));
    }
    #[test]
    fn cancelled_stores_preserve_existing_entry_and_remove_partial_files() {
        let parent = tempdir().unwrap();
        let cache = DiskMosaicCache::new(parent.path());
        let key = test_key(93);
        let original = test_mosaic();
        let path = cache.store(key, &original).unwrap();
        let before = std::fs::read(&path).unwrap();
        for cancel_at in 1..=7 {
            let mut checks = 0;
            assert!(matches!(
                cache.store_with_cancel(key, &original, || {
                    checks += 1;
                    checks >= cancel_at
                }),
                Err(CacheError::Cancelled)
            ));
            assert_eq!(std::fs::read(&path).unwrap(), before);
            assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        }
        cache.store_with_cancel(key, &original, || false).unwrap();
        assert_eq!(cache.load(key).unwrap().unwrap().mosaic.pixels, original.pixels);
    }
    #[test]
    fn cancelled_incoming_store_does_not_evict_live_resident() {
        for count_limit in [true, false] {
            let directory = tempdir().unwrap();
            let cache = DiskMosaicCache::new(directory.path());
            let resident = test_key(197);
            let incoming = test_key(198);
            cache.store(resident, &test_mosaic()).unwrap();
            let path = cache.path_for(resident);
            let original = std::fs::read(&path).unwrap();
            let cache = if count_limit {
                cache.with_max_entries(Some(1))
            } else {
                DiskMosaicCache::with_max_bytes(directory.path(), original.len() as u64)
            };
            for cancel_at in 1..=7 {
                let mut calls = 0;
                assert!(matches!(
                    cache.store_with_cancel(incoming, &test_mosaic(), || {
                        calls += 1;
                        calls >= cancel_at
                    }),
                    Err(CacheError::Cancelled)
                ));
                assert_eq!(
                    std::fs::read(&path).unwrap(),
                    original,
                    "cancel point {cancel_at} evicted resident"
                );
                assert!(!cache.contains(incoming));
                assert_eq!(cache.usage().unwrap().entries, 1);
            }
            cache.store(incoming, &test_mosaic()).unwrap();
            assert!(!cache.contains(resident));
            assert!(cache.contains(incoming));
            assert_eq!(cache.usage().unwrap().entries, 1);
        }
    }
    #[test]
    fn waiting_write_lock_can_cancel_without_releasing_current_writer() {
        let parent = tempdir().unwrap();
        let cache = DiskMosaicCache::new(parent.path());
        let held = cache.lock_writes(|| false).unwrap();
        let mut checks = 0;
        assert!(matches!(
            cache.lock_writes(|| {
                checks += 1;
                checks >= 2
            }),
            Err(CacheError::Cancelled)
        ));
        assert_eq!(checks, 2);
        drop(held);
        let next = cache.lock_writes(|| false).unwrap();
        drop(next);
    }
    #[test]
    fn disk_count_is_independent_and_replacement_does_not_count_twice() {
        let parent = tempdir().unwrap();
        let cache = DiskMosaicCache::with_max_bytes(parent.path(), u64::MAX).with_max_entries(Some(1));
        let original = test_mosaic();
        cache.store(test_key(94), &original).unwrap();
        cache.store(test_key(94), &original).unwrap();
        assert_eq!(cache.usage().unwrap().entries, 1);
        cache.store(test_key(95), &original).unwrap();
        assert!(cache.load(test_key(94)).unwrap().is_none());
        assert!(cache.load(test_key(95)).unwrap().is_some());
        assert_eq!(cache.usage().unwrap().entries, 1);
        let disabled = cache.clone().with_max_entries(Some(0));
        assert!(matches!(
            disabled.store(test_key(96), &original),
            Err(CacheError::DiskCountExceeded { limit: 0 })
        ));
        assert!(cache.load(test_key(95)).unwrap().is_some());
    }
    #[test]
    fn disk_ttl_uses_write_age_and_prunes_expired_entries_under_lock() {
        let parent = tempdir().unwrap();
        let cache = DiskMosaicCache::new(parent.path()).with_ttl(Some(std::time::Duration::from_secs(10)));
        let original = test_mosaic();
        let old = cache.store(test_key(97), &original).unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&old).unwrap();
        file.set_times(std::fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
            .unwrap();
        assert!(!cache.contains(test_key(97)));
        assert!(cache.load(test_key(97)).unwrap().is_none());
        assert!(old.exists());
        cache.store(test_key(98), &original).unwrap();
        assert!(!old.exists());
        assert!(cache.contains(test_key(98)));
        let metadata = std::fs::metadata(cache.path_for(test_key(98))).unwrap();
        let written = metadata.modified().unwrap();
        assert!(!cache.expired(&metadata, written + std::time::Duration::from_secs(9)));
        assert!(cache.expired(&metadata, written + std::time::Duration::from_secs(10)));
        cache.load(test_key(98)).unwrap().unwrap();
        assert_eq!(
            std::fs::metadata(cache.path_for(test_key(98)))
                .unwrap()
                .modified()
                .unwrap(),
            written
        );
        let zero = cache.clone().with_ttl(Some(std::time::Duration::ZERO));
        assert!(!zero.contains(test_key(98)));
        assert!(zero.load(test_key(98)).unwrap().is_none());
        let future = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
        let path = cache.path_for(test_key(98));
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(future))
            .unwrap();
        assert!(cache.load(test_key(98)).unwrap().is_none());
    }
}
