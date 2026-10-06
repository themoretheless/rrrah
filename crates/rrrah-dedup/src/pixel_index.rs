//! Exact selected-frame candidates indexed by canonical normalized pixels.
use crate::{
    decode::DecodeSourceSnapshot as ContentSnapshot,
    decode::{CachedError, decode_snapshot_frame},
    exact::SnapshotError,
    raster::{AdapterError, NormalizedRaster},
    scan::{FileIssue, PixelConfirmation, PixelSearchPolicy, ScanError, VisualPolicy, confirm_pixels},
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
};
#[derive(Debug)]
pub struct IndexedPixelReport {
    pub analysed: Vec<u64>,
    /// Only same-digest candidate pairs are compared. This is not all unequal pairs.
    pub pixels: PixelConfirmation,
    pub issues: Vec<FileIssue>,
    pub source_issues: Vec<(u64, SnapshotError)>,
    /// Successful key-producing decodes, including later invalidated observations.
    /// Excludes additional direct-confirmation decodes.
    pub indexed_decodes: usize,
    /// Same-key pairs admitted before final source validation.
    pub candidate_pairs: usize,
}
/// Decode each admitted source once for its exact normalized pixel key, then
/// directly confirm same-key pairs. Hash equality never proves pixel equality.
/// All batch snapshots are revalidated; late cancellation discards the report.
/// `max_pairs` bounds same-key confirmations, rather than all possible file pairs.
/// No persistent cache, approximate normalization or whole-container claim.
///
/// # Errors
/// Invalid limits, duplicate ids, file/candidate budgets or cancellation.
pub fn scan_indexed_pixels(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: PixelSearchPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<IndexedPixelReport, ScanError> {
    scan_impl(files, policy, budget, cancel, |frame, cancel| {
        frame.selected_frame_digest(cancel)
    })
}
#[allow(clippy::too_many_lines)] // Keep source observations, bounded candidates and final invalidation together.
fn scan_impl(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: PixelSearchPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    digest: impl Fn(&NormalizedRaster, &dyn Fn() -> bool) -> Result<[u8; 32], AdapterError>,
) -> Result<IndexedPixelReport, ScanError> {
    let requests = admit_requests(files, policy, &cancel)?;
    let mut ordered_ids = Vec::new();
    ordered_ids
        .try_reserve_exact(requests.len())
        .map_err(|_| ScanError::Budget)?;
    ordered_ids.extend(requests.keys().copied());
    ordered_ids.sort_unstable();
    let latch = Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || requests.values().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let mut snapshots = Vec::new();
    let mut issues = Vec::new();
    let mut source_issues = Vec::new();
    let mut buckets: HashMap<[u8; 32], Vec<u64>> = HashMap::new();
    let mut candidates = 0usize;
    for &id in &ordered_ids {
        let request = &requests[&id];
        let source = ContentSnapshot::read(&request.path, policy.decode.max_file_bytes, cancelled);
        if cancelled() {
            return Err(ScanError::Cancelled);
        }
        match source {
            Ok(source) => {
                crate::local::reserve_slot(&mut snapshots, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                snapshots.push((id, source));
            }
            Err(error) => {
                crate::local::reserve_slot(&mut source_issues, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                source_issues.push((id, error));
            }
        }
    }
    for (id, source) in &snapshots {
        let id = *id;
        let request = &requests[&id];
        let key = (|| -> Result<[u8; 32], CachedError> {
            let frame = decode_snapshot_frame(request, source, policy.decode, budget, cancelled)?;
            Ok(digest(&frame, &cancelled)?)
        })();
        if cancelled() {
            return Err(ScanError::Cancelled);
        }
        match key {
            Ok(key) => {
                if !buckets.contains_key(&key) {
                    buckets.try_reserve(1).map_err(|_| ScanError::Budget)?;
                }
                let bucket = buckets.entry(key).or_default();
                candidates = candidates.checked_add(bucket.len()).ok_or(ScanError::Budget)?;
                if candidates > policy.max_pairs {
                    return Err(ScanError::Budget);
                }
                crate::local::reserve_slot(bucket, policy.max_files).map_err(|_| ScanError::Budget)?;
                bucket.push(id);
            }
            Err(error) => {
                crate::local::reserve_slot(&mut issues, policy.max_files).map_err(|_| ScanError::Budget)?;
                issues.push(FileIssue { id, error });
            }
        }
    }
    let indexed_decodes = buckets.values().map(Vec::len).sum();
    let mut ordered_buckets = Vec::new();
    ordered_buckets
        .try_reserve_exact(buckets.len())
        .map_err(|_| ScanError::Budget)?;
    ordered_buckets.extend(buckets.iter());
    ordered_buckets.sort_unstable_by_key(|&(key, _)| *key);
    let pairs = ordered_buckets.iter().flat_map(|(_, ids)| {
        ids.iter()
            .enumerate()
            .flat_map(move |(i, &a)| ids[i + 1..].iter().map(move |&b| (a, b)))
    });
    let mut pixels = confirm_pixels(
        ordered_ids.iter().map(|&id| (id, requests[&id].clone())),
        pairs,
        VisualPolicy {
            fingerprint: policy.decode,
            max_files: policy.max_files,
            max_pairs: policy.max_pairs,
            radius: 0,
            allow_transforms: false,
            require_information: false,
        },
        budget,
        cancelled,
    )?;
    let invalid = verify_sources(snapshots, &mut source_issues, cancelled)?;
    pixels
        .equal
        .retain(|(a, b)| !invalid.contains(a) && !invalid.contains(b));
    pixels
        .different
        .retain(|(a, b)| !invalid.contains(a) && !invalid.contains(b));
    let mut analysed = Vec::new();
    analysed
        .try_reserve_exact(indexed_decodes)
        .map_err(|_| ScanError::Budget)?;
    for id in buckets.into_values().flatten() {
        if cancelled() {
            return Err(ScanError::Cancelled);
        }
        if !invalid.contains(&id) {
            analysed.push(id);
        }
    }
    analysed.sort_unstable();
    source_issues.sort_by_key(|(id, _)| *id);
    if cancelled() {
        return Err(ScanError::Cancelled);
    }
    Ok(IndexedPixelReport {
        analysed,
        pixels,
        issues,
        source_issues,
        indexed_decodes,
        candidate_pairs: candidates,
    })
}
fn verify_sources(
    snapshots: Vec<(u64, ContentSnapshot)>,
    source_issues: &mut Vec<(u64, SnapshotError)>,
    cancelled: impl Fn() -> bool,
) -> Result<HashSet<u64>, ScanError> {
    let mut invalid = HashSet::new();
    for (id, source) in snapshots {
        let result = source.verify(&cancelled);
        if cancelled() {
            return Err(ScanError::Cancelled);
        }
        if let Err(error) = result {
            invalid.try_reserve(1).map_err(|_| ScanError::Budget)?;
            invalid.insert(id);
            source_issues.try_reserve(1).map_err(|_| ScanError::Budget)?;
            source_issues.push((id, error));
        }
    }
    Ok(invalid)
}

fn admit_requests(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: PixelSearchPolicy,
    cancel: impl Fn() -> bool,
) -> Result<HashMap<u64, DecodeRequest>, ScanError> {
    if policy.decode.max_file_bytes == 0 || policy.decode.max_pixels == 0 || policy.decode.max_frames == 0 {
        return Err(ScanError::InvalidPolicy);
    }
    let mut requests = HashMap::new();
    for (id, request) in files {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        if requests.contains_key(&id) {
            return Err(ScanError::DuplicateId);
        }
        if requests.len() >= policy.max_files {
            return Err(ScanError::Budget);
        }
        requests.try_reserve(1).map_err(|_| ScanError::Budget)?;
        requests.insert(id, request);
    }
    Ok(requests)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn injected_constant_keys_still_require_direct_pixel_confirmation() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/information");
        let files = ["black", "white", "transparent-red", "transparent-blue"]
            .into_iter()
            .enumerate()
            .map(|(i, name)| {
                (
                    u64::try_from(i).unwrap() + 1,
                    DecodeRequest::new(root.join(format!("{name}.png"))),
                )
            });
        let policy = PixelSearchPolicy {
            decode: crate::decode::FingerprintPolicy {
                recipe: [0; 32],
                max_file_bytes: 1024 * 1024,
                max_pixels: 30_000,
                max_frames: 10,
                max_cache_entries: 0,
            },
            max_files: 4,
            max_pairs: 6,
        };
        let budget = MemoryBudget::new(8 * 1024 * 1024);
        let report = scan_impl(files, policy, &budget, || false, |_, _| Ok([0; 32])).unwrap();
        assert_eq!(report.candidate_pairs, 6);
        assert_eq!(report.indexed_decodes, 4);
        assert_eq!(report.pixels.equal, [(3, 4)]);
        assert_eq!(report.pixels.different.len(), 5);
        assert!(report.issues.is_empty());
        assert_eq!(budget.used(), 0);
    }
}

#[derive(Debug)]
pub struct DirectoryIndexedPixelReport {
    pub files: Vec<(u64, std::path::PathBuf)>,
    pub aliases: Vec<crate::exact::Aliases>,
    pub traversal_issues: Vec<crate::exact::Issue>,
    pub indexed: IndexedPixelReport,
}
/// Recursive exact pixel-key discovery with physical aliases and traversal errors.
///
/// # Errors
/// Invalid limits, file/candidate pair budgets or cancellation.
pub fn scan_indexed_pixel_roots(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: PixelSearchPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryIndexedPixelReport, ScanError> {
    scan_indexed_pixel_roots_with_requests(
        roots,
        traversal,
        policy,
        budget,
        |path| DecodeRequest::new(path),
        cancel,
    )
}
/// Recursive search with per-file frame, declared source-color and generation
/// settings. The request factory must retain the discovered path; aliases and
/// report identities must refer to the same physical discovery entries.
///
/// # Errors
/// Invalid limits or substituted paths, file/candidate budgets or cancellation.
pub fn scan_indexed_pixel_roots_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: PixelSearchPolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryIndexedPixelReport, ScanError> {
    if policy.decode.max_file_bytes == 0 || policy.decode.max_pixels == 0 || policy.decode.max_frames == 0 {
        return Err(ScanError::InvalidPolicy);
    }
    let discovery = crate::exact::discover(roots, traversal, &cancel);
    if discovery.cancelled || cancel() {
        return Err(ScanError::Cancelled);
    }
    if discovery.files.len() > policy.max_files {
        return Err(ScanError::Budget);
    }
    let mut files = Vec::new();
    files
        .try_reserve_exact(discovery.files.len())
        .map_err(|_| ScanError::Budget)?;
    for (i, path) in discovery.files.into_iter().enumerate() {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        files.push((u64::try_from(i).map_err(|_| ScanError::Budget)?, path));
    }
    let mut requests = Vec::new();
    requests
        .try_reserve_exact(files.len())
        .map_err(|_| ScanError::Budget)?;
    for (id, path) in &files {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        let request = request_for(path);
        if request.path != *path {
            return Err(ScanError::InvalidPolicy);
        }
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        requests.push((*id, request));
    }
    let indexed = scan_indexed_pixels(requests, policy, budget, &cancel)?;
    if cancel() {
        return Err(ScanError::Cancelled);
    }
    Ok(DirectoryIndexedPixelReport {
        files,
        aliases: discovery.aliases,
        traversal_issues: discovery.issues,
        indexed,
    })
}
