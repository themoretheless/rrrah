//! Bounded selected-frame visual batch search using decoder, cache and index.

use crate::{
    cache::FingerprintCache,
    decode::{CachedError, FingerprintPolicy, fingerprint_file},
    pixels::Comparison,
    visual::{VisualError, VisualIndex},
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;

#[derive(Debug, Clone, Copy)]
pub struct VisualPolicy {
    pub fingerprint: FingerprintPolicy,
    pub max_files: usize,
    pub max_pairs: usize,
    pub radius: u32,
    pub allow_transforms: bool,
    /// Low-information matches can be inspected by disabling this filter.
    pub require_information: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualPair {
    pub left: u64,
    pub right: u64,
    pub evidence: Comparison,
}

#[derive(Debug)]
pub struct FileIssue {
    pub id: u64,
    pub error: CachedError,
}

#[derive(Debug)]
pub struct VisualReport {
    /// Successfully fingerprinted selected frames, including cache hits.
    pub analysed: Vec<u64>,
    pub cache_hits: usize,
    /// Candidate evidence only, never file/pixel identity or whole animations.
    pub pairs: Vec<VisualPair>,
    pub issues: Vec<FileIssue>,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("invalid comparison acceptance policy")]
    InvalidPolicy,
    #[error(transparent)]
    Group(#[from] crate::groups::GroupError),
    #[error("pair must reference two distinct known ids")]
    InvalidPair,
    #[error("duplicate application id")]
    DuplicateId,
    #[error("batch search exceeds the supplied file or pair budget")]
    Budget,
    #[error("batch search cancelled")]
    Cancelled,
    #[error(transparent)]
    Index(#[from] VisualError),
}

/// Decode in id order, isolate file errors and search each successful fingerprint
/// through the metric index. Every pair appears once in deterministic id order.
/// A failing file is excluded with an explicit issue. Cache entries admitted
/// before a later batch cancellation remain valid independent observations.
///
/// # Errors
/// Returns duplicate-id, batch-budget or cancellation errors without a partial
/// report. Input can have multiple selected frames per path under distinct ids.
pub fn scan_visual(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: VisualPolicy,
    budget: &MemoryBudget,
    cache: &mut FingerprintCache,
    cancel: impl Fn() -> bool,
) -> Result<VisualReport, ScanError> {
    let mut requests = std::collections::HashMap::new();
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
    let mut ordered_requests = Vec::new();
    ordered_requests
        .try_reserve_exact(requests.len())
        .map_err(|_| ScanError::Budget)?;
    ordered_requests.extend(requests);
    ordered_requests.sort_unstable_by_key(|(id, _)| *id);
    let mut fingerprints = Vec::new();
    let mut issues = Vec::new();
    let mut cache_hits = 0;
    for (id, request) in ordered_requests {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        match fingerprint_file(&request, policy.fingerprint, budget, cache, &cancel) {
            Ok(result) => {
                cache_hits += usize::from(result.cache_hit);
                crate::local::reserve_slot(&mut fingerprints, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                fingerprints.push((id, result.fingerprint));
            }
            Err(error) => {
                crate::local::reserve_slot(&mut issues, policy.max_files).map_err(|_| ScanError::Budget)?;
                issues.push(FileIssue { id, error });
            }
        }
        if cancel() {
            return Err(ScanError::Cancelled);
        }
    }
    let index = VisualIndex::new(
        fingerprints.iter().map(|(id, fp)| (*id, fp.clone())),
        policy.max_files,
        &cancel,
    )?;
    let mut pairs = Vec::new();
    for (left, query) in &fingerprints {
        let left = *left;
        for candidate in index.search(query, policy.radius, policy.allow_transforms, &cancel)? {
            if candidate.id <= left || (policy.require_information && !candidate.evidence.informative) {
                continue;
            }
            if pairs.len() >= policy.max_pairs {
                return Err(ScanError::Budget);
            }
            crate::local::reserve_slot(&mut pairs, policy.max_pairs).map_err(|_| ScanError::Budget)?;
            pairs.push(VisualPair {
                left,
                right: candidate.id,
                evidence: candidate.evidence,
            });
        }
    }
    pairs.sort_unstable_by_key(|pair| (pair.left, pair.right));
    if cancel() {
        return Err(ScanError::Cancelled);
    }
    let mut analysed = Vec::new();
    analysed
        .try_reserve_exact(fingerprints.len())
        .map_err(|_| ScanError::Budget)?;
    analysed.extend(fingerprints.iter().map(|(id, _)| *id));
    if cancel() {
        return Err(ScanError::Cancelled);
    }
    Ok(VisualReport {
        analysed,
        cache_hits,
        pairs,
        issues,
    })
}

#[derive(Debug)]
pub struct PixelConfirmation {
    /// Exact selected-frame pixels, sample scale and hotspot; no container claim.
    pub equal: Vec<(u64, u64)>,
    pub different: Vec<(u64, u64)>,
    pub issues: Vec<PixelIssue>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfirmationError {
    #[error(transparent)]
    Prepared(#[from] CachedError),
    #[error(transparent)]
    Source(#[from] crate::exact::SnapshotError),
    #[error(transparent)]
    Decode(#[from] crate::decode::FileError),
    #[error(transparent)]
    Normalize(#[from] crate::raster::AdapterError),
}

#[derive(Debug)]
pub struct PixelIssue {
    pub left: u64,
    pub right: u64,
    pub error: ConfirmationError,
}

/// Confirm supplied candidate pairs by fresh full-resolution decoding and direct
/// pixel comparison. Each pair retains only two decoded frames. Source content
/// is validated before and after processing; mutation never admits equality.
/// Candidates may include uniform images omitted by perceptual-information gates.
///
/// # Errors
/// Returns invalid ids/pairs, resource limits or cancellation without a report.
pub fn confirm_pixels(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    pairs: impl IntoIterator<Item = (u64, u64)>,
    policy: VisualPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<PixelConfirmation, ScanError> {
    use crate::decode::{DecodeSourceSnapshot as ContentSnapshot, decode_snapshot_frame};
    let mut requests = std::collections::HashMap::new();
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
    let mut candidates = std::collections::HashSet::new();
    for (consumed, (a, b)) in pairs.into_iter().enumerate() {
        if consumed >= policy.max_pairs {
            return Err(ScanError::Budget);
        }
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        if a == b || !requests.contains_key(&a) || !requests.contains_key(&b) {
            return Err(ScanError::InvalidPair);
        }
        let pair = (a.min(b), a.max(b));
        if !candidates.contains(&pair) && candidates.len() >= policy.max_pairs {
            return Err(ScanError::Budget);
        }
        if !candidates.contains(&pair) {
            candidates.try_reserve(1).map_err(|_| ScanError::Budget)?;
            candidates.insert(pair);
        }
    }
    let mut report = PixelConfirmation {
        equal: Vec::new(),
        different: Vec::new(),
        issues: Vec::new(),
    };
    let mut ordered_candidates = Vec::new();
    ordered_candidates
        .try_reserve_exact(candidates.len())
        .map_err(|_| ScanError::Budget)?;
    ordered_candidates.extend(candidates);
    ordered_candidates.sort_unstable();
    for (a, b) in ordered_candidates {
        let left = &requests[&a];
        let right = &requests[&b];
        let cancelled = || {
            cancel()
                || [left, right].iter().any(|r| {
                    r.cancellation
                        .as_ref()
                        .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
                })
        };
        if cancelled() {
            return Err(ScanError::Cancelled);
        }
        let compare = || -> Result<bool, ConfirmationError> {
            let x = ContentSnapshot::read(&left.path, policy.fingerprint.max_file_bytes, cancelled)?;
            let y = ContentSnapshot::read(&right.path, policy.fingerprint.max_file_bytes, cancelled)?;
            let l = decode_snapshot_frame(left, &x, policy.fingerprint, budget, cancelled)?;
            let r = decode_snapshot_frame(right, &y, policy.fingerprint, budget, cancelled)?;
            let equal = l.same_selected_frame(&r, cancelled)?;
            x.verify(cancelled)?;
            y.verify(cancelled)?;
            Ok(equal)
        };
        let result = compare();
        if cancelled() {
            return Err(ScanError::Cancelled);
        }
        match result {
            Ok(true) => {
                crate::local::reserve_slot(&mut report.equal, policy.max_pairs)
                    .map_err(|_| ScanError::Budget)?;
                report.equal.push((a, b));
            }
            Ok(false) => {
                crate::local::reserve_slot(&mut report.different, policy.max_pairs)
                    .map_err(|_| ScanError::Budget)?;
                report.different.push((a, b));
            }
            Err(error) => {
                crate::local::reserve_slot(&mut report.issues, policy.max_pairs)
                    .map_err(|_| ScanError::Budget)?;
                report.issues.push(PixelIssue {
                    left: a,
                    right: b,
                    error,
                });
            }
        }
    }
    if cancel() {
        return Err(ScanError::Cancelled);
    }
    Ok(report)
}

#[derive(Debug)]
pub struct ConfirmedGroups {
    /// Singletons mean no complete accepted group, not proof of uniqueness.
    pub grouping: crate::groups::Grouping,
    pub confirmation: PixelConfirmation,
    /// Files without a stable observation across the complete batch.
    pub source_issues: Vec<(u64, crate::exact::SnapshotError)>,
}

/// Confirm supplied selected-frame candidates and partition only accepted edges.
/// Every pair in a returned multi-entry group has direct pixel evidence. Failed
/// and missing comparisons never create an edge; their diagnostics are retained.
///
/// # Errors
/// Returns validation, batch/group budgets or cancellation without partial output.
pub fn confirm_pixel_groups(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    pairs: impl IntoIterator<Item = (u64, u64)>,
    policy: VisualPolicy,
    group_budget: crate::groups::GroupBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ConfirmedGroups, ScanError> {
    let mut admitted = Vec::new();
    for entry in files {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        if admitted.len() >= policy.max_files {
            return Err(ScanError::Budget);
        }
        crate::local::reserve_slot(&mut admitted, policy.max_files).map_err(|_| ScanError::Budget)?;
        admitted.push(entry);
    }
    let mut tokens = Vec::new();
    tokens
        .try_reserve_exact(admitted.len())
        .map_err(|_| ScanError::Budget)?;
    tokens.extend(
        admitted
            .iter()
            .filter_map(|(_, request)| request.cancellation.clone()),
    );
    let caller_cancel = &cancel;
    let cancel = || caller_cancel() || tokens.iter().any(rrrah_decode::GenerationToken::is_cancelled);
    let mut ids = Vec::new();
    ids.try_reserve_exact(admitted.len())
        .map_err(|_| ScanError::Budget)?;
    ids.extend(admitted.iter().map(|&(id, _)| id));
    let mut snapshots = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut source_issues = Vec::new();
    for (id, request) in &admitted {
        if seen.contains(id) {
            return Err(ScanError::DuplicateId);
        }
        seen.try_reserve(1).map_err(|_| ScanError::Budget)?;
        seen.insert(*id);
        match crate::decode::DecodeSourceSnapshot::read(
            &request.path,
            policy.fingerprint.max_file_bytes,
            cancel,
        ) {
            Ok(snapshot) => {
                crate::local::reserve_slot(&mut snapshots, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                snapshots.push((*id, snapshot));
            }
            Err(crate::exact::SnapshotError::Cancelled) => return Err(ScanError::Cancelled),
            Err(error) => {
                crate::local::reserve_slot(&mut source_issues, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                source_issues.push((*id, error));
            }
        }
        if cancel() {
            return Err(ScanError::Cancelled);
        }
    }
    snapshots.sort_unstable_by_key(|(id, _)| *id);
    let mut confirmation = confirm_pixels(admitted, pairs, policy, budget, cancel)?;
    for (id, snapshot) in snapshots {
        match snapshot.verify(cancel) {
            Ok(()) => {}
            Err(crate::exact::SnapshotError::Cancelled) => return Err(ScanError::Cancelled),
            Err(error) => {
                crate::local::reserve_slot(&mut source_issues, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                source_issues.push((id, error));
            }
        }
        if cancel() {
            return Err(ScanError::Cancelled);
        }
    }
    source_issues.sort_by_key(|(id, _)| *id);
    let mut invalid = std::collections::HashSet::new();
    invalid
        .try_reserve(source_issues.len())
        .map_err(|_| ScanError::Budget)?;
    invalid.extend(source_issues.iter().map(|(id, _)| *id));
    confirmation
        .equal
        .retain(|(a, b)| !invalid.contains(a) && !invalid.contains(b));
    confirmation
        .different
        .retain(|(a, b)| !invalid.contains(a) && !invalid.contains(b));
    let grouping = crate::groups::complete_link_groups(
        ids,
        confirmation
            .equal
            .iter()
            .map(|&(left, right)| crate::groups::Pair { left, right }),
        group_budget,
        cancel,
    )?;
    Ok(ConfirmedGroups {
        grouping,
        confirmation,
        source_issues,
    })
}

#[derive(Debug)]
pub struct DirectoryVisualReport {
    /// Stable within this ordered traversal; ids are not persistent path identities.
    pub files: Vec<(u64, std::path::PathBuf)>,
    pub aliases: Vec<crate::exact::Aliases>,
    pub traversal_issues: Vec<crate::exact::Issue>,
    pub visual: VisualReport,
}

/// Recursively discover physical files and search their selected first frames.
/// All regular files reach the decoder: unsupported/corrupt contents are retained
/// as issues rather than silently excluded by filename extension. Aliases do not
/// inflate duplicate candidates. This does not enumerate animation/page frames.
///
/// # Errors
/// Returns batch budgets or cancellation without a partial visual report.
pub fn scan_visual_roots(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: VisualPolicy,
    budget: &MemoryBudget,
    cache: &mut FingerprintCache,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryVisualReport, ScanError> {
    let discovery = crate::exact::discover(roots, traversal, &cancel);
    if discovery.cancelled || cancel() {
        return Err(ScanError::Cancelled);
    }
    if discovery.files.len() > policy.max_files {
        return Err(ScanError::Budget);
    }
    let files = enumerate_discovered_files(discovery.files, &cancel)?;
    let visual = scan_visual(
        files.iter().map(|(id, path)| (*id, DecodeRequest::new(path))),
        policy,
        budget,
        cache,
        &cancel,
    )?;
    Ok(DirectoryVisualReport {
        files,
        aliases: discovery.aliases,
        traversal_issues: discovery.issues,
        visual,
    })
}

/// Exact selected-frame pixel search independent of perceptual information gates.
/// Decoder limits apply; fingerprint recipe/cache settings are not used.
#[derive(Debug, Clone, Copy)]
pub struct PixelSearchPolicy {
    pub decode: FingerprintPolicy,
    pub max_files: usize,
    pub max_pairs: usize,
}
#[derive(Debug)]
pub struct PixelSearchReport {
    pub pixels: PixelConfirmation,
    /// Sources unavailable at admission or changed during the whole scan.
    pub source_issues: Vec<(u64, crate::exact::SnapshotError)>,
}
/// Exhaustive exact selected-frame search, including uniform and transparent images.
/// This does not require visual candidates. Full batch source snapshots discard
/// stale equal/different decisions; omissions are diagnostics, not uniqueness.
/// Pair count is quadratic, admitted before source reads; only two decoded frames
/// are retained at once. Equality is direct full-resolution pixel comparison.
///
/// # Errors
/// Invalid limits, duplicate ids, file/pair budget or latched cancellation.
#[allow(clippy::too_many_lines)] // Keep admitted pairs, source observations and batch-wide invalidation together.
pub fn scan_equal_pixels(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: PixelSearchPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<PixelSearchReport, ScanError> {
    use crate::decode::DecodeSourceSnapshot as ContentSnapshot;
    if policy.decode.max_file_bytes == 0 || policy.decode.max_pixels == 0 || policy.decode.max_frames == 0 {
        return Err(ScanError::InvalidPolicy);
    }
    let mut requests = std::collections::HashMap::new();
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
    let count = requests.len();
    let pairs = count
        .checked_mul(count.saturating_sub(1))
        .ok_or(ScanError::Budget)?
        / 2;
    if pairs > policy.max_pairs {
        return Err(ScanError::Budget);
    }
    let latch = std::cell::Cell::new(false);
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
    let mut ordered_ids = Vec::new();
    ordered_ids
        .try_reserve_exact(requests.len())
        .map_err(|_| ScanError::Budget)?;
    ordered_ids.extend(requests.keys().copied());
    ordered_ids.sort_unstable();
    let mut snapshots = Vec::new();
    let mut source_issues = Vec::new();
    for &id in &ordered_ids {
        let request = &requests[&id];
        let snapshot = ContentSnapshot::read(&request.path, policy.decode.max_file_bytes, cancelled);
        if cancelled() {
            return Err(ScanError::Cancelled);
        }
        match snapshot {
            Ok(s) => {
                crate::local::reserve_slot(&mut snapshots, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                snapshots.push((id, s));
            }
            Err(error) => {
                crate::local::reserve_slot(&mut source_issues, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                source_issues.push((id, error));
            }
        }
    }
    let mut ids = Vec::new();
    ids.try_reserve_exact(snapshots.len())
        .map_err(|_| ScanError::Budget)?;
    ids.extend(snapshots.iter().map(|(id, _)| *id));
    let candidates = ids
        .iter()
        .enumerate()
        .flat_map(|(i, &a)| ids[i + 1..].iter().map(move |&b| (a, b)));
    let mut pixels = confirm_pixels(
        ids.iter().map(|&id| (id, requests[&id].clone())),
        candidates,
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
    let mut invalid = std::collections::HashSet::new();
    for (id, snapshot) in snapshots {
        let result = snapshot.verify(cancelled);
        if cancelled() {
            return Err(ScanError::Cancelled);
        }
        if let Err(error) = result {
            invalid.try_reserve(1).map_err(|_| ScanError::Budget)?;
            invalid.insert(id);
            crate::local::reserve_slot(&mut source_issues, policy.max_files)
                .map_err(|_| ScanError::Budget)?;
            source_issues.push((id, error));
        }
    }
    pixels
        .equal
        .retain(|(a, b)| !invalid.contains(a) && !invalid.contains(b));
    pixels
        .different
        .retain(|(a, b)| !invalid.contains(a) && !invalid.contains(b));
    source_issues.sort_by_key(|(id, _)| *id);
    if cancelled() {
        return Err(ScanError::Cancelled);
    }
    Ok(PixelSearchReport {
        pixels,
        source_issues,
    })
}

#[derive(Debug)]
pub struct DirectoryPixelSearchReport {
    pub files: Vec<(u64, std::path::PathBuf)>,
    pub aliases: Vec<crate::exact::Aliases>,
    pub traversal_issues: Vec<crate::exact::Issue>,
    pub search: PixelSearchReport,
}
/// Discover recursive roots, then search exact selected-frame pixels independently
/// of visual information gates. Physical aliases and traversal errors are exposed.
///
/// # Errors
/// Invalid decode limits, file/pair budgets or cancellation without partial results.
pub fn scan_equal_pixel_roots(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: PixelSearchPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryPixelSearchReport, ScanError> {
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
    let files = enumerate_discovered_files(discovery.files, &cancel)?;
    let search = scan_equal_pixels(
        files.iter().map(|(id, path)| (*id, DecodeRequest::new(path))),
        policy,
        budget,
        &cancel,
    )?;
    if cancel() {
        return Err(ScanError::Cancelled);
    }
    Ok(DirectoryPixelSearchReport {
        files,
        aliases: discovery.aliases,
        traversal_issues: discovery.issues,
        search,
    })
}

/// Preserve discovered path order while admitting identifier storage fallibly.
pub(crate) fn enumerate_discovered_files(
    paths: Vec<std::path::PathBuf>,
    cancel: &impl Fn() -> bool,
) -> Result<Vec<(u64, std::path::PathBuf)>, ScanError> {
    let mut files = Vec::new();
    files
        .try_reserve_exact(paths.len())
        .map_err(|_| ScanError::Budget)?;
    for (index, path) in paths.into_iter().enumerate() {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        files.push((u64::try_from(index).map_err(|_| ScanError::Budget)?, path));
    }
    if cancel() {
        return Err(ScanError::Cancelled);
    }
    Ok(files)
}
