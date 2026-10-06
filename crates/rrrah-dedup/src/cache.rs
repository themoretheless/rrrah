//! Bounded, versioned fingerprint persistence. Paths/timestamps alone never key
//! reuse: the caller must supply a fresh full-file digest and complete recipe id.

use crate::pixels::Fingerprint;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
};
const MAGIC: &[u8; 8] = b"RRDUPC01";
const RECORD_BYTES: usize = 360;

/// Recipe digest must cover decoder/color versions, orientation, compositing,
/// fingerprint algorithm/options and frame-selection policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CacheKey {
    pub content: [u8; 32],
    pub recipe: [u8; 32],
    pub frame_index: u64,
}

#[derive(Debug, Default)]
pub struct FingerprintCache {
    entries: BTreeMap<CacheKey, Fingerprint>,
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("unsupported, corrupt or invalid fingerprint cache")]
    Invalid,
    #[error("fingerprint cache exceeds supplied entry budget")]
    Budget,
    #[error("cache published, but parent directory synchronization failed: {0}")]
    PublicationSync(std::io::Error),
    #[error("fingerprint cache operation cancelled")]
    Cancelled,
}

fn valid(fp: &Fingerprint) -> bool {
    fp.mean_linear_rgb.iter().all(|v| v.is_finite())
        && fp.luminance_stddev.is_finite()
        && fp.luminance_stddev >= 0.0
}

impl FingerprintCache {
    /// Publish a completed cache using a temporary file in the destination
    /// directory and an atomic replacement. The file contents are synchronized
    /// before replacement. On Unix, successful publication also synchronizes
    /// the parent directory; actual power-loss durability remains filesystem-dependent.
    /// Other platforms currently do not synchronize the directory.
    /// Concurrent successful writers use last-published-writer semantics.
    /// Abrupt process exit can leave an incomplete temporary file in the parent
    /// directory; only the published destination is loaded as the cache.
    ///
    /// # Errors
    /// I/O/cancellation before publication leaves the previous destination
    /// untouched. `PublicationSync` means replacement already occurred, but its
    /// directory durability could not be confirmed. Pre-publication failures
    /// remove temporary files.
    pub fn save_atomic(
        &self,
        path: impl AsRef<std::path::Path>,
        cancel: impl Fn() -> bool,
    ) -> Result<(), CacheError> {
        self.save_atomic_with_sync(path, cancel, |parent| {
            #[cfg(unix)]
            {
                std::fs::File::open(parent)?.sync_all()
            }
            #[cfg(not(unix))]
            {
                let _ = parent;
                Ok(())
            }
        })
    }

    fn save_atomic_with_sync(
        &self,
        path: impl AsRef<std::path::Path>,
        cancel: impl Fn() -> bool,
        sync_parent: impl Fn(&std::path::Path) -> std::io::Result<()>,
    ) -> Result<(), CacheError> {
        if cancel() {
            return Err(CacheError::Cancelled);
        }
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        self.write(temporary.as_file_mut(), &cancel)?;
        temporary.as_file_mut().flush()?;
        temporary.as_file().sync_all()?;
        if cancel() {
            return Err(CacheError::Cancelled);
        }
        temporary
            .persist(path)
            .map_err(|failure| CacheError::Io(failure.error))?;
        sync_parent(parent).map_err(CacheError::PublicationSync)?;
        Ok(())
    }

    /// # Errors
    /// Returns missing/corrupt/over-budget cache or cancellation errors.
    pub fn load_file(
        path: impl AsRef<std::path::Path>,
        max_entries: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, CacheError> {
        if cancel() {
            return Err(CacheError::Cancelled);
        }
        Self::read(std::fs::File::open(path)?, max_entries, cancel)
    }

    pub fn get(&self, key: &CacheKey) -> Option<&Fingerprint> {
        self.entries.get(key)
    }

    /// # Errors
    /// Rejects invalid values or entry-budget excess. Replacing an existing key
    /// does not consume another slot.
    pub fn insert(
        &mut self,
        key: CacheKey,
        fingerprint: Fingerprint,
        max_entries: usize,
    ) -> Result<(), CacheError> {
        if !valid(&fingerprint) {
            return Err(CacheError::Invalid);
        }
        if !self.entries.contains_key(&key) && self.entries.len() >= max_entries {
            return Err(CacheError::Budget);
        }
        self.entries.insert(key, fingerprint);
        Ok(())
    }

    /// Write deterministic binary records and an end-to-end BLAKE3 checksum.
    /// Caller must publish a successfully written temporary file atomically;
    /// cancellation/I/O failures may leave a partial destination stream.
    ///
    /// # Errors
    /// Returns I/O, count overflow or cancellation errors.
    pub fn write(&self, mut output: impl Write, cancel: impl Fn() -> bool) -> Result<(), CacheError> {
        let count = u64::try_from(self.entries.len()).map_err(|_| CacheError::Budget)?;
        let mut hash = blake3::Hasher::new();
        for header in [MAGIC.as_slice(), count.to_le_bytes().as_slice()] {
            output.write_all(header)?;
            hash.update(header);
        }
        for (key, fp) in &self.entries {
            if cancel() {
                return Err(CacheError::Cancelled);
            }
            let mut record = Vec::with_capacity(RECORD_BYTES);
            record.extend_from_slice(&key.content);
            record.extend_from_slice(&key.recipe);
            record.extend_from_slice(&key.frame_index.to_le_bytes());
            for variant in fp.variants {
                for value in variant {
                    record.extend_from_slice(&value.to_le_bytes());
                }
            }
            for value in fp.mean_linear_rgb.into_iter().chain([fp.luminance_stddev]) {
                record.extend_from_slice(&value.to_bits().to_le_bytes());
            }
            output.write_all(&record)?;
            hash.update(&record);
        }
        if cancel() {
            return Err(CacheError::Cancelled);
        }
        output.write_all(hash.finalize().as_bytes())?;
        Ok(())
    }

    /// Read a complete checksummed cache under an entry budget. Reject trailing
    /// bytes, truncation, duplicate keys and invalid floating-point values.
    ///
    /// # Errors
    /// Returns corrupt/version/budget/cancellation/I/O errors without partial data.
    pub fn read(
        mut input: impl Read,
        max_entries: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, CacheError> {
        if cancel() {
            return Err(CacheError::Cancelled);
        }
        let mut header = [0_u8; 16];
        input.read_exact(&mut header)?;
        if &header[..8] != MAGIC {
            return Err(CacheError::Invalid);
        }
        let count = u64::from_le_bytes(header[8..].try_into().map_err(|_| CacheError::Invalid)?);
        if count > u64::try_from(max_entries).map_err(|_| CacheError::Budget)? {
            return Err(CacheError::Budget);
        }
        let mut hash = blake3::Hasher::new();
        hash.update(&header);
        let mut cache = Self::default();
        for _ in 0..count {
            if cancel() {
                return Err(CacheError::Cancelled);
            }
            let mut record = [0_u8; RECORD_BYTES];
            input.read_exact(&mut record)?;
            hash.update(&record);
            let key = CacheKey {
                content: record[..32].try_into().map_err(|_| CacheError::Invalid)?,
                recipe: record[32..64].try_into().map_err(|_| CacheError::Invalid)?,
                frame_index: word(&record[64..72])?,
            };
            if cache.entries.contains_key(&key) {
                return Err(CacheError::Invalid);
            }
            let mut words = record[72..328].as_chunks::<8>().0.iter();
            let mut variants = [[0; 4]; 8];
            for variant in &mut variants {
                for value in variant {
                    *value = word(words.next().ok_or(CacheError::Invalid)?)?;
                }
            }
            let mut values = [0.0; 4];
            for (value, bytes) in values.iter_mut().zip(record[328..].as_chunks::<8>().0) {
                *value = f64::from_bits(word(bytes)?);
            }
            cache.insert(
                key,
                Fingerprint {
                    variants,
                    mean_linear_rgb: [values[0], values[1], values[2]],
                    luminance_stddev: values[3],
                },
                max_entries,
            )?;
        }
        let mut checksum = [0; 32];
        input.read_exact(&mut checksum)?;
        if hash.finalize().as_bytes() != &checksum {
            return Err(CacheError::Invalid);
        }
        let mut trailing = [0; 1];
        if input.read(&mut trailing)? != 0 {
            return Err(CacheError::Invalid);
        }
        if cancel() {
            return Err(CacheError::Cancelled);
        }
        Ok(cache)
    }
}

fn word(bytes: &[u8]) -> Result<u64, CacheError> {
    Ok(u64::from_le_bytes(
        bytes.try_into().map_err(|_| CacheError::Invalid)?,
    ))
}

#[cfg(test)]
mod publication_tests {
    use super::*;

    #[test]
    fn directory_sync_failure_reports_already_published_complete_cache() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("cache.bin");
        std::fs::write(&path, b"previous-publication").unwrap();
        let cache = FingerprintCache::default();
        let outcome = cache.save_atomic_with_sync(
            &path,
            || false,
            |parent| {
                assert_eq!(parent, folder.path());
                // Publication must precede directory sync; independently load the file.
                FingerprintCache::load_file(&path, 0, || false).unwrap();
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "injected directory sync failure",
                ))
            },
        );
        assert!(
            matches!(outcome, Err(CacheError::PublicationSync(error)) if error.kind() == std::io::ErrorKind::PermissionDenied)
        );
        FingerprintCache::load_file(&path, 0, || false).unwrap();
        assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 1);
        cache.save_atomic(&path, || false).unwrap();
        FingerprintCache::load_file(&path, 0, || false).unwrap();
    }
}
