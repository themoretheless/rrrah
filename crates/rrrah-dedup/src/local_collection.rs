//! Descriptor-indexed selected-frame candidates, followed by fresh geometry/pixels.
use crate::{
    decode::DecodeSourceSnapshot as ContentSnapshot,
    decode::{CachedError, FingerprintPolicy, decode_snapshot_frame},
    local::{Feature, LocalError},
    local_index::{FileFeatureBudgets, descriptor_file_pairs_shared},
    local_scan::{
        LocalFileError, LocalFileReport, LocalPair, LocalSearchPolicy, SpatialFeaturePolicy,
        compare_local_files_spatial_with_policy, compare_local_files_with_policy, extract_search_features,
        validate_search, validate_spatial,
    },
    scan::ScanError,
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
};
#[derive(Debug, Clone, Copy)]
pub struct LocalCollectionPolicy {
    pub search: LocalSearchPolicy,
    pub budgets: FileFeatureBudgets,
}
#[derive(Debug)]
pub struct LocalCollectionReport<E = crate::local_scan::LocalFileEvidence> {
    /// Stable successful feature extractions; insufficient texture is separately exposed.
    pub analysed: Vec<u64>,
    pub insufficient_features: Vec<u64>,
    pub file_issues: Vec<(u64, LocalFileError)>,
    /// Only retrieved pairs are attempted, with the same strict/fitted verification.
    pub local: LocalFileReport<E>,
    pub indexed_features: usize,
    pub descriptor_hits: usize,
    /// Admission count before final batch source invalidation.
    pub proposed_pairs: usize,
    pub pixel_verification_pairs: usize,
}
fn mapped(error: &LocalError) -> ScanError {
    match error {
        LocalError::Invalid => ScanError::InvalidPolicy,
        LocalError::Budget => ScanError::Budget,
        LocalError::Cancelled => ScanError::Cancelled,
    }
}
/// Extract one selected-frame feature set per file, retrieve necessary descriptor
/// pairs, then re-decode and verify those pairs with the existing geometry and
/// bidirectional pixel/color policy. No whole-image hash prefilter can hide crops.
/// Initial/final batch snapshots discard all stale decisions. Uniform/no-feature
/// sources require the separate exact-pixel search, not a uniqueness interpretation.
///
/// # Errors
/// Invalid policy/IDs, file/feature/retrieval budgets or latched cancellation.
pub fn scan_local_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: LocalCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport, ScanError> {
    scan_local_collection_selected(files, &policy, None, budget, cancel)
}
/// Search with spatial quotas shared by index construction and fresh pixel checks.
///
/// # Errors
/// Invalid policy, source errors, resource limits or cancellation.
pub fn scan_local_collection_spatial(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: LocalCollectionPolicy,
    spatial: SpatialFeaturePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport, ScanError> {
    validate_spatial(spatial, policy.search.local.extract.max_features).map_err(|e| match e {
        LocalFileError::InvalidPolicy => ScanError::InvalidPolicy,
        _ => ScanError::Budget,
    })?;
    scan_local_collection_selected(files, &policy, Some(spatial), budget, cancel)
}
#[allow(clippy::too_many_lines)] // Keep admission, retrieval and final invalidation in one pipeline.
fn scan_local_collection_selected(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: &LocalCollectionPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport, ScanError> {
    scan_collection_with_verifiers(
        files,
        policy,
        spatial,
        budget,
        cancel,
        |left, right, cancel| preverify(left, right, policy.search, cancel),
        |left, right, cancel| compare_selected_pair(left, right, policy.search, spatial, budget, cancel),
    )
}
#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // One admission/index/source lifecycle for both evidence types.
fn scan_collection_with_verifiers<E>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: &LocalCollectionPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    precheck: impl Fn(&[Feature], &[Feature], &dyn Fn() -> bool) -> Result<Option<E>, LocalFileError>,
    compare: impl Fn(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<E, LocalFileError>,
) -> Result<LocalCollectionReport<E>, ScanError> {
    scan_collection_with_extractor(
        files,
        policy,
        cancel,
        |request, source, cancel| extract_file(request, source, policy.search, spatial, budget, cancel),
        precheck,
        compare,
    )
}
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn scan_collection_with_extractor<E>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: &LocalCollectionPolicy,
    cancel: impl Fn() -> bool,
    extract: impl Fn(
        &DecodeRequest,
        &ContentSnapshot,
        &dyn Fn() -> bool,
    ) -> Result<rrrah_core::SharedBuffer<Feature>, LocalFileError>,
    precheck: impl Fn(&[Feature], &[Feature], &dyn Fn() -> bool) -> Result<Option<E>, LocalFileError>,
    compare: impl Fn(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<E, LocalFileError>,
) -> Result<LocalCollectionReport<E>, ScanError> {
    validate_search(policy.search)?;
    let requests = admit(files, policy.budgets.max_files, &cancel)?;
    let mut generations = Vec::new();
    generations
        .try_reserve_exact(requests.len())
        .map_err(|_| ScanError::Budget)?;
    generations.extend(
        requests
            .values()
            .filter_map(|request| request.cancellation.as_ref()),
    );
    let latch = Cell::new(false);
    let cancelled = || {
        let value = latch.get() || cancel() || generations.iter().any(|token| token.is_cancelled());
        latch.set(value);
        value
    };
    let mut local = LocalFileReport::default();
    let snapshots = snapshot_sources(
        &requests,
        policy.search.local.decode.max_file_bytes,
        &mut local,
        cancelled,
    )?;
    let mut features = HashMap::new();
    let mut file_issues = Vec::new();
    let mut feature_count = 0usize;
    for (id, source) in &snapshots {
        let id = *id;
        let result = extract(&requests[&id], source, &cancelled);
        if cancelled() {
            return Err(ScanError::Cancelled);
        }
        match result {
            Ok(f) => {
                feature_count = feature_count.checked_add(f.len()).ok_or(ScanError::Budget)?;
                if feature_count > policy.budgets.max_features {
                    return Err(ScanError::Budget);
                }
                features.try_reserve(1).map_err(|_| ScanError::Budget)?;
                features.insert(id, f);
            }
            Err(error) => {
                crate::local::reserve_slot(&mut file_issues, policy.budgets.max_files)
                    .map_err(|_| ScanError::Budget)?;
                file_issues.push((id, error));
            }
        }
    }
    let mut analysed = Vec::new();
    analysed
        .try_reserve_exact(features.len())
        .map_err(|_| ScanError::Budget)?;
    analysed.extend(features.keys().copied());
    analysed.sort_unstable();
    let retrieved = descriptor_file_pairs_shared(
        features.iter().map(|(&id, f)| (id, f.clone())),
        policy.search.local.matching.max_distance,
        policy.search.local.geometry.min_inliers,
        policy.budgets,
        cancelled,
    )
    .map_err(|error| mapped(&error))?;
    let proposed_pairs = retrieved.pairs.len();
    let mut pixel_verification_pairs = 0;
    for (left, right) in retrieved.pairs {
        let result = precheck(&features[&left], &features[&right], &cancelled).and_then(|evidence| {
            if let Some(evidence) = evidence {
                return Ok(evidence);
            }
            pixel_verification_pairs += 1;
            compare(&requests[&left], &requests[&right], &cancelled)
        });
        if cancelled() || matches!(result, Err(LocalFileError::Cancelled)) {
            return Err(ScanError::Cancelled);
        }
        match result {
            Ok(evidence) => {
                crate::local::reserve_slot(&mut local.pairs, policy.budgets.max_pairs)
                    .map_err(|_| ScanError::Budget)?;
                local.pairs.push(LocalPair {
                    left,
                    right,
                    evidence,
                });
            }
            Err(error) => {
                crate::local::reserve_slot(&mut local.issues, policy.budgets.max_pairs)
                    .map_err(|_| ScanError::Budget)?;
                local.issues.push((left, right, error));
            }
        }
    }
    let invalid = verify_sources(snapshots, &mut local, cancelled)?;
    local
        .pairs
        .retain(|p| !invalid.contains(&p.left) && !invalid.contains(&p.right));
    analysed.retain(|id| !invalid.contains(id));
    let mut insufficient_features = retrieved.insufficient_features;
    insufficient_features.retain(|id| !invalid.contains(id));
    if cancelled() {
        return Err(ScanError::Cancelled);
    }
    Ok(LocalCollectionReport {
        analysed,
        insufficient_features,
        file_issues,
        local,
        indexed_features: retrieved.indexed_features,
        descriptor_hits: retrieved.descriptor_hits,
        proposed_pairs,
        pixel_verification_pairs,
    })
}
fn compare_selected_pair(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: LocalSearchPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::local_scan::LocalFileEvidence, LocalFileError> {
    if let Some(grid) = spatial {
        compare_local_files_spatial_with_policy(left, right, search, grid, budget, cancel)
    } else {
        compare_local_files_with_policy(left, right, search, budget, cancel)
    }
}
fn snapshot_sources<E>(
    requests: &HashMap<u64, DecodeRequest>,
    max_file_bytes: u64,
    report: &mut LocalFileReport<E>,
    cancel: impl Fn() -> bool,
) -> Result<Vec<(u64, ContentSnapshot)>, ScanError> {
    let mut ids = Vec::new();
    ids.try_reserve_exact(requests.len())
        .map_err(|_| ScanError::Budget)?;
    ids.extend(requests.keys().copied());
    ids.sort_unstable();
    let mut snapshots = Vec::new();
    for id in ids {
        let source = ContentSnapshot::read(&requests[&id].path, max_file_bytes, &cancel);
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        match source {
            Ok(source) => {
                crate::local::reserve_slot(&mut snapshots, requests.len()).map_err(|_| ScanError::Budget)?;
                snapshots.push((id, source));
            }
            Err(error) => {
                crate::local::reserve_slot(&mut report.source_issues, requests.len())
                    .map_err(|_| ScanError::Budget)?;
                report.source_issues.push((id, error));
            }
        }
    }
    Ok(snapshots)
}

fn admit(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    max_files: usize,
    cancel: impl Fn() -> bool,
) -> Result<HashMap<u64, DecodeRequest>, ScanError> {
    let mut requests = HashMap::new();
    for (id, request) in files {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        if requests.contains_key(&id) {
            return Err(ScanError::DuplicateId);
        }
        if requests.len() >= max_files {
            return Err(ScanError::Budget);
        }
        requests.try_reserve(1).map_err(|_| ScanError::Budget)?;
        requests.insert(id, request);
    }
    Ok(requests)
}
fn extract_file(
    request: &DecodeRequest,
    source: &ContentSnapshot,
    policy: LocalSearchPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<Feature>, LocalFileError> {
    extract_file_with(request, source, policy, budget, cancel, |view, cancel| {
        extract_search_features(view, policy.local.extract, spatial, budget, cancel)
    })
}
fn extract_file_with(
    request: &DecodeRequest,
    source: &ContentSnapshot,
    policy: LocalSearchPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    extract: impl Fn(
        &crate::linear::LinearRgbaView<'_>,
        &dyn Fn() -> bool,
    ) -> Result<rrrah_core::SharedBuffer<Feature>, LocalError>,
) -> Result<rrrah_core::SharedBuffer<Feature>, LocalFileError> {
    let limits = policy.local.decode;
    let frame = decode_snapshot_frame(
        request,
        source,
        FingerprintPolicy {
            recipe: [0; 32],
            max_file_bytes: limits.max_file_bytes,
            max_pixels: limits.max_pixels,
            max_frames: limits.max_frames,
            max_cache_entries: 0,
        },
        budget,
        &cancel,
    )?;
    let view = frame.view(&cancel).map_err(CachedError::from)?;
    let features = extract(&view, &cancel)?;
    source.verify(cancel)?;
    Ok(features)
}
fn verify_sources<E>(
    snapshots: Vec<(u64, ContentSnapshot)>,
    report: &mut LocalFileReport<E>,
    cancel: impl Fn() -> bool,
) -> Result<HashSet<u64>, ScanError> {
    let mut invalid = HashSet::new();
    invalid
        .try_reserve(snapshots.len())
        .map_err(|_| ScanError::Budget)?;
    let max_issues = report
        .source_issues
        .len()
        .checked_add(snapshots.len())
        .ok_or(ScanError::Budget)?;
    for (id, source) in snapshots {
        let result = source.verify(&cancel);
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        if let Err(error) = result {
            invalid.insert(id);
            crate::local::reserve_slot(&mut report.source_issues, max_issues)
                .map_err(|_| ScanError::Budget)?;
            report.source_issues.push((id, error));
        }
    }
    report.source_issues.sort_by_key(|(id, _)| *id);
    Ok(invalid)
}

fn preverify(
    left: &[Feature],
    right: &[Feature],
    policy: LocalSearchPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Option<crate::local_scan::LocalFileEvidence>, LocalFileError> {
    let pairs = left.len().checked_mul(right.len()).ok_or(LocalError::Budget)?;
    if u64::try_from(pairs).map_err(|_| LocalError::Budget)? > policy.local.matching.max_comparisons {
        return Err(LocalError::Budget.into());
    }
    // This pair is already retrieved by the collection descriptor index and
    // admitted under the exhaustive comparison cap. Avoid rebuilding two
    // per-pair indexes; the same mutual-nearest/ratio metric is used directly.
    let correspondences = crate::local::match_features(left, right, policy.local.matching, &cancel)?;
    if crate::geometry::verify_similarity(&correspondences, policy.local.geometry, &cancel)?.is_some() {
        return Ok(None);
    }
    Ok(Some(crate::local_scan::LocalFileEvidence {
        correspondences,
        geometry: None,
        pixels: None,
        photometric: None,
        photometric_fit_range: match policy.comparison {
            crate::local_scan::LocalComparisonMode::RangePhotometric { range, .. }
            | crate::local_scan::LocalComparisonMode::DisplayProjection { range, .. } => Some(range),
            _ => None,
        },
        display_projection: matches!(
            policy.comparison,
            crate::local_scan::LocalComparisonMode::DisplayProjection { .. }
        ),
        photometric_failure: None,
        filtered: None,
        candidate: false,
    }))
}

#[derive(Debug)]
pub struct DirectoryLocalCollectionReport<E = crate::local_scan::LocalFileEvidence> {
    pub files: Vec<(u64, std::path::PathBuf)>,
    pub aliases: Vec<crate::exact::Aliases>,
    pub traversal_issues: Vec<crate::exact::Issue>,
    pub indexed: LocalCollectionReport<E>,
}
/// Recursive transformed-copy discovery with physical aliases and traversal errors.
///
/// # Errors
/// Invalid search policy, file/feature/candidate budgets or cancellation.
pub fn scan_local_collection_roots(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: LocalCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport, ScanError> {
    scan_local_collection_roots_with_requests(
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
/// Invalid search policy or substituted paths, file/feature/candidate budgets or cancellation.
pub fn scan_local_collection_roots_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: LocalCollectionPolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport, ScanError> {
    scan_roots_selected(roots, traversal, &policy, None, budget, request_for, cancel)
}
/// Recursive spatial search with explicit per-file decode settings.
///
/// # Errors
/// Invalid grid/search/request path, resource limits or cancellation.
pub fn scan_local_collection_roots_spatial_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: LocalCollectionPolicy,
    spatial: SpatialFeaturePolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport, ScanError> {
    validate_spatial(spatial, policy.search.local.extract.max_features).map_err(|e| match e {
        LocalFileError::InvalidPolicy => ScanError::InvalidPolicy,
        _ => ScanError::Budget,
    })?;
    scan_roots_selected(
        roots,
        traversal,
        &policy,
        Some(spatial),
        budget,
        request_for,
        cancel,
    )
}
fn scan_roots_selected(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: &LocalCollectionPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport, ScanError> {
    validate_search(policy.search)?;
    scan_roots_with_collection(
        roots,
        traversal,
        policy.budgets.max_files,
        request_for,
        cancel,
        |requests, cancel| scan_local_collection_selected(requests, policy, spatial, budget, cancel),
    )
}
fn scan_roots_with_collection<E>(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    max_files: usize,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
    scan: impl FnOnce(Vec<(u64, DecodeRequest)>, &dyn Fn() -> bool) -> Result<LocalCollectionReport<E>, ScanError>,
) -> Result<DirectoryLocalCollectionReport<E>, ScanError> {
    let discovery = crate::exact::discover(roots, traversal, &cancel);
    if discovery.cancelled || cancel() {
        return Err(ScanError::Cancelled);
    }
    if discovery.files.len() > max_files {
        return Err(ScanError::Budget);
    }
    let files = crate::scan::enumerate_discovered_files(discovery.files, &cancel)?;
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
    let indexed = scan(requests, &cancel)?;
    if cancel() {
        return Err(ScanError::Cancelled);
    }
    Ok(DirectoryLocalCollectionReport {
        files,
        aliases: discovery.aliases,
        traversal_issues: discovery.issues,
        indexed,
    })
}

fn extract_reflection_file(
    request: &DecodeRequest,
    source: &ContentSnapshot,
    policy: LocalSearchPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<(rrrah_core::SharedBuffer<Feature>, usize), LocalFileError> {
    let limits = policy.local.decode;
    let frame = decode_snapshot_frame(
        request,
        source,
        FingerprintPolicy {
            recipe: [0; 32],
            max_file_bytes: limits.max_file_bytes,
            max_pixels: limits.max_pixels,
            max_frames: limits.max_frames,
            max_cache_entries: 0,
        },
        budget,
        &cancel,
    )?;
    let view = frame.view(&cancel).map_err(CachedError::from)?;
    let normal = extract_search_features(&view, policy.local.extract, spatial, budget, &cancel)?;
    let reflected = crate::local_scan::extract_search_features_selected(
        &view,
        policy.local.extract,
        spatial,
        budget,
        &cancel,
        true,
    )?;
    let count = normal
        .len()
        .checked_add(reflected.len())
        .ok_or(LocalError::Budget)?;
    let bytes = count
        .checked_mul(std::mem::size_of::<Feature>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let reservation = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
    let mut combined = Vec::new();
    combined
        .try_reserve_exact(count)
        .map_err(|_| LocalError::Budget)?;
    for feature in normal.iter().chain(reflected.iter()) {
        if cancel() {
            return Err(LocalFileError::Cancelled);
        }
        combined.push(*feature);
    }
    let features = reservation.try_adopt(combined).map_err(|_| LocalError::Budget)?;
    source.verify(cancel)?;
    Ok((features, normal.len()))
}
#[derive(Debug)]
pub struct ReflectedCollectionPair {
    pub left: u64,
    pub right: u64,
    pub evidence: crate::local_scan::ReflectedLocalFileEvidence,
}
#[derive(Debug)]
pub struct ReflectedCollectionReport {
    pub analysed: Vec<u64>,
    pub insufficient_features: Vec<u64>,
    pub file_issues: Vec<(u64, LocalFileError)>,
    pub pairs: Vec<ReflectedCollectionPair>,
    pub issues: Vec<(u64, u64, LocalFileError)>,
    pub source_issues: Vec<(u64, crate::exact::SnapshotError)>,
    pub indexed_features: usize,
    pub descriptor_hits: usize,
    pub proposed_pairs: usize,
    /// Fresh decoded pixel confirmations after cached descriptor/geometry refusal.
    pub pixel_verification_pairs: usize,
}

/// Retrieve reflected selected-frame candidates by indexing ordinary and reflected
/// multiscale descriptors together, then freshly verify reflected geometry/pixels.
/// Features from both orientations count against the supplied total feature cap.
/// Strict and explicit linear photometric comparisons retain strict pixel evidence.
/// Both initial and final batch source observations protect returned pair evidence.
///
/// # Errors
/// Invalid policy/IDs, exhausted resources or latched cancellation; no partial report.
#[allow(clippy::too_many_lines)] // Keep admission, retrieval and source invalidation together.
pub fn scan_reflected_local_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: LocalCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ReflectedCollectionReport, ScanError> {
    scan_reflected_collection_selected(files, &policy, None, budget, cancel)
}

/// Reflected collection retrieval and fresh confirmation sharing spatial quotas.
///
/// # Errors
/// Invalid grid/search policy, resources or cancellation without a report.
pub fn scan_reflected_local_collection_spatial(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: LocalCollectionPolicy,
    spatial: SpatialFeaturePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ReflectedCollectionReport, ScanError> {
    validate_spatial(spatial, policy.search.local.extract.max_features).map_err(|e| match e {
        LocalFileError::InvalidPolicy => ScanError::InvalidPolicy,
        _ => ScanError::Budget,
    })?;
    scan_reflected_collection_selected(files, &policy, Some(spatial), budget, cancel)
}
#[allow(clippy::too_many_lines)] // Admission, retrieval and final source invalidation.
fn scan_reflected_collection_selected(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: &LocalCollectionPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ReflectedCollectionReport, ScanError> {
    validate_search(policy.search)?;
    let photometric = reflected_color_policy(policy.search.comparison);
    let requests = admit(files, policy.budgets.max_files, &cancel)?;
    let latch = Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || requests.values().any(|request| {
                request
                    .cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let mut observations: LocalFileReport = LocalFileReport::default();
    let snapshots = snapshot_sources(
        &requests,
        policy.search.local.decode.max_file_bytes,
        &mut observations,
        cancelled,
    )?;
    let mut features = HashMap::new();
    let mut file_issues = Vec::new();
    let mut total = 0usize;
    for (id, source) in &snapshots {
        let id = *id;
        let result =
            extract_reflection_file(&requests[&id], source, policy.search, spatial, budget, cancelled);
        if cancelled() {
            return Err(ScanError::Cancelled);
        }
        match result {
            Ok(set) => {
                total = total.checked_add(set.0.len()).ok_or(ScanError::Budget)?;
                if total > policy.budgets.max_features {
                    return Err(ScanError::Budget);
                }
                features.try_reserve(1).map_err(|_| ScanError::Budget)?;
                features.insert(id, set);
            }
            Err(error) => {
                crate::local::reserve_slot(&mut file_issues, policy.budgets.max_files)
                    .map_err(|_| ScanError::Budget)?;
                file_issues.push((id, error));
            }
        }
    }
    let mut analysed = Vec::new();
    analysed
        .try_reserve_exact(features.len())
        .map_err(|_| ScanError::Budget)?;
    analysed.extend(features.keys().copied());
    analysed.sort_unstable();
    let retrieved = descriptor_file_pairs_shared(
        features.iter().map(|(&id, f)| (id, f.0.clone())),
        policy.search.local.matching.max_distance,
        policy.search.local.geometry.min_inliers,
        policy.budgets,
        cancelled,
    )
    .map_err(|e| mapped(&e))?;
    let proposed_pairs = retrieved.pairs.len();
    let mut pairs = Vec::new();
    let mut issues = Vec::new();
    let mut pixel_verification_pairs = 0;
    for (left, right) in retrieved.pairs {
        let (left_set, left_count) = &features[&left];
        let (right_set, right_count) = &features[&right];
        // These slices exactly match ordinary-left/reflected-right fresh extraction.
        // Mixing both orientations in mutual-nearest matching could lose valid pairs.
        let result = preverify_reflected(
            &left_set[..*left_count],
            &right_set[*right_count..],
            photometric,
            policy.search.local,
            cancelled,
        )
        .and_then(|evidence| {
            if let Some(evidence) = evidence {
                return Ok(evidence);
            }
            pixel_verification_pairs += 1;
            crate::local_scan::compare_reflected_local_files_with_signals(
                &requests[&left],
                &requests[&right],
                policy.search.local,
                spatial,
                photometric,
                budget,
                cancelled,
            )
        });
        if cancelled() || matches!(result, Err(LocalFileError::Cancelled)) {
            return Err(ScanError::Cancelled);
        }
        match result {
            Ok(evidence) => {
                crate::local::reserve_slot(&mut pairs, policy.budgets.max_pairs)
                    .map_err(|_| ScanError::Budget)?;
                pairs.push(ReflectedCollectionPair {
                    left,
                    right,
                    evidence,
                });
            }
            Err(error) => {
                crate::local::reserve_slot(&mut issues, policy.budgets.max_pairs)
                    .map_err(|_| ScanError::Budget)?;
                issues.push((left, right, error));
            }
        }
    }
    let invalid = verify_sources(snapshots, &mut observations, cancelled)?;
    pairs.retain(|p| !invalid.contains(&p.left) && !invalid.contains(&p.right));
    analysed.retain(|id| !invalid.contains(id));
    let mut insufficient_features = retrieved.insufficient_features;
    insufficient_features.retain(|id| !invalid.contains(id));
    if cancelled() {
        return Err(ScanError::Cancelled);
    }
    Ok(ReflectedCollectionReport {
        analysed,
        insufficient_features,
        file_issues,
        pairs,
        issues,
        source_issues: observations.source_issues,
        indexed_features: retrieved.indexed_features,
        descriptor_hits: retrieved.descriptor_hits,
        proposed_pairs,
        pixel_verification_pairs,
    })
}

fn preverify_reflected(
    left: &[Feature],
    right: &[Feature],
    signals: crate::local_scan::ReflectedSignals,
    policy: crate::local_scan::LocalFilePolicy,
    cancel: impl Fn() -> bool,
) -> Result<Option<crate::local_scan::ReflectedLocalFileEvidence>, LocalFileError> {
    let correspondences = crate::local::match_features(left, right, policy.matching, &cancel)?;
    if crate::geometry::verify_reflected_similarity(&correspondences, policy.geometry, &cancel)?.is_some() {
        return Ok(None);
    }
    Ok(Some(crate::local_scan::ReflectedLocalFileEvidence {
        correspondences,
        geometry: None,
        pixels: None,
        photometric: None,
        photometric_fit_range: signals.2,
        display_projection: signals.3,
        photometric_failure: None,
        filtered: None,
        candidate: false,
    }))
}

#[derive(Debug)]
pub struct DirectoryReflectedCollectionReport {
    pub files: Vec<(u64, std::path::PathBuf)>,
    pub aliases: Vec<crate::exact::Aliases>,
    pub traversal_issues: Vec<crate::exact::Issue>,
    pub indexed: ReflectedCollectionReport,
}

/// Recursively discover reflected selected-frame pairs with physical aliases.
///
/// # Errors
/// Invalid policy, file/feature/retrieval budgets or cancellation without a report.
pub fn scan_reflected_local_collection_roots(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: LocalCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryReflectedCollectionReport, ScanError> {
    scan_reflected_local_collection_roots_with_requests(
        roots,
        traversal,
        policy,
        budget,
        |path| DecodeRequest::new(path),
        cancel,
    )
}

/// Recursive reflected discovery retaining declared color/frame/generation settings.
/// The request factory must preserve every discovered file path. Unsupported
/// comparison modes refuse before traversal or factory callbacks are performed.
///
/// # Errors
/// Invalid settings/substituted paths, exceeded resource limits or cancellation.
pub fn scan_reflected_local_collection_roots_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: LocalCollectionPolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryReflectedCollectionReport, ScanError> {
    scan_reflected_roots_selected(roots, traversal, &policy, None, budget, request_for, cancel)
}

/// Recursive reflected search with spatial quotas shared by retrieval/confirmation.
/// Strict or explicit linear fitted comparison retains original pixel evidence.
///
/// # Errors
/// Invalid grid/policy, resource exhaustion or cancellation without partial report.
pub fn scan_reflected_local_collection_roots_spatial(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: LocalCollectionPolicy,
    spatial: SpatialFeaturePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryReflectedCollectionReport, ScanError> {
    scan_reflected_local_collection_roots_spatial_with_requests(
        roots,
        traversal,
        policy,
        spatial,
        budget,
        |path| DecodeRequest::new(path),
        cancel,
    )
}

/// Recursive spatial reflected search retaining per-path decode settings.
/// Invalid quotas refuse before traversal/request factory callbacks.
///
/// # Errors
/// Invalid policy/grid/path substitution, resource limits or cancellation.
pub fn scan_reflected_local_collection_roots_spatial_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: LocalCollectionPolicy,
    spatial: SpatialFeaturePolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryReflectedCollectionReport, ScanError> {
    validate_spatial(spatial, policy.search.local.extract.max_features).map_err(|error| match error {
        LocalFileError::InvalidPolicy => ScanError::InvalidPolicy,
        _ => ScanError::Budget,
    })?;
    scan_reflected_roots_selected(
        roots,
        traversal,
        &policy,
        Some(spatial),
        budget,
        request_for,
        cancel,
    )
}
fn scan_reflected_roots_selected(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: &LocalCollectionPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryReflectedCollectionReport, ScanError> {
    validate_search(policy.search)?;
    let discovery = crate::exact::discover(roots, traversal, &cancel);
    if discovery.cancelled || cancel() {
        return Err(ScanError::Cancelled);
    }
    if discovery.files.len() > policy.budgets.max_files {
        return Err(ScanError::Budget);
    }
    let files = crate::scan::enumerate_discovered_files(discovery.files, &cancel)?;
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
    let indexed = scan_reflected_collection_selected(requests, policy, spatial, budget, &cancel)?;
    if cancel() {
        return Err(ScanError::Cancelled);
    }
    Ok(DirectoryReflectedCollectionReport {
        files,
        aliases: discovery.aliases,
        traversal_issues: discovery.issues,
        indexed,
    })
}

fn reflected_color_policy(
    mode: crate::local_scan::LocalComparisonMode,
) -> crate::local_scan::ReflectedSignals {
    crate::local_scan::reflected_signals(mode)
}

/// Descriptor-indexed planar candidates with fresh bounded pixel registration.
/// Shares ordinary collection admission, dependency snapshots and final stale
/// pair removal. Evidence remains selected-frame visual similarity only.
///
/// # Errors
/// Invalid policy, duplicate IDs, resource refusal or cancellation yields no report.
pub fn scan_projective_local_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    local: crate::local_scan::LocalFilePolicy,
    filter: crate::warp::ColorFilterPolicy,
    registration: crate::warp::ProjectiveRegistrationPolicy,
    limits: FileFeatureBudgets,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    scan_projective_collection_selected(
        files,
        ProjectiveCollectionPolicy {
            local,
            filter,
            registration,
            budgets: limits,
        },
        None,
        None,
        None,
        budget,
        cancel,
    )
}

/// Planar retrieval and fresh confirmation using the same spatial feature quotas.
///
/// # Errors
/// Invalid policy, resource refusal or cancellation yields no report.
pub fn scan_projective_local_collection_spatial(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: ProjectiveCollectionPolicy,
    spatial: SpatialFeaturePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    scan_projective_collection_selected(files, policy, Some(spatial), None, None, budget, cancel)
}

fn scan_projective_collection_selected(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveCollectionPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    sampling: Option<crate::geometry::ProjectiveSamplingPolicy>,
    anchored: Option<crate::warp::ProjectiveRegistrationTrustPolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    let ProjectiveCollectionPolicy {
        local,
        filter,
        registration,
        budgets: limits,
    } = config;
    if let Some(trust) = anchored {
        crate::warp::validate_registration_trust_policy(trust).map_err(|_| ScanError::InvalidPolicy)?;
    }
    if let Some(sample) = sampling {
        if sample.seed == 0 || sample.trials == 0 {
            return Err(ScanError::InvalidPolicy);
        }
        if sample.trials > local.geometry.max_hypotheses {
            return Err(ScanError::Budget);
        }
    }
    crate::warp::validate_registration_policy(registration).map_err(|_| ScanError::InvalidPolicy)?;
    crate::warp::validate_filter_policy(filter.filter).map_err(|_| ScanError::InvalidPolicy)?;
    if local.geometry.min_inliers < 4 {
        return Err(ScanError::InvalidPolicy);
    }
    if let Some(grid) = spatial {
        validate_spatial(grid, local.extract.max_features).map_err(|_| ScanError::InvalidPolicy)?;
    }
    let policy = LocalCollectionPolicy {
        search: local.into(),
        budgets: limits,
    };
    scan_collection_with_verifiers(
        files,
        &policy,
        spatial,
        budget,
        cancel,
        |left, right, cancel| {
            let correspondences = crate::local::match_features(left, right, local.matching, cancel)?;
            let geometry = match sampling {
                Some(sample) => crate::geometry::verify_projective_sampled(
                    &correspondences,
                    local.geometry,
                    sample,
                    cancel,
                )?,
                None => crate::geometry::verify_projective(&correspondences, local.geometry, cancel)?,
            };
            if geometry.is_some() {
                return Ok(None);
            }
            Ok(Some(crate::local_scan::ProjectiveFileEvidence {
                correspondences,
                geometry,
                pixels: None,
                filtered: None,
                registered_transform: None,
                candidate: false,
            }))
        },
        |left, right, cancel| {
            if let Some(trust) = anchored {
                crate::local_scan::compare_local_files_projective_anchored(
                    left,
                    right,
                    crate::local_scan::ProjectiveAnchoredFilePolicy {
                        local,
                        filter,
                        registration: trust,
                        spatial,
                        sampling,
                    },
                    budget,
                    cancel,
                )
            } else if let Some(sample) = sampling {
                crate::local_scan::compare_local_files_projective_sampled(
                    left,
                    right,
                    crate::local_scan::ProjectiveSampledFilePolicy {
                        local,
                        filter,
                        registration,
                        spatial,
                        sampling: sample,
                    },
                    budget,
                    cancel,
                )
            } else {
                match spatial {
                    Some(grid) => crate::local_scan::compare_local_files_projective_registered_spatial(
                        left,
                        right,
                        local,
                        filter,
                        registration,
                        grid,
                        budget,
                        cancel,
                    ),
                    None => crate::local_scan::compare_local_files_projective_registered(
                        left,
                        right,
                        local,
                        filter,
                        registration,
                        budget,
                        cancel,
                    ),
                }
            }
        },
    )
}

/// Explicit planar collection configuration shared by recursive discovery.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveCollectionPolicy {
    pub local: crate::local_scan::LocalFilePolicy,
    pub filter: crate::warp::ColorFilterPolicy,
    pub registration: crate::warp::ProjectiveRegistrationPolicy,
    pub budgets: FileFeatureBudgets,
}
/// Recursive planar search with caller-selected frame/color/generation settings.
/// Physical aliases and traversal issues use the ordinary directory lifecycle.
///
/// # Errors
/// Invalid policy/request path, source/resource failure or latched cancellation.
pub fn scan_projective_local_collection_roots_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveCollectionPolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    scan_projective_roots_selected(
        roots,
        traversal,
        policy,
        None,
        None,
        None,
        budget,
        request_for,
        cancel,
    )
}

/// Recursive spatial planar search retaining caller-selected decode requests.
///
/// # Errors
/// Invalid policy/request path, resource refusal or cancellation yields no report.
pub fn scan_projective_local_collection_roots_spatial_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveCollectionPolicy,
    spatial: SpatialFeaturePolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    scan_projective_roots_selected(
        roots,
        traversal,
        policy,
        Some(spatial),
        None,
        None,
        budget,
        request_for,
        cancel,
    )
}

/// Recursive spatial planar search with native default decode requests.
///
/// # Errors
/// Invalid policy, resource refusal or cancellation yields no report.
pub fn scan_projective_local_collection_roots_spatial(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveCollectionPolicy,
    spatial: SpatialFeaturePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    scan_projective_local_collection_roots_spatial_with_requests(
        roots,
        traversal,
        policy,
        spatial,
        budget,
        |path| DecodeRequest::new(path),
        cancel,
    )
}

#[allow(clippy::too_many_arguments)] // Shared discovery accepts explicit optional selection modes.
fn scan_projective_roots_selected(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveCollectionPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    sampling: Option<crate::geometry::ProjectiveSamplingPolicy>,
    anchored: Option<crate::warp::ProjectiveRegistrationTrustPolicy>,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    if let Some(trust) = anchored {
        crate::warp::validate_registration_trust_policy(trust).map_err(|_| ScanError::InvalidPolicy)?;
    }
    if let Some(sample) = sampling {
        if sample.seed == 0 || sample.trials == 0 {
            return Err(ScanError::InvalidPolicy);
        }
        if sample.trials > policy.local.geometry.max_hypotheses {
            return Err(ScanError::Budget);
        }
    }
    validate_search(policy.local.into())?;
    crate::warp::validate_registration_policy(policy.registration).map_err(|_| ScanError::InvalidPolicy)?;
    crate::warp::validate_filter_policy(policy.filter.filter).map_err(|_| ScanError::InvalidPolicy)?;
    if policy.local.geometry.min_inliers < 4 {
        return Err(ScanError::InvalidPolicy);
    }
    if let Some(grid) = spatial {
        validate_spatial(grid, policy.local.extract.max_features).map_err(|_| ScanError::InvalidPolicy)?;
    }
    scan_roots_with_collection(
        roots,
        traversal,
        policy.budgets.max_files,
        request_for,
        cancel,
        |requests, cancel| {
            scan_projective_collection_selected(requests, policy, spatial, sampling, anchored, budget, cancel)
        },
    )
}

/// Recursive planar search with native default decode requests.
///
/// # Errors
/// Invalid policy, source/resource failures or cancellation with no report.
pub fn scan_projective_local_collection_roots(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    scan_projective_local_collection_roots_with_requests(
        roots,
        traversal,
        policy,
        budget,
        |path| DecodeRequest::new(path),
        cancel,
    )
}

/// Indexed sampled planar search using identical feature/model policies in both stages.
///
/// # Errors
/// Invalid policies, duplicate IDs, resource/work refusal or cancellation.
pub fn scan_projective_local_collection_sampled(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: crate::local_scan::ProjectiveSampledFilePolicy,
    limits: FileFeatureBudgets,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    scan_projective_collection_selected(
        files,
        ProjectiveCollectionPolicy {
            local: policy.local,
            filter: policy.filter,
            registration: policy.registration,
            budgets: limits,
        },
        policy.spatial,
        Some(policy.sampling),
        None,
        budget,
        cancel,
    )
}

/// Sampled planar collection configuration shared by recursive discovery.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveSampledCollectionPolicy {
    pub search: crate::local_scan::ProjectiveSampledFilePolicy,
    pub budgets: FileFeatureBudgets,
}

/// Recursive sampled planar search retaining per-path decode requests.
///
/// # Errors
/// Invalid policy/request path, resource refusal or cancellation yields no report.
pub fn scan_projective_local_collection_roots_sampled_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveSampledCollectionPolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    let search = policy.search;
    scan_projective_roots_selected(
        roots,
        traversal,
        ProjectiveCollectionPolicy {
            local: search.local,
            filter: search.filter,
            registration: search.registration,
            budgets: policy.budgets,
        },
        search.spatial,
        Some(search.sampling),
        None,
        budget,
        request_for,
        cancel,
    )
}

/// Recursive sampled planar search with native default decode requests.
///
/// # Errors
/// Invalid policy, resource refusal or cancellation yields no report.
pub fn scan_projective_local_collection_roots_sampled(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveSampledCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    scan_projective_local_collection_roots_sampled_with_requests(
        roots,
        traversal,
        policy,
        budget,
        |path| DecodeRequest::new(path),
        cancel,
    )
}

/// Explicit anchored planar configuration for collection and recursive search.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveAnchoredCollectionPolicy {
    pub search: crate::local_scan::ProjectiveAnchoredFilePolicy,
    pub budgets: FileFeatureBudgets,
}

/// Indexed anchored planar search using the direct file confirmation lifecycle.
///
/// # Errors
/// Invalid policies, source/work/resource refusal or cancellation.
pub fn scan_projective_local_collection_anchored(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: ProjectiveAnchoredCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    let p = policy.search;
    scan_projective_collection_selected(
        files,
        ProjectiveCollectionPolicy {
            local: p.local,
            filter: p.filter,
            registration: p.registration.registration,
            budgets: policy.budgets,
        },
        p.spatial,
        p.sampling,
        Some(p.registration),
        budget,
        cancel,
    )
}

/// Recursive anchored search preserving per-path native decode requests.
///
/// # Errors
/// Invalid policy/request paths, source/work/resource refusal or cancellation.
pub fn scan_projective_local_collection_roots_anchored_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveAnchoredCollectionPolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    let p = policy.search;
    scan_projective_roots_selected(
        roots,
        traversal,
        ProjectiveCollectionPolicy {
            local: p.local,
            filter: p.filter,
            registration: p.registration.registration,
            budgets: policy.budgets,
        },
        p.spatial,
        p.sampling,
        Some(p.registration),
        budget,
        request_for,
        cancel,
    )
}
/// Recursive anchored search with native default requests.
///
/// # Errors
/// Invalid policy, source/work/resource refusal or cancellation.
pub fn scan_projective_local_collection_roots_anchored(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveAnchoredCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectiveFileEvidence>, ScanError> {
    scan_projective_local_collection_roots_anchored_with_requests(
        roots,
        traversal,
        policy,
        budget,
        |path| DecodeRequest::new(path),
        cancel,
    )
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectivePhotometricCollectionPolicy {
    pub search: crate::local_scan::ProjectivePhotometricFilePolicy,
    pub budgets: FileFeatureBudgets,
}
/// Indexed perspective/color candidates with fresh native file confirmation.
/// Geometry-only proposals cannot bypass the bidirectional fitted residuals.
///
/// # Errors
/// Invalid policy, collection limits, changed sources or cancellation. Individual
/// decode/model refusals remain attributed to their file or pair.
pub fn scan_projective_local_collection_photometric(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectivePhotometricCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectivePhotometricFileEvidence>, ScanError> {
    validate_projective_photometric_collection(&config)?;
    let p = config.search.search;
    let policy = LocalCollectionPolicy {
        search: p.local.into(),
        budgets: config.budgets,
    };
    scan_collection_with_verifiers(
        files,
        &policy,
        p.spatial,
        budget,
        cancel,
        |left, right, cancel| {
            let correspondences = crate::local::match_features(left, right, p.local.matching, cancel)?;
            let geometry = match p.sampling {
                Some(sample) => crate::geometry::verify_projective_sampled(
                    &correspondences,
                    p.local.geometry,
                    sample,
                    cancel,
                )?,
                None => crate::geometry::verify_projective(&correspondences, p.local.geometry, cancel)?,
            };
            if geometry.is_some() {
                return Ok(None);
            }
            Ok(Some(crate::local_scan::ProjectivePhotometricFileEvidence {
                correspondences,
                geometry,
                registered_transform: None,
                pixels: None,
                fit_failure: None,
                unfitted: None,
                candidate: false,
            }))
        },
        |left, right, cancel| {
            crate::local_scan::compare_local_files_projective_photometric(
                left,
                right,
                config.search,
                budget,
                cancel,
            )
        },
    )
}

fn validate_projective_photometric_collection(
    config: &ProjectivePhotometricCollectionPolicy,
) -> Result<(), ScanError> {
    let p = config.search.search;
    crate::warp::validate_photometric_policy(config.search.photometric)
        .map_err(|_| ScanError::InvalidPolicy)?;
    crate::warp::validate_registration_trust_policy(p.registration).map_err(|_| ScanError::InvalidPolicy)?;
    crate::warp::validate_filter_policy(p.filter.filter).map_err(|_| ScanError::InvalidPolicy)?;
    if p.local.geometry.min_inliers < 4 {
        return Err(ScanError::InvalidPolicy);
    }
    if let Some(sample) = p.sampling {
        if sample.seed == 0 || sample.trials == 0 {
            return Err(ScanError::InvalidPolicy);
        }
        if sample.trials > p.local.geometry.max_hypotheses {
            return Err(ScanError::Budget);
        }
    }
    if let Some(grid) = p.spatial {
        validate_spatial(grid, p.local.extract.max_features).map_err(|_| ScanError::InvalidPolicy)?;
    }
    validate_search(p.local.into())?;
    Ok(())
}

/// Recursive perspective/color search with caller-preserved frame requests.
///
/// # Errors
/// Invalid policies or substituted request paths, traversal/collection limits,
/// source changes or cancellation; individual decode refusals remain attributed.
pub fn scan_projective_local_collection_roots_photometric_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectivePhotometricCollectionPolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectivePhotometricFileEvidence>, ScanError> {
    validate_projective_photometric_collection(&policy)?;
    scan_roots_with_collection(
        roots,
        traversal,
        policy.budgets.max_files,
        request_for,
        cancel,
        |requests, cancel| scan_projective_local_collection_photometric(requests, policy, budget, cancel),
    )
}
/// Recursive perspective/color search with native default selected frames.
///
/// # Errors
/// Invalid policy, traversal/collection refusal or cancellation.
pub fn scan_projective_local_collection_roots_photometric(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectivePhotometricCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectivePhotometricFileEvidence>, ScanError> {
    scan_projective_local_collection_roots_photometric_with_requests(
        roots,
        traversal,
        policy,
        budget,
        |path| DecodeRequest::new(path),
        cancel,
    )
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectivePortfolioCollectionPolicy {
    pub search: crate::local_scan::ProjectivePortfolioFilePolicy,
    pub budgets: FileFeatureBudgets,
}
/// Indexed local candidates with two separately verified registration lanes.
/// No geometry-only proposal can bypass fresh full-grid file confirmation.
///
/// # Errors
/// Invalid policy, collection/source/work refusal or cancellation; individual
/// file and pair errors remain attributed through the shared collection lifecycle.
pub fn scan_projective_local_collection_portfolio(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectivePortfolioCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectivePortfolioFileEvidence>, ScanError> {
    let p = config.search;
    crate::local_scan::validate_projective_portfolio_file_policy(&p).map_err(|error| match error {
        LocalFileError::Pixels(crate::warp::WarpError::Budget)
        | LocalFileError::Geometry(crate::geometry::GeometryError::Budget) => ScanError::Budget,
        _ => ScanError::InvalidPolicy,
    })?;
    let policy = LocalCollectionPolicy {
        search: p.local.into(),
        budgets: config.budgets,
    };
    scan_collection_with_verifiers(
        files,
        &policy,
        p.spatial,
        budget,
        cancel,
        |left, right, cancel| {
            let correspondences = crate::local::match_features(left, right, p.local.matching, cancel)?;
            let geometry = match p.sampling {
                Some(sample) => crate::geometry::verify_projective_sampled(
                    &correspondences,
                    p.local.geometry,
                    sample,
                    cancel,
                )?,
                None => crate::geometry::verify_projective(&correspondences, p.local.geometry, cancel)?,
            };
            if geometry.is_some() {
                return Ok(None);
            }
            Ok(Some(crate::local_scan::ProjectivePortfolioFileEvidence {
                correspondences,
                geometry,
                pixels: None,
                accepted_lanes: [false; 2],
                candidate: false,
            }))
        },
        |left, right, cancel| {
            crate::local_scan::compare_local_files_projective_portfolio(left, right, p, budget, cancel)
        },
    )
}

/// Recursive two-lane registration search preserving per-path decode requests.
/// Policies are admitted before traversal or request-factory invocation.
///
/// # Errors
/// Invalid policy/request paths, cumulative work or collection limits, source
/// changes, or cancellation; individual decode refusals remain attributed.
pub fn scan_projective_local_collection_roots_portfolio_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectivePortfolioCollectionPolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectivePortfolioFileEvidence>, ScanError> {
    crate::local_scan::validate_projective_portfolio_file_policy(&policy.search).map_err(
        |error| match error {
            LocalFileError::Pixels(crate::warp::WarpError::Budget)
            | LocalFileError::Geometry(crate::geometry::GeometryError::Budget) => ScanError::Budget,
            _ => ScanError::InvalidPolicy,
        },
    )?;
    scan_roots_with_collection(
        roots,
        traversal,
        policy.budgets.max_files,
        request_for,
        cancel,
        |requests, cancel| scan_projective_local_collection_portfolio(requests, policy, budget, cancel),
    )
}
/// Recursive two-lane registration search with native default frame requests.
///
/// # Errors
/// Invalid policy, traversal/collection refusal, source changes or cancellation.
pub fn scan_projective_local_collection_roots_portfolio(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectivePortfolioCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectivePortfolioFileEvidence>, ScanError> {
    scan_projective_local_collection_roots_portfolio_with_requests(
        roots,
        traversal,
        policy,
        budget,
        |path| DecodeRequest::new(path),
        cancel,
    )
}

/// Indexed search using the same managed pyramid as fresh file confirmation.
#[derive(Debug, Clone, Copy)]
pub struct ProjectivePyramidCollectionPolicy {
    pub search: crate::local_scan::ProjectivePyramidPhotometricFilePolicy,
    pub budgets: FileFeatureBudgets,
}
fn validate_projective_pyramid_collection(
    config: &ProjectivePyramidCollectionPolicy,
) -> Result<(), ScanError> {
    crate::local_scan::validate_projective_pyramid_file_policy(&config.search).map_err(|e| match e {
        LocalFileError::InvalidPolicy => ScanError::InvalidPolicy,
        _ => ScanError::Budget,
    })?;
    validate_search(config.search.local.into())
}
/// Retrieve scale-pyramid descriptor pairs and confirm through the public file
/// API. The shared collection lifecycle retains source and cancellation checks.
///
/// # Errors
/// Invalid policy, collection/work/memory limits or cancellation. Individual
/// decode/geometry/pixel refusals remain attributed to their source or pair.
pub fn scan_projective_local_collection_pyramid(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectivePyramidCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectivePhotometricFileEvidence>, ScanError> {
    validate_projective_pyramid_collection(&config)?;
    let p = config.search;
    let policy = LocalCollectionPolicy {
        search: p.local.into(),
        budgets: config.budgets,
    };
    let pyramid = crate::pyramid::PyramidPolicy {
        local: p.local.extract,
        max_levels: p.max_levels,
        max_total_pixels: p.max_total_pixels,
        max_total_features: p.max_total_features,
    };
    scan_collection_with_extractor(
        files,
        &policy,
        cancel,
        |request, source, cancel| {
            extract_file_with(request, source, policy.search, budget, cancel, |view, cancel| {
                crate::pyramid::extract_oriented_pyramid_managed(view, pyramid, budget, cancel)
            })
        },
        |left, right, cancel| {
            let correspondences = crate::local::match_features(left, right, p.local.matching, cancel)?;
            let geometry = crate::geometry::verify_projective_sampled(
                &correspondences,
                p.local.geometry,
                p.sampling,
                cancel,
            )?;
            if geometry.is_some() {
                return Ok(None);
            }
            Ok(Some(crate::local_scan::ProjectivePhotometricFileEvidence {
                correspondences,
                geometry,
                registered_transform: None,
                pixels: None,
                fit_failure: None,
                unfitted: None,
                candidate: false,
            }))
        },
        |left, right, cancel| {
            crate::local_scan::compare_local_files_projective_pyramid_photometric(
                left, right, p, budget, cancel,
            )
        },
    )
}
/// Recursively search managed pyramid features with caller-selected frames.
///
/// # Errors
/// Policy/traversal/collection limits, substituted paths or cancellation.
pub fn scan_projective_local_collection_roots_pyramid_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectivePyramidCollectionPolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectivePhotometricFileEvidence>, ScanError> {
    validate_projective_pyramid_collection(&policy)?;
    scan_roots_with_collection(
        roots,
        traversal,
        policy.budgets.max_files,
        request_for,
        cancel,
        |requests, cancel| scan_projective_local_collection_pyramid(requests, policy, budget, cancel),
    )
}
/// Recursively search native default selected frames with managed pyramids.
///
/// # Errors
/// Invalid policy, traversal/collection refusal or cancellation.
pub fn scan_projective_local_collection_roots_pyramid(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectivePyramidCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectivePhotometricFileEvidence>, ScanError> {
    scan_projective_local_collection_roots_pyramid_with_requests(
        roots,
        traversal,
        policy,
        budget,
        |path| DecodeRequest::new(path),
        cancel,
    )
}

/// Descriptor proposals from both families, followed by shared-view confirmation.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveComplementaryCollectionPolicy {
    pub search: crate::local_scan::ProjectiveComplementaryFilePolicy,
    pub budgets: FileFeatureBudgets,
}
fn validate_projective_complementary_collection(
    config: &ProjectiveComplementaryCollectionPolicy,
) -> Result<(), ScanError> {
    crate::local_scan::validate_projective_complementary_file_policy(&config.search).map_err(
        |e| match e {
            LocalFileError::InvalidPolicy => ScanError::InvalidPolicy,
            _ => ScanError::Budget,
        },
    )?;
    validate_search(config.search.pyramid.local.into())
}
/// Index the union of compatible descriptors from both searches. Merged features
/// are proposals only: ambiguous duplicate descriptors are never used to reject
/// a pair geometrically. Each retrieved pair receives both original searches,
/// preserving their individual matching and verification policies.
///
/// # Errors
/// Invalid policy, file/index/work/memory limits or cancellation. Source and
/// phase refusals retain the existing per-file/per-pair attribution and guards.
pub fn scan_projective_local_collection_complementary(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveComplementaryCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectiveComplementaryFileEvidence>, ScanError> {
    validate_projective_complementary_collection(&config)?;
    let c = config.search;
    let p = c.pyramid;
    let r = c.registration;
    let mut proposal = p.local;
    proposal.matching.max_distance = proposal.matching.max_distance.max(r.local.matching.max_distance);
    proposal.geometry.min_inliers = proposal.geometry.min_inliers.min(r.local.geometry.min_inliers);
    let policy = LocalCollectionPolicy {
        search: proposal.into(),
        budgets: config.budgets,
    };
    let pyramid = crate::pyramid::PyramidPolicy {
        local: p.local.extract,
        max_levels: p.max_levels,
        max_total_pixels: p.max_total_pixels,
        max_total_features: p.max_total_features,
    };
    let capacity = p
        .max_total_features
        .checked_add(r.local.extract.max_features)
        .ok_or(ScanError::Budget)?;
    let bytes = capacity
        .checked_mul(std::mem::size_of::<Feature>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(ScanError::Budget)?;
    scan_collection_with_extractor(
        files,
        &policy,
        cancel,
        |request, source, cancel| {
            extract_file_with(request, source, policy.search, budget, cancel, |view, cancel| {
                let retained = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
                let ordinary = extract_search_features(view, r.local.extract, r.spatial, budget, cancel)?;
                let multiscale =
                    crate::pyramid::extract_oriented_pyramid_managed(view, pyramid, budget, cancel)?;
                let count = ordinary
                    .len()
                    .checked_add(multiscale.len())
                    .ok_or(LocalError::Budget)?;
                if count > capacity {
                    return Err(LocalError::Budget);
                }
                let mut combined = Vec::new();
                combined
                    .try_reserve_exact(count)
                    .map_err(|_| LocalError::Budget)?;
                for feature in ordinary.iter().chain(multiscale.iter()) {
                    if cancel() {
                        return Err(LocalError::Cancelled);
                    }
                    combined.push(*feature);
                }
                retained.try_adopt(combined).map_err(|_| LocalError::Budget)
            })
        },
        |_, _, _| Ok(None),
        |left, right, cancel| {
            crate::local_scan::compare_local_files_projective_complementary(left, right, c, budget, cancel)
        },
    )
}
/// Recursive complementary search with caller-selected frame requests.
///
/// # Errors
/// Invalid policy, traversal/index/source/work limits, path substitution or cancellation.
pub fn scan_projective_local_collection_roots_complementary_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveComplementaryCollectionPolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectiveComplementaryFileEvidence>, ScanError>
{
    validate_projective_complementary_collection(&policy)?;
    scan_roots_with_collection(
        roots,
        traversal,
        policy.budgets.max_files,
        request_for,
        cancel,
        |requests, cancel| scan_projective_local_collection_complementary(requests, policy, budget, cancel),
    )
}
/// Recursive complementary search with native default selected frames.
///
/// # Errors
/// Invalid policy, traversal/index/source/work refusal or cancellation.
pub fn scan_projective_local_collection_roots_complementary(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveComplementaryCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryLocalCollectionReport<crate::local_scan::ProjectiveComplementaryFileEvidence>, ScanError>
{
    scan_projective_local_collection_roots_complementary_with_requests(
        roots,
        traversal,
        policy,
        budget,
        |path| DecodeRequest::new(path),
        cancel,
    )
}
