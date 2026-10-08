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
    scan_collection_with_typed_extractor(
        files,
        policy,
        None,
        cancel,
        extract,
        precheck,
        compare,
        |features, cancel| {
            descriptor_file_pairs_shared(
                features.iter().map(|(&id, f)| (id, f.clone())),
                policy.search.local.matching.max_distance,
                policy.search.local.geometry.min_inliers,
                policy.budgets,
                cancel,
            )
            .map_err(|error| mapped(&error))
        },
    )
}
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn scan_collection_with_typed_extractor<E, F>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: &LocalCollectionPolicy,
    proposal_budget: Option<&MemoryBudget>,
    cancel: impl Fn() -> bool,
    extract: impl Fn(
        &DecodeRequest,
        &ContentSnapshot,
        &dyn Fn() -> bool,
    ) -> Result<rrrah_core::SharedBuffer<F>, LocalFileError>,
    precheck: impl Fn(&[F], &[F], &dyn Fn() -> bool) -> Result<Option<E>, LocalFileError>,
    compare: impl Fn(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<E, LocalFileError>,
    retrieve: impl Fn(
        &HashMap<u64, rrrah_core::SharedBuffer<F>>,
        &dyn Fn() -> bool,
    ) -> Result<crate::local_index::DescriptorFilePairs, ScanError>,
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
    // Keep proposal storage admitted while fresh comparisons retain their evidence.
    let mut proposal_credit = if let Some(budget) = proposal_budget {
        let possible_pairs = features
            .len()
            .checked_mul(features.len().saturating_sub(1))
            .map(|n| n / 2)
            .ok_or(ScanError::Budget)?;
        let admitted_pairs = policy.budgets.max_pairs.min(possible_pairs);
        let bytes = admitted_pairs
            .checked_mul(std::mem::size_of::<(u64, u64)>())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(ScanError::Budget)?;
        Some(budget.try_reserve(bytes).map_err(|_| ScanError::Budget)?)
    } else {
        None
    };
    let retrieved = retrieve(&features, &cancelled)?;
    if let Some(credit) = proposal_credit.as_mut() {
        let bytes = retrieved
            .pairs
            .capacity()
            .checked_mul(std::mem::size_of::<(u64, u64)>())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(ScanError::Budget)?;
        credit.ensure_bytes(bytes).map_err(|_| ScanError::Budget)?;
    }
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
fn extract_file_with<F>(
    request: &DecodeRequest,
    source: &ContentSnapshot,
    policy: LocalSearchPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    extract: impl Fn(
        &crate::linear::LinearRgbaView<'_>,
        &dyn Fn() -> bool,
    ) -> Result<rrrah_core::SharedBuffer<F>, LocalError>,
) -> Result<rrrah_core::SharedBuffer<F>, LocalFileError> {
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
    scan_complementary_with_verifier(files, config, budget, cancel, |left, right, cancel| {
        crate::local_scan::compare_local_files_projective_complementary(
            left,
            right,
            config.search,
            budget,
            cancel,
        )
    })
}
fn scan_complementary_with_verifier<E>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveComplementaryCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    verify: impl Fn(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<E, LocalFileError>,
) -> Result<LocalCollectionReport<E>, ScanError> {
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
        verify,
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

/// Same descriptor proposal index; registration and both pyramid filters confirm
/// each pair independently of merged descriptor ambiguity.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveComplementaryFilterPortfolioCollectionPolicy {
    pub search: crate::local_scan::ProjectiveComplementaryFilterPortfolioPolicy,
    pub budgets: FileFeatureBudgets,
}
fn validate_complementary_filter_collection(
    policy: &ProjectiveComplementaryFilterPortfolioCollectionPolicy,
) -> Result<(), ScanError> {
    crate::local_scan::validate_projective_complementary_filter_portfolio_policy(&policy.search).map_err(
        |e| match e {
            LocalFileError::InvalidPolicy => ScanError::InvalidPolicy,
            _ => ScanError::Budget,
        },
    )?;
    validate_projective_complementary_collection(&ProjectiveComplementaryCollectionPolicy {
        search: policy.search.searches,
        budgets: policy.budgets,
    })
}
/// Scan explicit requests with shared lifecycle and all three verification phases.
///
/// # Errors
/// Invalid policy, index/work/memory limits or cancellation. Pair failures and
/// changed sources retain their attribution and cannot produce partial success.
pub fn scan_projective_local_collection_complementary_filter_portfolio(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    policy: ProjectiveComplementaryFilterPortfolioCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectiveComplementaryFilterPortfolioEvidence>, ScanError>
{
    validate_complementary_filter_collection(&policy)?;
    scan_complementary_with_verifier(
        files,
        ProjectiveComplementaryCollectionPolicy {
            search: policy.search.searches,
            budgets: policy.budgets,
        },
        budget,
        cancel,
        |left, right, cancel| {
            crate::local_scan::compare_local_files_projective_complementary_filter_portfolio(
                left,
                right,
                policy.search,
                budget,
                cancel,
            )
        },
    )
}
/// Recursive three-phase scan preserving caller-selected frames and path identity.
///
/// # Errors
/// Invalid policy, traversal/index/work limits, cancellation or path substitution.
pub fn scan_projective_local_collection_roots_complementary_filter_portfolio_with_requests(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveComplementaryFilterPortfolioCollectionPolicy,
    budget: &MemoryBudget,
    request_for: impl Fn(&std::path::Path) -> DecodeRequest,
    cancel: impl Fn() -> bool,
) -> Result<
    DirectoryLocalCollectionReport<crate::local_scan::ProjectiveComplementaryFilterPortfolioEvidence>,
    ScanError,
> {
    validate_complementary_filter_collection(&policy)?;
    scan_roots_with_collection(
        roots,
        traversal,
        policy.budgets.max_files,
        request_for,
        cancel,
        |requests, cancel| {
            scan_projective_local_collection_complementary_filter_portfolio(requests, policy, budget, cancel)
        },
    )
}
/// Recursive three-phase scan with native default frame requests.
///
/// # Errors
/// Invalid policy, traversal/index/work limits or cancellation.
pub fn scan_projective_local_collection_roots_complementary_filter_portfolio(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    policy: ProjectiveComplementaryFilterPortfolioCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    DirectoryLocalCollectionReport<crate::local_scan::ProjectiveComplementaryFilterPortfolioEvidence>,
    ScanError,
> {
    scan_projective_local_collection_roots_complementary_filter_portfolio_with_requests(
        roots,
        traversal,
        policy,
        budget,
        |path| DecodeRequest::new(path),
        cancel,
    )
}

/// Explicit managed gradient recipe proposals with fresh native file confirmation.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveGradientCollectionPolicy {
    pub search: crate::local_scan::ProjectiveGradientPyramidFilePolicy,
    pub recipe: crate::gradient::GradientCellRecipe,
    pub budgets: FileFeatureBudgets,
    /// Conservative cumulative admission of N squared descriptor comparisons.
    pub max_retrieval_comparisons: u64,
}
/// Retrieve every within-radius descriptor proposal, including ambiguous matches.
/// Source changes invalidate all affected batch decisions; a proposal alone never accepts a copy.
pub fn scan_projective_local_collection_gradient(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveGradientCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectivePhotometricFileEvidence>, ScanError> {
    scan_gradient_collection_selected(files, config, None, budget, cancel)
}

/// Gradient proposals and file confirmation with explicit per-level spatial quotas.
/// Retrieval and confirmation use the same selection and descriptor recipes.
/// A proposal or fitted model alone never accepts a copy.
///
/// # Errors
/// Invalid grid/policy, cumulative retrieval limits, source failures or cancellation.
pub fn scan_projective_local_collection_spatial_gradient(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveGradientCollectionPolicy,
    spatial: crate::local_scan::SpatialFeaturePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectivePhotometricFileEvidence>, ScanError> {
    scan_gradient_collection_selected(files, config, Some(spatial), budget, cancel)
}

/// Explicit intermediate-scale gradient retrieval with fresh file confirmation.
/// Both phases use the same factors and descriptor recipe. Batch-wide source
/// validation removes affected edges if any input changes.
///
/// # Errors
/// Invalid factors/policy before source access, resource refusal or cancellation.
pub fn scan_projective_local_collection_gradient_scales(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveGradientCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectivePhotometricFileEvidence>, ScanError> {
    if factors.is_empty() || factors.len() > config.search.search.max_levels {
        return Err(ScanError::Budget);
    }
    let mut previous = 0.;
    for &factor in factors {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        if !factor.is_finite() || factor < 1. || factor <= previous {
            return Err(ScanError::InvalidPolicy);
        }
        previous = factor;
    }
    scan_gradient_collection_with_confirmation(
        files,
        config,
        None,
        Some(factors),
        None,
        None,
        budget,
        cancel,
        |left, right, cancel| {
            crate::local_scan::compare_local_files_projective_gradient_scales_with_recipe(
                left,
                right,
                config.search,
                config.recipe,
                factors,
                budget,
                cancel,
            )
        },
    )
}

/// Intermediate-scale spatial proposals followed by matching file confirmation.
/// Both stages use the same factors/grid/recipe and batch source guards.
/// # Errors
/// Invalid selection before IO, cumulative resource refusal or cancellation.
pub fn scan_projective_local_collection_spatial_gradient_scales(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveGradientCollectionPolicy,
    factors: &[f64],
    spatial: crate::local_scan::SpatialFeaturePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectivePhotometricFileEvidence>, ScanError> {
    if factors.is_empty() || factors.len() > config.search.search.max_levels {
        return Err(ScanError::Budget);
    }
    let mut previous = 0.;
    for &factor in factors {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        if !factor.is_finite() || factor < 1. || factor <= previous {
            return Err(ScanError::InvalidPolicy);
        }
        previous = factor;
    }
    scan_gradient_collection_with_confirmation(
        files,
        config,
        Some(spatial),
        Some(factors),
        None,
        None,
        budget,
        cancel,
        |left, right, cancel| {
            crate::local_scan::compare_local_files_projective_spatial_gradient_scales_with_recipe(
                left,
                right,
                config.search,
                config.recipe,
                factors,
                spatial,
                budget,
                cancel,
            )
        },
    )
}

fn scan_gradient_collection_selected(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveGradientCollectionPolicy,
    spatial: Option<crate::local_scan::SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectivePhotometricFileEvidence>, ScanError> {
    scan_gradient_collection_with_confirmation(
        files,
        config,
        spatial,
        None,
        None,
        None,
        budget,
        cancel,
        |left, right, cancel| {
            if let Some(grid) = spatial {
                crate::local_scan::compare_local_files_projective_spatial_gradient_pyramid_with_recipe(
                    left,
                    right,
                    config.search,
                    config.recipe,
                    grid,
                    budget,
                    cancel,
                )
            } else {
                crate::local_scan::compare_local_files_projective_gradient_pyramid_with_recipe(
                    left,
                    right,
                    config.search,
                    config.recipe,
                    budget,
                    cancel,
                )
            }
        },
    )
}

/// Spatial gradient collection policy with independently verified local regions.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveSpatialGradientRegionCollectionPolicy {
    pub gradient: ProjectiveGradientCollectionPolicy,
    pub spatial: crate::local_scan::SpatialFeaturePolicy,
    pub grid: (u32, u32),
    pub max_regions: usize,
    /// Whole-gradient confirmation plus both region grids, admitted before I/O.
    pub max_total_sample_pairs: u64,
}

/// Whole-image and local region evidence retain their independent decisions.
#[derive(Debug)]
pub struct ProjectiveSpatialGradientRegionsEvidence {
    pub whole: crate::local_scan::ProjectivePhotometricFileEvidence,
    pub regions: Option<crate::local_scan::ProjectiveTransformedRegionsFileEvidence>,
}

/// Retrieve spatial-gradient proposals, then confirm both-image region grids.
/// The common batch source guard covers both fresh file confirmation phases;
/// local supports never change the whole-image candidate decision.
///
/// # Errors
/// Invalid policy, cumulative limits, source failures or cancellation.
pub fn scan_projective_local_collection_spatial_gradient_regions(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveSpatialGradientRegionCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<ProjectiveSpatialGradientRegionsEvidence>, ScanError> {
    scan_spatial_gradient_regions_selected(files, config, None, budget, cancel)
}

/// Spatial intermediate-scale proposals with independent local confirmations.
/// Whole and region candidates remain separate; the batch source guard covers
/// both phases. Factors form part of the explicit extraction recipe.
/// # Errors
/// Invalid factors/grid/policy before source IO, limits or cancellation.
pub fn scan_projective_local_collection_spatial_gradient_scale_regions(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveSpatialGradientRegionCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<ProjectiveSpatialGradientRegionsEvidence>, ScanError> {
    if factors.is_empty() || factors.len() > config.gradient.search.search.max_levels {
        return Err(ScanError::Budget);
    }
    let mut previous = 0.;
    for &factor in factors {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        if !factor.is_finite() || factor < 1. || factor <= previous {
            return Err(ScanError::InvalidPolicy);
        }
        previous = factor;
    }
    scan_spatial_gradient_regions_selected(files, config, Some(factors), budget, cancel)
}

fn scan_spatial_gradient_regions_selected(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveSpatialGradientRegionCollectionPolicy,
    scales: Option<&[f64]>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<ProjectiveSpatialGradientRegionsEvidence>, ScanError> {
    if config.grid.0 == 0 || config.grid.1 == 0 || config.max_regions == 0 {
        return Err(ScanError::InvalidPolicy);
    }
    let cells = u64::from(config.grid.0)
        .checked_mul(u64::from(config.grid.1))
        .and_then(|n| n.checked_mul(2))
        .ok_or(ScanError::Budget)?;
    if cells > u64::try_from(config.max_regions).map_err(|_| ScanError::Budget)? {
        return Err(ScanError::Budget);
    }
    let samples = config.gradient.search.search.filter.filter.max_sample_pairs;
    let total = cells
        .checked_add(2)
        .and_then(|n| n.checked_mul(samples))
        .ok_or(ScanError::Budget)?;
    if total > config.max_total_sample_pairs {
        return Err(ScanError::Budget);
    }
    let regional = crate::local_scan::ProjectivePyramidRegionsFilePolicy {
        search: config.gradient.search.search,
        max_regions: config.max_regions,
        max_total_sample_pairs: cells
            .checked_add(1)
            .and_then(|n| n.checked_mul(samples))
            .ok_or(ScanError::Budget)?,
    };
    scan_gradient_collection_with_confirmation(
        files,
        config.gradient,
        Some(config.spatial),
        scales,
        None,
        None,
        budget,
        cancel,
        |left, right, cancel| {
            let whole = if let Some(factors) = scales {
                crate::local_scan::compare_local_files_projective_spatial_gradient_scales_with_recipe(
                    left,
                    right,
                    config.gradient.search,
                    config.gradient.recipe,
                    factors,
                    config.spatial,
                    budget,
                    cancel,
                )?
            } else {
                crate::local_scan::compare_local_files_projective_spatial_gradient_pyramid_with_recipe(
                    left,
                    right,
                    config.gradient.search,
                    config.gradient.recipe,
                    config.spatial,
                    budget,
                    cancel,
                )?
            };
            let model = whole
                .registered_transform
                .or_else(|| whole.geometry.as_ref().map(|v| v.transform));
            let regions = if let Some(model) = model {
                Some(
                    crate::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
                        left,
                        right,
                        model,
                        regional,
                        config.grid,
                        budget,
                        cancel,
                    )?,
                )
            } else {
                None
            };
            Ok(ProjectiveSpatialGradientRegionsEvidence { whole, regions })
        },
    )
}

fn gradient_file_id_scratch<T>(
    features: &HashMap<u64, T>,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<(rrrah_core::SharedBuffer<u64>, Vec<u64>, rrrah_core::Reservation), ScanError> {
    if cancel() {
        return Err(ScanError::Cancelled);
    }
    let bytes = features
        .len()
        .checked_mul(std::mem::size_of::<u64>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(ScanError::Budget)?;
    let file_credit = budget.try_reserve(bytes).map_err(|_| ScanError::Budget)?;
    let mut insufficient_credit = budget.try_reserve(bytes).map_err(|_| ScanError::Budget)?;
    let mut files = Vec::new();
    files
        .try_reserve_exact(features.len())
        .map_err(|_| ScanError::Budget)?;
    for &id in features.keys() {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        files.push(id);
    }
    files.sort_unstable();
    let files = file_credit.try_adopt(files).map_err(|_| ScanError::Budget)?;
    let mut insufficient = Vec::new();
    insufficient
        .try_reserve_exact(features.len())
        .map_err(|_| ScanError::Budget)?;
    insufficient_credit
        .ensure_bytes(
            insufficient
                .capacity()
                .checked_mul(std::mem::size_of::<u64>())
                .and_then(|n| u64::try_from(n).ok())
                .ok_or(ScanError::Budget)?,
        )
        .map_err(|_| ScanError::Budget)?;
    if cancel() {
        return Err(ScanError::Cancelled);
    }
    Ok((files, insufficient, insufficient_credit))
}

#[cfg(test)]
mod gradient_metadata_tests {
    use super::*;
    #[test]
    fn gradient_id_scratch_exact_limit_cancellation_and_ownership() {
        let features = HashMap::from([(u64::MAX, ()), (0, ())]);
        let bytes = 4 * std::mem::size_of::<u64>() as u64;
        let budget = MemoryBudget::new(bytes);
        let calls = Cell::new(0);
        let (ids, mut insufficient, credit) = gradient_file_id_scratch(&features, &budget, &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        assert_eq!(&*ids, &[0, u64::MAX]);
        assert_eq!(budget.used(), bytes);
        insufficient.extend([0, u64::MAX]);
        assert_eq!(insufficient.len(), 2);
        let clone = ids.clone();
        drop(ids);
        drop(insufficient);
        drop(credit);
        assert_eq!(budget.used(), bytes / 2);
        drop(clone);
        assert_eq!(budget.used(), 0);
        let short = MemoryBudget::new(bytes - 1);
        assert!(matches!(
            gradient_file_id_scratch(&features, &short, &|| false),
            Err(ScanError::Budget)
        ));
        assert_eq!(short.used(), 0);
        for stop in 1..=calls.get() {
            let current = Cell::new(0);
            assert!(matches!(
                gradient_file_id_scratch(&features, &budget, &|| {
                    current.set(current.get() + 1);
                    current.get() == stop
                }),
                Err(ScanError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
        let zero = MemoryBudget::new(0);
        drop(gradient_file_id_scratch(&HashMap::<u64, ()>::new(), &zero, &|| false).unwrap());
        assert_eq!(zero.peak(), 0);
    }
}

fn scan_gradient_collection_with_confirmation<E>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveGradientCollectionPolicy,
    spatial: Option<crate::local_scan::SpatialFeaturePolicy>,
    scales: Option<&[f64]>,
    area_taps: Option<u64>,
    candidate_union: Option<(
        crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
        &[u32],
    )>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    confirm: impl Fn(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<E, LocalFileError>,
) -> Result<LocalCollectionReport<E>, ScanError> {
    if let Some(grid) = spatial {
        crate::local_scan::validate_spatial(grid, config.search.search.local.extract.max_features).map_err(
            |e| match e {
                LocalFileError::InvalidPolicy => ScanError::InvalidPolicy,
                _ => ScanError::Budget,
            },
        )?;
    }
    crate::local_scan::validate_projective_gradient_pyramid_file_policy(&config.search).map_err(
        |e| match e {
            LocalFileError::InvalidPolicy | LocalFileError::Features(LocalError::Invalid) => {
                ScanError::InvalidPolicy
            }
            _ => ScanError::Budget,
        },
    )?;
    let p = config.search.search;
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
    scan_collection_with_typed_extractor(
        files,
        &policy,
        Some(budget),
        cancel,
        |request, source, cancel| {
            extract_file_with(request, source, policy.search, budget, cancel, |view, cancel| {
                if let Some((union, radii)) = candidate_union {
                    extract_candidate_union_portfolio_proposals(
                        view,
                        union,
                        radii,
                        scales.ok_or(LocalError::Invalid)?,
                        budget,
                        cancel,
                    )
                } else if let Some(factors) = scales {
                    if let (Some(grid), Some(limit)) = (spatial, area_taps) {
                        return crate::gradient_scale::extract_spatial_gradient_scales_area_managed(
                            view,
                            pyramid,
                            factors,
                            config.search.max_total_gradient_samples,
                            config.recipe,
                            (grid.columns, grid.rows, grid.max_per_cell),
                            limit,
                            budget,
                            cancel,
                        );
                    }
                    if let Some(grid) = spatial {
                        crate::gradient_scale::extract_spatial_gradient_scales_managed(
                            view,
                            pyramid,
                            factors,
                            config.search.max_total_gradient_samples,
                            config.recipe,
                            (grid.columns, grid.rows, grid.max_per_cell),
                            budget,
                            cancel,
                        )
                    } else {
                        crate::gradient_scale::extract_gradient_scales_managed(
                            view,
                            pyramid,
                            factors,
                            config.search.max_total_gradient_samples,
                            config.recipe,
                            budget,
                            cancel,
                        )
                    }
                } else if let Some(grid) = spatial {
                    crate::gradient::extract_spatial_gradient_pyramid_with_recipe_managed(
                        view,
                        pyramid,
                        config.search.max_total_gradient_samples,
                        config.recipe,
                        (grid.columns, grid.rows, grid.max_per_cell),
                        budget,
                        cancel,
                    )
                } else {
                    crate::gradient::extract_gradient_pyramid_with_recipe_managed(
                        view,
                        pyramid,
                        config.search.max_total_gradient_samples,
                        config.recipe,
                        budget,
                        cancel,
                    )
                }
            })
        },
        |_, _, _| Ok(None),
        confirm,
        |features, cancel| {
            let count = features
                .values()
                .try_fold(0_usize, |n, f| n.checked_add(f.len()).ok_or(ScanError::Budget))?;
            let bytes = count
                .checked_mul(
                    std::mem::size_of::<(u64, crate::gradient::GradientDescriptor)>()
                        + std::mem::size_of::<u64>(),
                )
                .and_then(|n| u64::try_from(n).ok())
                .ok_or(ScanError::Budget)?;
            let mut retained = budget.try_reserve(bytes).map_err(|_| ScanError::Budget)?;
            let mut entries = Vec::new();
            let mut owners = Vec::new();
            entries.try_reserve_exact(count).map_err(|_| ScanError::Budget)?;
            owners.try_reserve_exact(count).map_err(|_| ScanError::Budget)?;
            let actual_bytes = entries
                .capacity()
                .checked_mul(std::mem::size_of::<(u64, crate::gradient::GradientDescriptor)>())
                .and_then(|n| {
                    owners
                        .capacity()
                        .checked_mul(std::mem::size_of::<u64>())
                        .and_then(|m| n.checked_add(m))
                })
                .and_then(|n| u64::try_from(n).ok())
                .ok_or(ScanError::Budget)?;
            retained
                .ensure_bytes(actual_bytes)
                .map_err(|_| ScanError::Budget)?;
            let (files, mut insufficient, _insufficient_credit) =
                gradient_file_id_scratch(features, budget, &cancel)?;
            for &id in files.iter() {
                if features[&id].len() < p.local.geometry.min_inliers {
                    insufficient.try_reserve_exact(1).map_err(|_| ScanError::Budget)?;
                    insufficient.push(id);
                }
                for feature in features[&id].iter() {
                    if cancel() {
                        return Err(ScanError::Cancelled);
                    }
                    let entry = u64::try_from(entries.len()).map_err(|_| ScanError::Budget)?;
                    entries.push((entry, feature.descriptor));
                    owners.push(id);
                }
            }
            let r = crate::gradient_index::gradient_file_pair_report(
                &entries,
                &owners,
                config.recipe,
                config.search.matching.max_squared_distance,
                config.max_retrieval_comparisons,
                config.budgets.max_hits,
                config.budgets.max_pairs.min(config.budgets.max_pair_counts),
                budget,
                cancel,
            )
            .map_err(|e| mapped(&e))?;
            Ok(crate::local_index::DescriptorFilePairs {
                pairs: r.pairs,
                indexed_features: count,
                descriptor_hits: r.descriptor_hits,
                insufficient_features: insufficient,
            })
        },
    )
}

fn append_portfolio_batch<T: Copy>(
    values: &mut Vec<T>,
    retained: &mut rrrah_core::Reservation,
    batch: impl ExactSizeIterator<Item = T>,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<(), LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let length = values.len().checked_add(batch.len()).ok_or(LocalError::Budget)?;
    let bytes = length
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let mut credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
    let mut next = Vec::new();
    next.try_reserve_exact(length).map_err(|_| LocalError::Budget)?;
    let actual = next
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    credit.ensure_bytes(actual).map_err(|_| LocalError::Budget)?;
    for value in values.iter().copied().chain(batch) {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        next.push(value);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let old = std::mem::replace(values, next);
    let old_credit = std::mem::replace(retained, credit);
    drop(old);
    drop(old_credit);
    Ok(())
}

#[cfg(test)]
// Both allocations remain admitted until the old buffer has been dropped.
fn compact_portfolio<T: Copy>(
    values: Vec<T>,
    retained: rrrah_core::Reservation,
    budget: &MemoryBudget,
    cancel: &mut impl FnMut() -> bool,
) -> Result<rrrah_core::SharedBuffer<T>, LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let bytes = values
        .len()
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let mut credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
    let mut compact = Vec::new();
    compact
        .try_reserve_exact(values.len())
        .map_err(|_| LocalError::Budget)?;
    let actual = compact
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    credit.ensure_bytes(actual).map_err(|_| LocalError::Budget)?;
    for value in &values {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        compact.push(*value);
    }
    drop(values);
    drop(retained);
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    credit.try_adopt(compact).map_err(|_| LocalError::Budget)
}

#[cfg(test)]
mod compact_portfolio_tests {
    use super::*;
    #[test]
    fn incremental_batches_admit_overlap_and_preserve_previous_batch_on_failure() {
        use std::cell::Cell;
        for cancel_at in 0..=5 {
            let budget = MemoryBudget::new(56);
            let mut values = Vec::with_capacity(4);
            values.extend([u64::MAX, 0]);
            let mut credit = budget.try_reserve(32).unwrap();
            let calls = Cell::new(0);
            let result = append_portfolio_batch(&mut values, &mut credit, [7].into_iter(), &budget, &|| {
                calls.set(calls.get() + 1);
                cancel_at == calls.get()
            });
            if cancel_at == 0 {
                result.unwrap();
                assert_eq!(values, [u64::MAX, 0, 7]);
                assert_eq!(budget.used(), 24);
                assert_eq!(budget.peak(), 56);
            } else {
                assert!(matches!(result, Err(LocalError::Cancelled)));
                assert_eq!(values, [u64::MAX, 0]);
                assert_eq!(budget.used(), 32);
            }
            drop(values);
            drop(credit);
            assert_eq!(budget.used(), 0);
        }
        let budget = MemoryBudget::new(55);
        let mut values = Vec::with_capacity(4);
        values.extend([u64::MAX, 0]);
        let mut credit = budget.try_reserve(32).unwrap();
        assert!(matches!(
            append_portfolio_batch(&mut values, &mut credit, [7].into_iter(), &budget, &|| false),
            Err(LocalError::Budget)
        ));
        assert_eq!(values, [u64::MAX, 0]);
        assert_eq!(budget.used(), 32);
        drop(values);
        drop(credit);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn compaction_admits_overlap_preserves_order_and_rolls_back() {
        fn run(
            limit: u64,
            cancel_at: usize,
        ) -> (
            Result<rrrah_core::SharedBuffer<u64>, LocalError>,
            MemoryBudget,
            usize,
        ) {
            let budget = MemoryBudget::new(limit);
            let mut values = Vec::new();
            values.try_reserve_exact(4).unwrap();
            values.extend([u64::MAX, 0]);
            assert_eq!(values.capacity(), 4);
            let credit = budget.try_reserve(32).unwrap();
            let mut calls = 0;
            let result = compact_portfolio(values, credit, &budget, &mut || {
                calls += 1;
                calls == cancel_at
            });
            (result, budget, calls)
        }
        let (result, budget, calls) = run(48, usize::MAX);
        let buffer = result.unwrap();
        assert_eq!(&*buffer, &[u64::MAX, 0]);
        assert_eq!(budget.peak(), 48);
        assert_eq!(budget.used(), 16);
        let clone = buffer.clone();
        drop(buffer);
        assert_eq!(budget.used(), 16);
        drop(clone);
        assert_eq!(budget.used(), 0);
        let (result, budget, _) = run(47, usize::MAX);
        assert!(matches!(result, Err(LocalError::Budget)));
        assert_eq!(budget.used(), 0);
        for at in 1..=calls {
            let (result, budget, _) = run(48, at);
            assert!(matches!(result, Err(LocalError::Cancelled)));
            assert_eq!(budget.used(), 0);
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PortfolioFeature {
    Binary(Feature),
    Fixed(crate::gradient::GradientFeature),
    Interpolated(crate::gradient::GradientFeature),
}
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveFiveSearchCollectionPolicy {
    pub search: crate::local_scan::ProjectiveComplementaryGradientPortfolioPolicy,
    pub budgets: FileFeatureBudgets,
    /// Combined conservative N squared admission for both gradient indices.
    pub max_gradient_retrieval_comparisons: u64,
}
/// Union proposals from registration/binary pyramids and both explicit gradient recipes.
/// All retrieved pairs execute the complete five-search file API. Batch source and
/// cancellation guards apply to every family; proposals never establish equality.
pub fn scan_projective_local_collection_five_searches(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveFiveSearchCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    LocalCollectionReport<crate::local_scan::ProjectiveComplementaryGradientPortfolioEvidence>,
    ScanError,
> {
    scan_five_searches_with_confirmation(files, config, budget, cancel, |left, right, cancel| {
        crate::local_scan::compare_local_files_projective_complementary_gradient_portfolio(
            left,
            right,
            config.search,
            budget,
            cancel,
        )
    })
}
fn scan_five_searches_with_confirmation<E>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveFiveSearchCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    compare: impl Fn(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<E, LocalFileError>,
) -> Result<LocalCollectionReport<E>, ScanError> {
    scan_five_searches_with_spatial_confirmation(files, config, None, budget, cancel, compare)
}

/// Explicit sixth lane: spatial interpolated gradients supplement all five legacy searches.
/// The interpolated retrieval index includes both selections and all cross-selection hits.
/// Extra proposals still require fresh independent confirmation; no default is changed.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveSixSearchCollectionPolicy {
    pub five: ProjectiveFiveSearchCollectionPolicy,
    pub spatial: SpatialFeaturePolicy,
    pub max_total_comparisons: u64,
    pub max_total_hypotheses: u64,
    pub max_total_sample_pairs: u64,
}

#[derive(Debug)]
pub struct ProjectiveSixSearchEvidence {
    pub base: crate::local_scan::ProjectiveComplementaryGradientPortfolioEvidence,
    pub spatial: crate::local_scan::ProjectivePhotometricFileEvidence,
}

/// Retrieve the union and confirm every pair in all six lanes under one batch guard.
/// Whole-image decisions stay independent; this API does not confirm local regions.
///
/// # Errors
/// Invalid selection/policy, cumulative limits, cancellation or source/decode failures.
pub fn scan_projective_local_collection_six_searches(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveSixSearchCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<ProjectiveSixSearchEvidence>, ScanError> {
    validate_six_search_collection(config)?;
    let five = config.five.search;
    let extra = five.interpolated;
    scan_five_searches_with_spatial_confirmation(
        files,
        config.five,
        Some(config.spatial),
        budget,
        cancel,
        |left, right, cancel| {
            let base = crate::local_scan::compare_local_files_projective_complementary_gradient_portfolio(
                left, right, five, budget, cancel,
            )?;
            let spatial =
                crate::local_scan::compare_local_files_projective_spatial_gradient_pyramid_with_recipe(
                    left,
                    right,
                    extra,
                    crate::gradient::GradientCellRecipe::Interpolated,
                    config.spatial,
                    budget,
                    cancel,
                )?;
            Ok(ProjectiveSixSearchEvidence { base, spatial })
        },
    )
}

fn validate_six_search_collection(config: ProjectiveSixSearchCollectionPolicy) -> Result<(), ScanError> {
    let five = config.five.search;
    let extra = five.interpolated;
    crate::local_scan::validate_projective_complementary_gradient_portfolio_policy(&five).map_err(
        |e| match e {
            LocalFileError::InvalidPolicy | LocalFileError::Features(LocalError::Invalid) => {
                ScanError::InvalidPolicy
            }
            _ => ScanError::Budget,
        },
    )?;
    validate_spatial(config.spatial, extra.search.local.extract.max_features).map_err(|e| match e {
        LocalFileError::InvalidPolicy => ScanError::InvalidPolicy,
        _ => ScanError::Budget,
    })?;
    let comparisons = five
        .max_total_comparisons
        .checked_add(extra.matching.max_comparisons)
        .ok_or(ScanError::Budget)?;
    let hypotheses = five
        .max_total_hypotheses
        .checked_add(extra.search.sampling.trials)
        .ok_or(ScanError::Budget)?;
    let samples = five
        .max_total_sample_pairs
        .checked_add(extra.search.filter.filter.max_sample_pairs)
        .ok_or(ScanError::Budget)?;
    if comparisons > config.max_total_comparisons
        || hypotheses > config.max_total_hypotheses
        || samples > config.max_total_sample_pairs
    {
        return Err(ScanError::Budget);
    }
    Ok(())
}

/// All six whole searches plus binary and three independent gradient-model grids.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveSixRegionCollectionPolicy {
    pub six: ProjectiveSixSearchCollectionPolicy,
    pub grid: (u32, u32),
    pub max_regions: usize,
    pub max_total_comparisons: u64,
    pub max_total_hypotheses: u64,
    pub max_total_sample_pairs: u64,
}
#[derive(Debug)]
pub struct ProjectiveSixRegionEvidence {
    pub whole: ProjectiveSixSearchEvidence,
    pub binary_regions: crate::local_scan::ProjectivePyramidRegionsFileEvidence,
    /// Fixed, interpolated and spatial-interpolated models, each with both-image grids.
    pub gradient_regions: [Option<crate::local_scan::ProjectiveTransformedRegionsFileEvidence>; 3],
}

/// Confirm the complete retrieved union within one source/cancellation lifecycle.
/// Independent local supports never promote any whole-image decision.
/// All four regional phases are cumulatively admitted even when a model is absent.
///
/// # Errors
/// Invalid policy/grid, cumulative resource refusal, cancellation or source errors.
pub fn scan_projective_local_collection_six_regions(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveSixRegionCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<ProjectiveSixRegionEvidence>, ScanError> {
    validate_six_search_collection(config.six)?;
    if config.grid.0 == 0 || config.grid.1 == 0 || config.max_regions == 0 {
        return Err(ScanError::InvalidPolicy);
    }
    let cells = u64::from(config.grid.0)
        .checked_mul(u64::from(config.grid.1))
        .ok_or(ScanError::Budget)?;
    if cells.checked_mul(2).ok_or(ScanError::Budget)?
        > u64::try_from(config.max_regions).map_err(|_| ScanError::Budget)?
    {
        return Err(ScanError::Budget);
    }
    let five = config.six.five.search;
    let mut binary = five.primary.base.searches.pyramid;
    binary.filter = five.primary.base.secondary;
    let searches = [
        binary,
        five.primary.gradient.search,
        five.interpolated.search,
        five.interpolated.search,
    ];
    let mut policies = [crate::local_scan::ProjectivePyramidRegionsFilePolicy {
        search: binary,
        max_regions: config.max_regions,
        max_total_sample_pairs: 0,
    }; 4];
    let mut samples = config.six.max_total_sample_pairs;
    for (index, search) in searches.into_iter().enumerate() {
        let count = cells
            .checked_mul(if index == 0 { 1 } else { 2 })
            .ok_or(ScanError::Budget)?;
        let work = count
            .checked_add(1)
            .and_then(|n| n.checked_mul(search.filter.filter.max_sample_pairs))
            .ok_or(ScanError::Budget)?;
        samples = samples.checked_add(work).ok_or(ScanError::Budget)?;
        policies[index] = crate::local_scan::ProjectivePyramidRegionsFilePolicy {
            search,
            max_regions: config.max_regions,
            max_total_sample_pairs: work,
        };
    }
    let comparisons = config
        .six
        .max_total_comparisons
        .checked_add(binary.local.matching.max_comparisons)
        .ok_or(ScanError::Budget)?;
    let hypotheses = config
        .six
        .max_total_hypotheses
        .checked_add(binary.sampling.trials)
        .ok_or(ScanError::Budget)?;
    if comparisons > config.max_total_comparisons
        || hypotheses > config.max_total_hypotheses
        || samples > config.max_total_sample_pairs
    {
        return Err(ScanError::Budget);
    }
    scan_five_searches_with_spatial_confirmation(
        files,
        config.six.five,
        Some(config.six.spatial),
        budget,
        cancel,
        |left, right, cancel| {
            let base = crate::local_scan::compare_local_files_projective_complementary_gradient_portfolio(
                left, right, five, budget, cancel,
            )?;
            let spatial =
                crate::local_scan::compare_local_files_projective_spatial_gradient_pyramid_with_recipe(
                    left,
                    right,
                    five.interpolated,
                    crate::gradient::GradientCellRecipe::Interpolated,
                    config.six.spatial,
                    budget,
                    cancel,
                )?;
            let binary_regions = crate::local_scan::compare_local_files_projective_pyramid_region_grid(
                left,
                right,
                policies[0],
                config.grid,
                budget,
                cancel,
            )?;
            let mut gradient_regions = [None, None, None];
            for (index, lane) in [&base.primary.gradient, &base.interpolated, &spatial]
                .into_iter()
                .enumerate()
            {
                if let Some(model) = lane
                    .registered_transform
                    .or_else(|| lane.geometry.as_ref().map(|e| e.transform))
                {
                    gradient_regions[index]=Some(crate::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(left,right,model,policies[index+1],config.grid,budget,cancel)?);
                }
            }
            Ok(ProjectiveSixRegionEvidence {
                whole: ProjectiveSixSearchEvidence { base, spatial },
                binary_regions,
                gradient_regions,
            })
        },
    )
}

fn scan_five_searches_with_spatial_confirmation<E>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveFiveSearchCollectionPolicy,
    spatial: Option<SpatialFeaturePolicy>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    compare: impl Fn(&DecodeRequest, &DecodeRequest, &dyn Fn() -> bool) -> Result<E, LocalFileError>,
) -> Result<LocalCollectionReport<E>, ScanError> {
    crate::local_scan::validate_projective_complementary_gradient_portfolio_policy(&config.search).map_err(
        |e| match e {
            LocalFileError::InvalidPolicy | LocalFileError::Features(LocalError::Invalid) => {
                ScanError::InvalidPolicy
            }
            _ => ScanError::Budget,
        },
    )?;
    let c = config.search;
    let binary = c.primary.base.searches;
    let p = binary.pyramid;
    let r = binary.registration;
    if let Some(grid) = spatial {
        validate_spatial(grid, c.interpolated.search.local.extract.max_features).map_err(|e| match e {
            LocalFileError::InvalidPolicy => ScanError::InvalidPolicy,
            _ => ScanError::Budget,
        })?;
    }
    let mut proposal = p.local;
    proposal.matching.max_distance = proposal.matching.max_distance.max(r.local.matching.max_distance);
    proposal.geometry.min_inliers = proposal.geometry.min_inliers.min(r.local.geometry.min_inliers);
    let policy = LocalCollectionPolicy {
        search: proposal.into(),
        budgets: config.budgets,
    };
    let pyramid =
        |p: crate::local_scan::ProjectivePyramidPhotometricFilePolicy| crate::pyramid::PyramidPolicy {
            local: p.local.extract,
            max_levels: p.max_levels,
            max_total_pixels: p.max_total_pixels,
            max_total_features: p.max_total_features,
        };
    let capacity = r
        .local
        .extract
        .max_features
        .checked_add(p.max_total_features)
        .and_then(|n| n.checked_add(c.primary.gradient.search.max_total_features))
        .and_then(|n| n.checked_add(c.interpolated.search.max_total_features))
        .and_then(|n| {
            n.checked_add(if spatial.is_some() {
                c.interpolated.search.max_total_features
            } else {
                0
            })
        })
        .ok_or(ScanError::Budget)?;
    let bytes = capacity
        .checked_mul(std::mem::size_of::<PortfolioFeature>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(ScanError::Budget)?;
    scan_collection_with_typed_extractor(
        files,
        &policy,
        Some(budget),
        cancel,
        |request, source, cancel| {
            extract_file_with(request, source, policy.search, budget, cancel, |view, cancel| {
                let mut retained = budget
                    .try_reserve(if spatial.is_some() { 0 } else { bytes })
                    .map_err(|_| LocalError::Budget)?;
                let mut result = Vec::new();
                if spatial.is_none() {
                    result
                        .try_reserve_exact(capacity)
                        .map_err(|_| LocalError::Budget)?;
                }
                let ordinary = extract_search_features(view, r.local.extract, r.spatial, budget, cancel)?;
                if spatial.is_some() {
                    append_portfolio_batch(
                        &mut result,
                        &mut retained,
                        ordinary.iter().copied().map(PortfolioFeature::Binary),
                        budget,
                        cancel,
                    )?;
                } else {
                    for feature in ordinary.iter() {
                        if cancel() {
                            return Err(LocalError::Cancelled);
                        }
                        result.push(PortfolioFeature::Binary(*feature));
                    }
                }
                drop(ordinary);
                let scaled =
                    crate::pyramid::extract_oriented_pyramid_managed(view, pyramid(p), budget, cancel)?;
                if spatial.is_some() {
                    append_portfolio_batch(
                        &mut result,
                        &mut retained,
                        scaled.iter().copied().map(PortfolioFeature::Binary),
                        budget,
                        cancel,
                    )?;
                } else {
                    for feature in scaled.iter() {
                        if cancel() {
                            return Err(LocalError::Cancelled);
                        }
                        result.push(PortfolioFeature::Binary(*feature));
                    }
                }
                drop(scaled);
                for (g, recipe) in [
                    (c.primary.gradient, crate::gradient::GradientCellRecipe::Fixed),
                    (c.interpolated, crate::gradient::GradientCellRecipe::Interpolated),
                ] {
                    let gradients = crate::gradient::extract_gradient_pyramid_with_recipe_managed(
                        view,
                        pyramid(g.search),
                        g.max_total_gradient_samples,
                        recipe,
                        budget,
                        cancel,
                    )?;
                    if spatial.is_some() {
                        append_portfolio_batch(
                            &mut result,
                            &mut retained,
                            gradients.iter().copied().map(|feature| {
                                if recipe == crate::gradient::GradientCellRecipe::Fixed {
                                    PortfolioFeature::Fixed(feature)
                                } else {
                                    PortfolioFeature::Interpolated(feature)
                                }
                            }),
                            budget,
                            cancel,
                        )?;
                    } else {
                        for feature in gradients.iter() {
                            if cancel() {
                                return Err(LocalError::Cancelled);
                            }
                            result.push(if recipe == crate::gradient::GradientCellRecipe::Fixed {
                                PortfolioFeature::Fixed(*feature)
                            } else {
                                PortfolioFeature::Interpolated(*feature)
                            });
                        }
                    }
                }
                if let Some(grid) = spatial {
                    let g = c.interpolated;
                    let gradients = crate::gradient::extract_spatial_gradient_pyramid_with_recipe_managed(
                        view,
                        pyramid(g.search),
                        g.max_total_gradient_samples,
                        crate::gradient::GradientCellRecipe::Interpolated,
                        (grid.columns, grid.rows, grid.max_per_cell),
                        budget,
                        cancel,
                    )?;
                    append_portfolio_batch(
                        &mut result,
                        &mut retained,
                        gradients.iter().copied().map(PortfolioFeature::Interpolated),
                        budget,
                        cancel,
                    )?;
                }
                if result.len() > capacity {
                    return Err(LocalError::Budget);
                }
                retained.try_adopt(result).map_err(|_| LocalError::Budget)
            })
        },
        |_, _, _| Ok(None),
        compare,
        |features, cancel| {
            let id_bytes = features
                .len()
                .checked_mul(std::mem::size_of::<u64>())
                .and_then(|n| u64::try_from(n).ok())
                .ok_or(ScanError::Budget)?;
            let id_credit = budget.try_reserve(id_bytes).map_err(|_| ScanError::Budget)?;
            let mut ids = Vec::new();
            ids.try_reserve_exact(features.len())
                .map_err(|_| ScanError::Budget)?;
            ids.extend(features.keys().copied());
            ids.sort_unstable();
            let ids = id_credit.try_adopt(ids).map_err(|_| ScanError::Budget)?;
            let metadata_bytes = |count: usize, item_bytes: usize| {
                count
                    .checked_mul(item_bytes)
                    .and_then(|n| u64::try_from(n).ok())
                    .ok_or(ScanError::Budget)
            };
            let file_item_bytes = std::mem::size_of::<(u64, rrrah_core::SharedBuffer<Feature>)>();
            let mut file_list_credit = budget
                .try_reserve(metadata_bytes(ids.len(), file_item_bytes)?)
                .map_err(|_| ScanError::Budget)?;
            let mut insufficient_credit = budget
                .try_reserve(metadata_bytes(ids.len(), std::mem::size_of::<u64>())?)
                .map_err(|_| ScanError::Budget)?;
            let mut binary_files = Vec::new();
            let mut insufficient = Vec::new();
            let mut indexed = 0_usize;
            binary_files
                .try_reserve_exact(ids.len())
                .map_err(|_| ScanError::Budget)?;
            insufficient
                .try_reserve_exact(ids.len())
                .map_err(|_| ScanError::Budget)?;
            file_list_credit
                .ensure_bytes(metadata_bytes(binary_files.capacity(), file_item_bytes)?)
                .map_err(|_| ScanError::Budget)?;
            insufficient_credit
                .ensure_bytes(metadata_bytes(
                    insufficient.capacity(),
                    std::mem::size_of::<u64>(),
                )?)
                .map_err(|_| ScanError::Budget)?;
            for &id in ids.iter() {
                let binary_count = features[&id]
                    .iter()
                    .filter(|feature| matches!(feature, PortfolioFeature::Binary(_)))
                    .count();
                let binary_bytes = binary_count
                    .checked_mul(std::mem::size_of::<Feature>())
                    .and_then(|n| u64::try_from(n).ok())
                    .ok_or(ScanError::Budget)?;
                let binary_credit = budget.try_reserve(binary_bytes).map_err(|_| ScanError::Budget)?;
                let mut values = Vec::new();
                values
                    .try_reserve_exact(binary_count)
                    .map_err(|_| ScanError::Budget)?;
                let mut count = [0_usize; 3];
                for feature in features[&id].iter() {
                    match feature {
                        PortfolioFeature::Binary(v) => {
                            values.try_reserve_exact(1).map_err(|_| ScanError::Budget)?;
                            values.push(*v);
                            count[0] += 1;
                        }
                        PortfolioFeature::Fixed(_) => count[1] += 1,
                        PortfolioFeature::Interpolated(_) => count[2] += 1,
                    }
                }
                if count[0] < policy.search.local.geometry.min_inliers
                    && count[1] < c.primary.gradient.search.local.geometry.min_inliers
                    && count[2] < c.interpolated.search.local.geometry.min_inliers
                {
                    insufficient.try_reserve_exact(1).map_err(|_| ScanError::Budget)?;
                    insufficient.push(id);
                }
                indexed = indexed
                    .checked_add(features[&id].len())
                    .ok_or(ScanError::Budget)?;
                binary_files.try_reserve_exact(1).map_err(|_| ScanError::Budget)?;
                binary_files.push((
                    id,
                    binary_credit.try_adopt(values).map_err(|_| ScanError::Budget)?,
                ));
            }
            let binary = crate::local_index::descriptor_file_pairs_shared_managed(
                binary_files,
                policy.search.local.matching.max_distance,
                policy.search.local.geometry.min_inliers,
                config.budgets,
                budget,
                cancel,
            )
            .map_err(|e| mapped(&e))?;
            let mut hits = binary.descriptor_hits;
            let mut pairs = binary.pairs;
            let mut work = 0_u64;
            // Keep the union sorted and unique while growing, rather than allocating
            // space for duplicate proposals from every lane before deduplication.
            let pair_bytes = |count: usize| {
                count
                    .checked_mul(std::mem::size_of::<(u64, u64)>())
                    .and_then(|n| u64::try_from(n).ok())
                    .ok_or(ScanError::Budget)
            };
            let mut union_credit = budget
                .try_reserve(pair_bytes(pairs.capacity())?)
                .map_err(|_| ScanError::Budget)?;
            pairs.sort_unstable();
            pairs.dedup();
            for (recipe, g) in [
                (crate::gradient::GradientCellRecipe::Fixed, c.primary.gradient),
                (crate::gradient::GradientCellRecipe::Interpolated, c.interpolated),
            ] {
                // Reserve the full flattened recipe before allocating or copying descriptors.
                let count = features
                    .values()
                    .map(|values| {
                        values
                            .iter()
                            .filter(|feature| {
                                matches!(
                                    (feature, recipe),
                                    (
                                        PortfolioFeature::Fixed(_),
                                        crate::gradient::GradientCellRecipe::Fixed
                                    ) | (
                                        PortfolioFeature::Interpolated(_),
                                        crate::gradient::GradientCellRecipe::Interpolated
                                    )
                                )
                            })
                            .count()
                    })
                    .try_fold(0_usize, |sum, n| sum.checked_add(n))
                    .ok_or(ScanError::Budget)?;
                let entry_bytes = count
                    .checked_mul(std::mem::size_of::<(u64, crate::gradient::GradientDescriptor)>())
                    .and_then(|n| u64::try_from(n).ok())
                    .ok_or(ScanError::Budget)?;
                let owner_bytes = count
                    .checked_mul(std::mem::size_of::<u64>())
                    .and_then(|n| u64::try_from(n).ok())
                    .ok_or(ScanError::Budget)?;
                let entry_credit = budget.try_reserve(entry_bytes).map_err(|_| ScanError::Budget)?;
                let owner_credit = budget.try_reserve(owner_bytes).map_err(|_| ScanError::Budget)?;
                let mut entries = Vec::new();
                let mut owners = Vec::new();
                entries.try_reserve_exact(count).map_err(|_| ScanError::Budget)?;
                owners.try_reserve_exact(count).map_err(|_| ScanError::Budget)?;
                for &id in ids.iter() {
                    for feature in features[&id].iter() {
                        if cancel() {
                            return Err(ScanError::Cancelled);
                        }
                        let value = match (feature, recipe) {
                            (PortfolioFeature::Fixed(v), crate::gradient::GradientCellRecipe::Fixed)
                            | (
                                PortfolioFeature::Interpolated(v),
                                crate::gradient::GradientCellRecipe::Interpolated,
                            ) => Some(v),
                            _ => None,
                        };
                        if let Some(v) = value {
                            entries.try_reserve_exact(1).map_err(|_| ScanError::Budget)?;
                            owners.try_reserve_exact(1).map_err(|_| ScanError::Budget)?;
                            entries.push((
                                u64::try_from(entries.len()).map_err(|_| ScanError::Budget)?,
                                v.descriptor,
                            ));
                            owners.push(id);
                        }
                    }
                }
                let entries = entry_credit.try_adopt(entries).map_err(|_| ScanError::Budget)?;
                let owners = owner_credit.try_adopt(owners).map_err(|_| ScanError::Budget)?;
                let n = u64::try_from(entries.len()).map_err(|_| ScanError::Budget)?;
                work = work
                    .checked_add(n.checked_mul(n).ok_or(ScanError::Budget)?)
                    .ok_or(ScanError::Budget)?;
                if work > config.max_gradient_retrieval_comparisons {
                    return Err(ScanError::Budget);
                }
                let retrieved = crate::gradient_index::gradient_file_pair_report(
                    &entries,
                    &owners,
                    recipe,
                    g.matching.max_squared_distance,
                    n * n,
                    config
                        .budgets
                        .max_hits
                        .checked_sub(hits)
                        .ok_or(ScanError::Budget)?,
                    config.budgets.max_pairs.min(config.budgets.max_pair_counts),
                    budget,
                    cancel,
                )
                .map_err(|e| mapped(&e))?;
                hits = hits
                    .checked_add(retrieved.descriptor_hits)
                    .ok_or(ScanError::Budget)?;
                // Each gradient report is already sorted/unique. Compare against
                // the immutable old prefix, then append only new pairs and sort.
                // This avoids both duplicate allocation and quadratic insertion.
                let prefix = pairs.len();
                let mut added = 0_usize;
                for pair in &retrieved.pairs {
                    if cancel() {
                        return Err(ScanError::Cancelled);
                    }
                    if pairs[..prefix].binary_search(pair).is_err() {
                        added = added.checked_add(1).ok_or(ScanError::Budget)?;
                    }
                }
                let next = prefix.checked_add(added).ok_or(ScanError::Budget)?;
                if next > config.budgets.max_pairs {
                    return Err(ScanError::Budget);
                }
                union_credit
                    .ensure_bytes(pair_bytes(pairs.capacity().max(next))?)
                    .map_err(|_| ScanError::Budget)?;
                pairs.try_reserve_exact(added).map_err(|_| ScanError::Budget)?;
                union_credit
                    .ensure_bytes(pair_bytes(pairs.capacity())?)
                    .map_err(|_| ScanError::Budget)?;
                for &pair in &retrieved.pairs {
                    if cancel() {
                        return Err(ScanError::Cancelled);
                    }
                    if pairs[..prefix].binary_search(&pair).is_err() {
                        pairs.push(pair);
                    }
                }
                pairs.sort_unstable();
            }
            Ok(crate::local_index::DescriptorFilePairs {
                pairs,
                indexed_features: indexed,
                descriptor_hits: hits,
                insufficient_features: insufficient,
            })
        },
    )
}

/// Explicit indexed regional search; whole-image and local-region evidence stay separate.
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveRegionGridCollectionPolicy {
    pub search: crate::local_scan::ProjectivePyramidRegionsFilePolicy,
    pub grid: (u32, u32),
    pub budgets: FileFeatureBudgets,
}
/// Retrieve pyramid descriptor proposals and verify automatic regions through
/// fresh native file comparisons. The common batch lifecycle removes all edges
/// involving a changed source, including locally accepted regions.
///
/// # Errors
/// Invalid policy/grid, aggregate resources or cancellation. Per-file/pair
/// errors are attributed; regional matches never become whole-image equality.
pub fn scan_projective_local_collection_region_grid(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveRegionGridCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<crate::local_scan::ProjectivePyramidRegionsFileEvidence>, ScanError> {
    let p = config.search.search;
    validate_projective_pyramid_collection(&ProjectivePyramidCollectionPolicy {
        search: p,
        budgets: config.budgets,
    })?;
    if config.grid.0 == 0 || config.grid.1 == 0 || config.search.max_regions == 0 {
        return Err(ScanError::InvalidPolicy);
    }
    let cells = u64::from(config.grid.0)
        .checked_mul(u64::from(config.grid.1))
        .ok_or(ScanError::Budget)?;
    if cells > u64::try_from(config.search.max_regions).map_err(|_| ScanError::Budget)? {
        return Err(ScanError::Budget);
    }
    let work = cells
        .checked_add(1)
        .and_then(|n| n.checked_mul(p.filter.filter.max_sample_pairs))
        .ok_or(ScanError::Budget)?;
    if work > config.search.max_total_sample_pairs {
        return Err(ScanError::Budget);
    }
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
        |_, _, _| Ok(None),
        |left, right, cancel| {
            crate::local_scan::compare_local_files_projective_pyramid_region_grid(
                left,
                right,
                config.search,
                config.grid,
                budget,
                cancel,
            )
        },
    )
}

/// One indexed union with independently retained whole and regional evidence.
#[derive(Debug)]
pub struct ProjectiveFiveRegionEvidence {
    pub whole: crate::local_scan::ProjectiveComplementaryGradientPortfolioEvidence,
    pub regional: crate::local_scan::ProjectivePyramidRegionsFileEvidence,
}
#[derive(Debug, Clone, Copy)]
pub struct ProjectiveFiveRegionCollectionPolicy {
    pub five: ProjectiveFiveSearchCollectionPolicy,
    pub grid: (u32, u32),
    pub max_regions: usize,
    pub max_total_comparisons: u64,
    pub max_total_hypotheses: u64,
    pub max_total_sample_pairs: u64,
}
/// Confirm every five-family indexed proposal with all whole-image searches
/// and automatic radius-3 regions using the same binary pyramid recipe.
/// Both phases must finish; batch source guards discard changed-source edges.
/// A locally confirmed region never promotes the whole-image candidate.
///
/// # Errors
/// Invalid grid/policy, cumulative phase admission, collection resources or
/// cancellation; source/pair refusals retain their ordinary attribution.
pub fn scan_projective_local_collection_five_regions(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveFiveRegionCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<ProjectiveFiveRegionEvidence>, ScanError> {
    let five = config.five.search;
    let mut search = five.primary.base.searches.pyramid;
    search.filter = five.primary.base.secondary;
    if config.grid.0 == 0 || config.grid.1 == 0 || config.max_regions == 0 {
        return Err(ScanError::InvalidPolicy);
    }
    let cells = u64::from(config.grid.0)
        .checked_mul(u64::from(config.grid.1))
        .ok_or(ScanError::Budget)?;
    if cells > u64::try_from(config.max_regions).map_err(|_| ScanError::Budget)? {
        return Err(ScanError::Budget);
    }
    let regional_work = cells
        .checked_add(1)
        .and_then(|n| n.checked_mul(search.filter.filter.max_sample_pairs))
        .ok_or(ScanError::Budget)?;
    let comparisons = five
        .max_total_comparisons
        .checked_add(search.local.matching.max_comparisons)
        .ok_or(ScanError::Budget)?;
    let hypotheses = five
        .max_total_hypotheses
        .checked_add(search.sampling.trials)
        .ok_or(ScanError::Budget)?;
    let samples = five
        .max_total_sample_pairs
        .checked_add(regional_work)
        .ok_or(ScanError::Budget)?;
    if comparisons > config.max_total_comparisons
        || hypotheses > config.max_total_hypotheses
        || samples > config.max_total_sample_pairs
    {
        return Err(ScanError::Budget);
    }
    crate::local_scan::validate_projective_pyramid_file_policy(&search).map_err(|e| match e {
        LocalFileError::InvalidPolicy => ScanError::InvalidPolicy,
        _ => ScanError::Budget,
    })?;
    let regional_policy = crate::local_scan::ProjectivePyramidRegionsFilePolicy {
        search,
        max_regions: config.max_regions,
        max_total_sample_pairs: regional_work,
    };
    scan_five_searches_with_confirmation(files, config.five, budget, cancel, |left, right, cancel| {
        let whole = crate::local_scan::compare_local_files_projective_complementary_gradient_portfolio(
            left, right, five, budget, cancel,
        )?;
        let regional = crate::local_scan::compare_local_files_projective_pyramid_region_grid(
            left,
            right,
            regional_policy,
            config.grid,
            budget,
            cancel,
        )?;
        Ok(ProjectiveFiveRegionEvidence { whole, regional })
    })
}

/// Regional confirmations from binary, fixed-gradient and interpolated-gradient models.
/// Each lane retains its own domains and pixel evidence; supports are not union area.
#[derive(Debug)]
pub struct ProjectiveFiveAllRegionsEvidence {
    pub base: ProjectiveFiveRegionEvidence,
    pub gradient_regions: [Option<crate::local_scan::ProjectiveTransformedRegionsFileEvidence>; 2],
}

/// Confirm five-family proposals with regional pixels under all available gradient models.
/// The existing whole-image candidate remains independent of every local confirmation.
/// Cumulative admission includes all three regional pixel phases even if a model is absent.
/// Fresh file guards and the common batch guard cover all phases atomically.
///
/// # Errors
/// Invalid policy, cumulative work overflow/refusal, source/decode or cancellation.
pub fn scan_projective_local_collection_five_all_regions(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveFiveRegionCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<ProjectiveFiveAllRegionsEvidence>, ScanError> {
    scan_five_all_regions_with_direction(files, config, false, budget, cancel)
}

/// Five-family collection search with both-image grids for each gradient model.
/// The binary grid and whole-image decisions retain their original policies.
/// Both full gradient grids are admitted before source access, even when a model
/// is absent. Overlapping regional supports remain separate from whole copies.
///
/// # Errors
/// Invalid policy, cumulative limits, source/decode failures or cancellation.
pub fn scan_projective_local_collection_five_bidirectional_gradient_regions(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveFiveRegionCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<ProjectiveFiveAllRegionsEvidence>, ScanError> {
    scan_five_all_regions_with_direction(files, config, true, budget, cancel)
}

fn scan_five_all_regions_with_direction(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveFiveRegionCollectionPolicy,
    bidirectional: bool,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalCollectionReport<ProjectiveFiveAllRegionsEvidence>, ScanError> {
    let five = config.five.search;
    if config.grid.0 == 0 || config.grid.1 == 0 || config.max_regions == 0 {
        return Err(ScanError::InvalidPolicy);
    }
    let cells = u64::from(config.grid.0)
        .checked_mul(u64::from(config.grid.1))
        .ok_or(ScanError::Budget)?;
    let maximum_cells = cells
        .checked_mul(if bidirectional { 2 } else { 1 })
        .ok_or(ScanError::Budget)?;
    if maximum_cells > u64::try_from(config.max_regions).map_err(|_| ScanError::Budget)? {
        return Err(ScanError::Budget);
    }
    let mut binary = five.primary.base.searches.pyramid;
    binary.filter = five.primary.base.secondary;
    let searches = [binary, five.primary.gradient.search, five.interpolated.search];
    let mut policies = [crate::local_scan::ProjectivePyramidRegionsFilePolicy {
        search: binary,
        max_regions: config.max_regions,
        max_total_sample_pairs: 0,
    }; 3];
    let mut samples = five.max_total_sample_pairs;
    for (index, search) in searches.into_iter().enumerate() {
        crate::local_scan::validate_projective_pyramid_file_policy(&search).map_err(|e| match e {
            LocalFileError::InvalidPolicy => ScanError::InvalidPolicy,
            _ => ScanError::Budget,
        })?;
        let lane_cells = cells
            .checked_mul(if bidirectional && index > 0 { 2 } else { 1 })
            .ok_or(ScanError::Budget)?;
        let work = lane_cells
            .checked_add(1)
            .and_then(|n| n.checked_mul(search.filter.filter.max_sample_pairs))
            .ok_or(ScanError::Budget)?;
        samples = samples.checked_add(work).ok_or(ScanError::Budget)?;
        policies[index] = crate::local_scan::ProjectivePyramidRegionsFilePolicy {
            search,
            max_regions: config.max_regions,
            max_total_sample_pairs: work,
        };
    }
    let comparisons = five
        .max_total_comparisons
        .checked_add(binary.local.matching.max_comparisons)
        .ok_or(ScanError::Budget)?;
    let hypotheses = five
        .max_total_hypotheses
        .checked_add(binary.sampling.trials)
        .ok_or(ScanError::Budget)?;
    if comparisons > config.max_total_comparisons
        || hypotheses > config.max_total_hypotheses
        || samples > config.max_total_sample_pairs
    {
        return Err(ScanError::Budget);
    }
    scan_five_searches_with_confirmation(files, config.five, budget, cancel, |left, right, cancel| {
        let whole = crate::local_scan::compare_local_files_projective_complementary_gradient_portfolio(
            left, right, five, budget, cancel,
        )?;
        let regional = crate::local_scan::compare_local_files_projective_pyramid_region_grid(
            left,
            right,
            policies[0],
            config.grid,
            budget,
            cancel,
        )?;
        let lanes = [&whole.primary.gradient, &whole.interpolated];
        let mut gradient_regions = [None, None];
        for (index, lane) in lanes.into_iter().enumerate() {
            let model = lane
                .registered_transform
                .or_else(|| lane.geometry.as_ref().map(|e| e.transform));
            if let Some(model) = model {
                gradient_regions[index] = Some(if bidirectional {
                    crate::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
                        left,
                        right,
                        model,
                        policies[index + 1],
                        config.grid,
                        budget,
                        cancel,
                    )?
                } else {
                    crate::local_scan::compare_local_files_projective_region_grid_transform(
                        left,
                        right,
                        model,
                        policies[index + 1],
                        config.grid,
                        budget,
                        cancel,
                    )?
                });
            }
        }
        Ok(ProjectiveFiveAllRegionsEvidence {
            base: ProjectiveFiveRegionEvidence { whole, regional },
            gradient_regions,
        })
    })
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectiveDistinctScaleRegionsCollectionPolicy {
    pub search: crate::local_scan::ProjectiveDistinctScaleRegionsFilePolicy,
    pub budgets: FileFeatureBudgets,
    pub max_retrieval_comparisons: u64,
}
/// Exact within-radius proposals with fresh managed whole/local file confirmation.
/// The common batch guard removes all affected edges after source changes.
/// Proposals never promote local support to whole-image acceptance.
/// # Errors
/// Invalid policy/scales before iteration, work/memory limits or cancellation;
/// source/confirmation issues remain per-file/per-pair diagnostics.
pub fn scan_projective_local_collection_spatial_gradient_scale_regions_distinct_managed(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveDistinctScaleRegionsCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    LocalCollectionReport<crate::local_scan::ManagedProjectiveDistinctScaleRegionsFileEvidence>,
    ScanError,
> {
    crate::local_scan::validate_distinct_scale_regions_file_policy(config.search, factors, &cancel).map_err(
        |e| match e {
            LocalFileError::Cancelled => ScanError::Cancelled,
            LocalFileError::Features(LocalError::Budget)
            | LocalFileError::Geometry(crate::geometry::GeometryError::Budget)
            | LocalFileError::Pixels(crate::warp::WarpError::Budget) => ScanError::Budget,
            _ => ScanError::InvalidPolicy,
        },
    )?;
    let gradient = ProjectiveGradientCollectionPolicy {
        search: config.search.gradient,
        recipe: config.search.recipe,
        budgets: config.budgets,
        max_retrieval_comparisons: config.max_retrieval_comparisons,
    };
    scan_gradient_collection_with_confirmation(
        files,
        gradient,
        Some(config.search.spatial),
        Some(factors),
        None,
        None,
        budget,
        cancel,
        |left, right, cancel| {
            crate::local_scan::compare_local_files_projective_spatial_gradient_scale_regions_distinct_managed(
                left,
                right,
                config.search,
                factors,
                budget,
                cancel,
            )
        },
    )
}

/// Area-sampled exact-radius proposals and fresh area-sampled file confirmation.
/// Source changes remove every affected edge using the shared batch lifecycle.
/// # Errors
/// Invalid policy/scales or zero work before iteration, resource/cancel refusals;
/// source/confirmation failures remain explicit file/pair diagnostics.
pub fn scan_projective_local_collection_spatial_gradient_scale_regions_area_distinct_managed(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveDistinctScaleRegionsCollectionPolicy,
    factors: &[f64],
    max_resample_taps: u64,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    LocalCollectionReport<crate::local_scan::ManagedProjectiveDistinctScaleRegionsFileEvidence>,
    ScanError,
> {
    crate::local_scan::validate_distinct_scale_regions_file_policy(config.search, factors, &cancel).map_err(
        |e| match e {
            LocalFileError::Cancelled => ScanError::Cancelled,
            LocalFileError::Features(LocalError::Budget)
            | LocalFileError::Geometry(crate::geometry::GeometryError::Budget)
            | LocalFileError::Pixels(crate::warp::WarpError::Budget) => ScanError::Budget,
            _ => ScanError::InvalidPolicy,
        },
    )?;
    if max_resample_taps == 0 && factors.iter().any(|&f| f > 1.) {
        return Err(ScanError::Budget);
    }
    let gradient = ProjectiveGradientCollectionPolicy {
        search: config.search.gradient,
        recipe: config.search.recipe,
        budgets: config.budgets,
        max_retrieval_comparisons: config.max_retrieval_comparisons,
    };
    scan_gradient_collection_with_confirmation(
        files,
        gradient,
        Some(config.search.spatial),
        Some(factors),
        Some(max_resample_taps),
        None,
        budget,
        cancel,
        |left, right, cancel| {
            crate::local_scan::compare_local_files_projective_spatial_gradient_scale_regions_area_distinct_managed(
            left,right,config.search,factors,max_resample_taps,budget,cancel,
        )
        },
    )
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectiveCandidateUnionRegionsCollectionPolicy {
    pub search: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    pub budgets: FileFeatureBudgets,
    pub max_retrieval_comparisons: u64,
}
/// Index both independent candidate recipes, then freshly confirm their matched
/// correspondence union. Cross-recipe descriptor hits are proposals only. The
/// shared batch lifecycle removes every changed-source incident edge and keeps
/// unaffected pairs. Local support never becomes whole-image equality.
/// # Errors
/// Policy/admission errors before iteration, resource limits or cancellation;
/// source and confirmation failures remain attributed file/pair diagnostics.
pub fn scan_projective_local_collection_candidate_union_regions_managed(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveCandidateUnionRegionsCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    LocalCollectionReport<crate::local_scan::ManagedProjectiveCandidateUnionRegionsFileEvidence>,
    ScanError,
> {
    crate::local_scan::validate_candidate_union_regions_file_policy(config.search, factors, &cancel)
        .map_err(|e| match e {
            LocalFileError::Cancelled => ScanError::Cancelled,
            LocalFileError::Features(LocalError::Budget)
            | LocalFileError::Geometry(crate::geometry::GeometryError::Budget)
            | LocalFileError::Pixels(crate::warp::WarpError::Budget) => ScanError::Budget,
            _ => ScanError::InvalidPolicy,
        })?;
    let search = config.search.search;
    let gradient = ProjectiveGradientCollectionPolicy {
        search: search.gradient,
        recipe: search.recipe,
        budgets: config.budgets,
        max_retrieval_comparisons: config.max_retrieval_comparisons,
    };
    scan_gradient_collection_with_confirmation(
        files,
        gradient,
        Some(search.spatial),
        Some(factors),
        Some(config.search.max_area_taps_per_extraction),
        Some((config.search, &[])),
        budget,
        cancel,
        |left, right, cancel| {
            crate::local_scan::compare_local_files_projective_candidate_union_regions_managed(
                left,
                right,
                config.search,
                factors,
                budget,
                cancel,
            )
        },
    )
}
fn extract_candidate_union_proposals(
    view: &crate::linear::LinearRgbaView<'_>,
    config: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<crate::gradient::GradientFeature>, LocalError> {
    let gradient = config.search.gradient;
    let grid = config.search.spatial;
    let extract = |view: &crate::linear::LinearRgbaView<'_>, score: f64| {
        let mut local = gradient.search.local.extract;
        local.minimum_corner_score = score;
        crate::gradient_scale::extract_spatial_gradient_scales_area_managed(
            view,
            crate::pyramid::PyramidPolicy {
                local,
                max_levels: gradient.search.max_levels,
                max_total_pixels: gradient.search.max_total_pixels,
                max_total_features: gradient.search.max_total_features,
            },
            factors,
            gradient.max_total_gradient_samples,
            config.search.recipe,
            (grid.columns, grid.rows, grid.max_per_cell),
            config.max_area_taps_per_extraction,
            budget,
            cancel,
        )
    };
    let original = extract(view, config.low_contrast_corner_score)?;
    let smoothed = {
        let samples = crate::gradient_scale::smooth_gradient_candidates_managed(
            view,
            config.smoothing_radius,
            config.max_smoothing_taps_per_image,
            budget,
            cancel,
        )?;
        let (w, h) = view.dimensions();
        let smooth = crate::linear::LinearRgbaView::new(
            w,
            h,
            &samples,
            gradient.search.local.decode.max_pixels,
            cancel,
        )
        .map_err(|e| match e {
            crate::pixels::PixelError::Layout => LocalError::Invalid,
            crate::pixels::PixelError::Budget => LocalError::Budget,
            crate::pixels::PixelError::Cancelled => LocalError::Cancelled,
        })?;
        extract(&smooth, gradient.search.local.extract.minimum_corner_score)?
    };
    let count = original
        .len()
        .checked_add(smoothed.len())
        .ok_or(LocalError::Budget)?;
    let bytes = count
        .checked_mul(std::mem::size_of::<crate::gradient::GradientFeature>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
    let mut combined = Vec::new();
    combined
        .try_reserve_exact(count)
        .map_err(|_| LocalError::Budget)?;
    for feature in original.iter().chain(smoothed.iter()) {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        combined.push(*feature);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    credit.try_adopt(combined).map_err(|_| LocalError::Budget)
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectiveCandidateUnionFallbackCollectionPolicy {
    pub search: crate::local_scan::ProjectiveCandidateUnionFallbackFilePolicy,
    pub budgets: FileFeatureBudgets,
    pub max_retrieval_comparisons: u64,
}
/// Index symmetric and both asymmetric candidate domains; confirm fresh pairs
/// with symmetric-first fallback. Changed-source incident edges are removed.
/// # Errors
/// Invalid policy or cumulative admission before iteration; bounded extraction,
/// retrieval, cancellation and attributed source/pair refusals.
pub fn scan_projective_local_collection_candidate_union_fallback_regions_managed(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveCandidateUnionFallbackCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    LocalCollectionReport<crate::local_scan::ManagedProjectiveCandidateUnionFallbackFileEvidence>,
    ScanError,
> {
    crate::local_scan::validate_candidate_union_fallback_file_policy(config.search, factors, &cancel)
        .map_err(|e| match e {
            LocalFileError::Cancelled => ScanError::Cancelled,
            LocalFileError::Features(LocalError::Budget)
            | LocalFileError::Geometry(crate::geometry::GeometryError::Budget)
            | LocalFileError::Pixels(crate::warp::WarpError::Budget) => ScanError::Budget,
            _ => ScanError::InvalidPolicy,
        })?;
    let union = config.search.search;
    let search = union.search;
    let gradient = ProjectiveGradientCollectionPolicy {
        search: search.gradient,
        recipe: search.recipe,
        budgets: config.budgets,
        max_retrieval_comparisons: config.max_retrieval_comparisons,
    };
    let radii = [config.search.asymmetric_radius, 0];
    scan_gradient_collection_with_confirmation(
        files,
        gradient,
        Some(search.spatial),
        Some(factors),
        Some(union.max_area_taps_per_extraction),
        Some((union, &radii)),
        budget,
        cancel,
        |left, right, cancel| {
            crate::local_scan::compare_local_files_projective_candidate_union_fallback_regions_managed(
                left,
                right,
                config.search,
                factors,
                budget,
                cancel,
            )
        },
    )
}
fn extract_candidate_union_portfolio_proposals(
    view: &crate::linear::LinearRgbaView<'_>,
    config: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    additional_radii: &[u32],
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<crate::gradient::GradientFeature>, LocalError> {
    let base = extract_candidate_union_proposals(view, config, factors, budget, cancel)?;
    if additional_radii.is_empty() {
        return Ok(base);
    }
    let mut values = Vec::new();
    let mut retained = budget.try_reserve(0).map_err(|_| LocalError::Budget)?;
    append_portfolio_batch(&mut values, &mut retained, base.iter().copied(), budget, cancel)?;
    drop(base);
    let g = config.search.gradient;
    let grid = config.search.spatial;
    let (w, h) = view.dimensions();
    for &radius in additional_radii {
        let samples = crate::gradient_scale::smooth_gradient_candidates_managed(
            view,
            radius,
            config.max_smoothing_taps_per_image,
            budget,
            cancel,
        )?;
        let smooth =
            crate::linear::LinearRgbaView::new(w, h, &samples, g.search.local.decode.max_pixels, cancel)
                .map_err(|e| match e {
                    crate::pixels::PixelError::Layout => LocalError::Invalid,
                    crate::pixels::PixelError::Budget => LocalError::Budget,
                    crate::pixels::PixelError::Cancelled => LocalError::Cancelled,
                })?;
        let batch = crate::gradient_scale::extract_spatial_gradient_scales_area_managed(
            &smooth,
            crate::pyramid::PyramidPolicy {
                local: g.search.local.extract,
                max_levels: g.search.max_levels,
                max_total_pixels: g.search.max_total_pixels,
                max_total_features: g.search.max_total_features,
            },
            factors,
            g.max_total_gradient_samples,
            config.search.recipe,
            (grid.columns, grid.rows, grid.max_per_cell),
            config.max_area_taps_per_extraction,
            budget,
            cancel,
        )?;
        append_portfolio_batch(&mut values, &mut retained, batch.iter().copied(), budget, cancel)?;
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    retained.try_adopt(values).map_err(|_| LocalError::Budget)
}

/// Descriptor-indexed candidate retrieval followed by fresh guarded anchor files.
/// Batch mutation invalidates all incident pairs; rank refusals remain pair issues.
/// This returns local support evidence, never exact identity or whole-file copies.
pub fn scan_anchor_rank_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveCandidateUnionRegionsCollectionPolicy,
    factors: &[f64],
    anchors: crate::anchor_rank::AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    LocalCollectionReport<crate::anchor_rank_file::AnchorCandidateSearchEvidence>,
    ScanError,
> {
    admit_anchor_collection(config.search.max_union_points, anchors, &cancel)?;
    crate::local_scan::validate_candidate_union_regions_file_policy(config.search, factors, &cancel)
        .map_err(|e| match e {
            LocalFileError::Cancelled => ScanError::Cancelled,
            LocalFileError::Features(LocalError::Budget)
            | LocalFileError::Geometry(crate::geometry::GeometryError::Budget)
            | LocalFileError::Pixels(crate::warp::WarpError::Budget) => ScanError::Budget,
            _ => ScanError::InvalidPolicy,
        })?;
    let search = config.search.search;
    let gradient = ProjectiveGradientCollectionPolicy {
        search: search.gradient,
        recipe: search.recipe,
        budgets: config.budgets,
        max_retrieval_comparisons: config.max_retrieval_comparisons,
    };
    scan_gradient_collection_with_confirmation(
        files,
        gradient,
        Some(search.spatial),
        Some(factors),
        Some(config.search.max_area_taps_per_extraction),
        Some((config.search, &[])),
        budget,
        cancel,
        |left, right, cancel| {
            crate::anchor_rank_file::search_anchor_rank_files(
                left, right, config.search, factors, anchors, budget, cancel,
            ).map_err(|error| match error {
                crate::anchor_rank_file::AnchorRankFileError::Cancelled => LocalFileError::Cancelled,
                crate::anchor_rank_file::AnchorRankFileError::Source(e) => LocalFileError::Source(e),
                crate::anchor_rank_file::AnchorRankFileError::Decode(e) => LocalFileError::Decode(e),
                crate::anchor_rank_file::AnchorRankFileError::Rank(e) => LocalFileError::Rank(e),
                crate::anchor_rank_file::AnchorRankFileError::Search(e) => e,
            })
        },
    )
}
/// Descriptor-indexed retrieval followed by geometry and original-pixel anchors.
/// Omits legacy regional color diagnostics; preserves proposal and source guards.
/// Batch mutation invalidates all incident pairs; rank refusals remain pair issues.
/// This returns local support evidence, never exact identity or whole-file copies.
pub fn scan_anchor_rank_geometry_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveCandidateUnionRegionsCollectionPolicy,
    factors: &[f64],
    anchors: crate::anchor_rank::AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    LocalCollectionReport<crate::anchor_rank_file::AnchorGeometrySearchEvidence>,
    ScanError,
> {
    admit_anchor_collection(config.search.max_union_points, anchors, &cancel)?;
    crate::local_scan::validate_candidate_union_regions_file_policy(config.search, factors, &cancel)
        .map_err(|e| match e {
            LocalFileError::Cancelled => ScanError::Cancelled,
            LocalFileError::Features(LocalError::Budget)
            | LocalFileError::Geometry(crate::geometry::GeometryError::Budget)
            | LocalFileError::Pixels(crate::warp::WarpError::Budget) => ScanError::Budget,
            _ => ScanError::InvalidPolicy,
        })?;
    let search = config.search.search;
    let gradient = ProjectiveGradientCollectionPolicy {
        search: search.gradient,
        recipe: search.recipe,
        budgets: config.budgets,
        max_retrieval_comparisons: config.max_retrieval_comparisons,
    };
    scan_gradient_collection_with_confirmation(
        files,
        gradient,
        Some(search.spatial),
        Some(factors),
        Some(config.search.max_area_taps_per_extraction),
        Some((config.search, &[])),
        budget,
        cancel,
        |left, right, cancel| {
            crate::anchor_rank_file::search_anchor_rank_geometry_files(
                left, right, config.search, factors, anchors, budget, cancel,
            ).map_err(|error| match error {
                crate::anchor_rank_file::AnchorRankFileError::Cancelled => LocalFileError::Cancelled,
                crate::anchor_rank_file::AnchorRankFileError::Source(e) => LocalFileError::Source(e),
                crate::anchor_rank_file::AnchorRankFileError::Decode(e) => LocalFileError::Decode(e),
                crate::anchor_rank_file::AnchorRankFileError::Rank(e) => LocalFileError::Rank(e),
                crate::anchor_rank_file::AnchorRankFileError::Search(e) => e,
            })
        },
    )
}
fn admit_anchor_collection(
    union_bound: usize,
    anchors: crate::anchor_rank::AnchorRankPolicy,
    cancel: &dyn Fn() -> bool,
) -> Result<(), ScanError> {
    if cancel() { return Err(ScanError::Cancelled); }
    crate::anchor_rank::validate_anchor_rank_policy(anchors)
        .map_err(|_| ScanError::InvalidPolicy)?;
    // Admit the complete configured union bound before consuming file input.
    let n = u64::try_from(union_bound).map_err(|_| ScanError::Budget)?;
    let pairs = n.checked_mul(n.saturating_sub(1)).and_then(|v|v.checked_div(2)).ok_or(ScanError::Budget)?;
    let size = anchors.window_radius.checked_mul(2).and_then(|v|v.checked_add(1)).ok_or(ScanError::Budget)?;
    let pad = anchors.local.rank.radius.checked_add(anchors.local.filter_radius).ok_or(ScanError::Budget)?;
    let extent = u64::from(size).checked_add(2*u64::from(pad)).ok_or(ScanError::Budget)?;
    let sites = n.checked_mul(2).and_then(|v|v.checked_mul(extent)).and_then(|v|v.checked_mul(extent)).ok_or(ScanError::Budget)?;
    if union_bound > anchors.local.maximum_points
        || pairs.checked_add(n).ok_or(ScanError::Budget)? > anchors.local.maximum_point_checks
        || pairs.checked_mul(3).and_then(|v|v.checked_add(n)).ok_or(ScanError::Budget)? > anchors.maximum_selection_checks
        || sites > anchors.local.rank.maximum_sites
        || sites.checked_mul(5).ok_or(ScanError::Budget)? > anchors.local.rank.maximum_pixel_reads {
        return Err(ScanError::Budget);
    }
    Ok(())
}

/// Indexed symmetric/asymmetric proposal domains followed by fresh guarded fallback anchors.
/// Shared batch source invalidation and typed pair refusals are preserved.
pub fn scan_anchor_rank_fallback_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: ProjectiveCandidateUnionFallbackCollectionPolicy,
    factors: &[f64],
    anchors: crate::anchor_rank::AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    LocalCollectionReport<crate::anchor_rank_file::AnchorFallbackSearchEvidence>,
    ScanError,
> {
    admit_anchor_collection(config.search.search.max_union_points, anchors, &cancel)?;
    crate::local_scan::validate_candidate_union_fallback_file_policy(config.search, factors, &cancel)
        .map_err(|e| match e {
            LocalFileError::Cancelled => ScanError::Cancelled,
            LocalFileError::Features(LocalError::Budget)
            | LocalFileError::Geometry(crate::geometry::GeometryError::Budget)
            | LocalFileError::Pixels(crate::warp::WarpError::Budget) => ScanError::Budget,
            _ => ScanError::InvalidPolicy,
        })?;
    let union = config.search.search;
    let search = union.search;
    let gradient = ProjectiveGradientCollectionPolicy {
        search: search.gradient,
        recipe: search.recipe,
        budgets: config.budgets,
        max_retrieval_comparisons: config.max_retrieval_comparisons,
    };
    let radii = [config.search.asymmetric_radius, 0];
    scan_gradient_collection_with_confirmation(
        files,
        gradient,
        Some(search.spatial),
        Some(factors),
        Some(union.max_area_taps_per_extraction),
        Some((union, &radii)),
        budget,
        cancel,
        |left, right, cancel| {
            crate::anchor_rank_file::search_anchor_rank_fallback_files(
                left, right, config.search, factors, anchors, budget, cancel,
            ).map_err(|error| match error {
                crate::anchor_rank_file::AnchorRankFileError::Cancelled => LocalFileError::Cancelled,
                crate::anchor_rank_file::AnchorRankFileError::Source(e) => LocalFileError::Source(e),
                crate::anchor_rank_file::AnchorRankFileError::Decode(e) => LocalFileError::Decode(e),
                crate::anchor_rank_file::AnchorRankFileError::Rank(e) => LocalFileError::Rank(e),
                crate::anchor_rank_file::AnchorRankFileError::Search(e) => e,
            })
        },
    )
}

/// Indexed proposal portfolio and explicit cumulative anchor confirmation limits.
#[derive(Debug, Clone, Copy)]
pub struct AnchorGeometryFallbackCollectionPolicy {
    pub search: crate::anchor_rank_file::AnchorGeometryFallbackPolicy,
    pub budgets: crate::local_index::FileFeatureBudgets,
    pub max_retrieval_comparisons: u64,
}

/// Indexed symmetric/asymmetric proposals followed by fresh pixel-rank-selected fallback.
/// Common batch invalidation removes every incident pair after source changes.
/// Returns finite local support and typed issues, never whole-file identity.
pub fn scan_anchor_rank_geometry_fallback_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    config: AnchorGeometryFallbackCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    LocalCollectionReport<crate::anchor_rank_file::AnchorGeometryFallbackEvidence>,
    ScanError,
> {
    admit_anchor_collection(config.search.search.search.max_union_points, config.search.anchors, &cancel)?;
    crate::anchor_rank_file::validate_anchor_geometry_fallback_policy(config.search, factors, &cancel)
        .map_err(|e| match e {
            crate::anchor_rank_file::AnchorRankFileError::Cancelled => ScanError::Cancelled,
            crate::anchor_rank_file::AnchorRankFileError::Rank(crate::local_rank::LocalRankError::Budget)
            | crate::anchor_rank_file::AnchorRankFileError::Search(LocalFileError::Features(LocalError::Budget)) => ScanError::Budget,
            _ => ScanError::InvalidPolicy,
        })?;
    let union = config.search.search.search;
    let search = union.search;
    let gradient = ProjectiveGradientCollectionPolicy {
        search: search.gradient,
        recipe: search.recipe,
        budgets: config.budgets,
        max_retrieval_comparisons: config.max_retrieval_comparisons,
    };
    let radii = [config.search.search.asymmetric_radius, 0];
    scan_gradient_collection_with_confirmation(
        files,
        gradient,
        Some(search.spatial),
        Some(factors),
        Some(union.max_area_taps_per_extraction),
        Some((union, &radii)),
        budget,
        cancel,
        |left, right, cancel| {
            crate::anchor_rank_file::search_anchor_rank_geometry_fallback_files(
                left, right, config.search, factors, budget, cancel,
            ).map_err(|error| match error {
                crate::anchor_rank_file::AnchorRankFileError::Cancelled => LocalFileError::Cancelled,
                crate::anchor_rank_file::AnchorRankFileError::Source(e) => LocalFileError::Source(e),
                crate::anchor_rank_file::AnchorRankFileError::Decode(e) => LocalFileError::Decode(e),
                crate::anchor_rank_file::AnchorRankFileError::Rank(e) => LocalFileError::Rank(e),
                crate::anchor_rank_file::AnchorRankFileError::Search(e) => e,
            })
        },
    )
}