//! Explicit complete-presentation equality, distinct from encoded-byte identity.
use crate::{
    animated::{AnimationBudget, AnimationError, AnimationKind, decode_animation},
    decode::DecodeSourceSnapshot as ContentSnapshot,
    decode::{FileError, decode_selected_frame},
    exact::SnapshotError,
    pages::{PageKind, decode_pages},
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;

#[derive(Debug, Clone, Copy)]
pub enum Presentation {
    SelectedFrame,
    Pages(PageKind),
    Animation(AnimationKind),
}

#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    #[error("invalid or over-budget presentation metadata")]
    Detection,
    #[error("comparison cancelled")]
    Cancelled,
    #[error("presentation modes are incompatible")]
    Incompatible,
    #[error(transparent)]
    Source(#[from] SnapshotError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Animation(#[from] AnimationError),
}

/// Compare complete explicitly selected presentation modes. Page order/count,
/// animation timeline/repetition and selected-frame scope are kept distinct.
/// Both sources are revalidated after both decodes, protecting the first source
/// against mutation during processing of the second. No encoded-byte claim.
///
/// # Errors
/// Returns mode, source, decode, resource, timing or cancellation failures.
pub fn same_presentation(
    left: (&DecodeRequest, Presentation),
    right: (&DecodeRequest, Presentation),
    limits: AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<bool, ContainerError> {
    let cancelled = || {
        cancel()
            || [left.0, right.0].iter().any(|request| {
                request
                    .cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            })
    };
    if cancelled() {
        return Err(ContainerError::Cancelled);
    }
    // Reject mode errors before reading unrelated files.
    if !matches!(
        (left.1, right.1),
        (Presentation::SelectedFrame, Presentation::SelectedFrame)
            | (Presentation::Pages(_), Presentation::Pages(_))
            | (Presentation::Animation(_), Presentation::Animation(_))
    ) {
        return Err(ContainerError::Incompatible);
    }
    let a = ContentSnapshot::read(&left.0.path, limits.max_file_bytes, cancelled)?;
    let b = ContentSnapshot::read(&right.0.path, limits.max_file_bytes, cancelled)?;
    let result = (|| -> Result<bool, ContainerError> {
        let x = prepare_presentation(left.0, left.1, &a, limits, budget, cancelled)?;
        let y = prepare_presentation(right.0, right.1, &b, limits, budget, cancelled)?;
        let equal = x.same(&y, cancelled)?;
        a.verify(cancelled)?;
        b.verify(cancelled)?;
        Ok(equal)
    })();
    if cancelled() {
        return Err(ContainerError::Cancelled);
    }
    result
}

enum PreparedPresentation {
    Selected(crate::raster::NormalizedRaster),
    Pages(crate::pages::Pages),
    Animation(crate::animated::Animation),
}
impl PreparedPresentation {
    fn same(&self, other: &Self, cancel: impl Fn() -> bool) -> Result<bool, ContainerError> {
        match (self, other) {
            (Self::Selected(a), Self::Selected(b)) => {
                Ok(a.same_selected_frame(b, cancel).map_err(FileError::from)?)
            }
            (Self::Pages(a), Self::Pages(b)) => Ok(a.same_pages(b, cancel)?),
            (Self::Animation(a), Self::Animation(b)) => Ok(a.same_timeline(b, cancel)?),
            _ => Err(ContainerError::Incompatible),
        }
    }
}
struct PresentationSignature {
    key: [u8; 32],
    source: ContentSnapshot,
}
fn presentation_signature(
    request: &DecodeRequest,
    mode: Presentation,
    limits: AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<PresentationSignature, ContainerError> {
    let source = ContentSnapshot::read(&request.path, limits.max_file_bytes, &cancel)?;
    let prepared = prepare_presentation(request, mode, &source, limits, budget, &cancel)?;
    let key = match prepared {
        PreparedPresentation::Selected(ref value) => {
            value.selected_frame_digest(&cancel).map_err(FileError::from)?
        }
        PreparedPresentation::Pages(ref value) => value.page_digest(&cancel)?,
        PreparedPresentation::Animation(ref value) => value.timeline_digest(&cancel)?,
    };
    #[cfg(test)]
    let key = match KEY_TEST_MODE.with(std::cell::Cell::get) {
        1 => [0; 32],
        2 => return Err(AnimationError::Sequence(crate::sequence::SequenceError::Timing).into()),
        _ => key,
    };
    source.verify(&cancel)?;
    if cancel() {
        return Err(ContainerError::Cancelled);
    }
    Ok(PresentationSignature { key, source })
}
fn compatible_modes(left: Presentation, right: Presentation) -> bool {
    matches!(
        (left, right),
        (Presentation::SelectedFrame, Presentation::SelectedFrame)
            | (Presentation::Pages(_), Presentation::Pages(_))
            | (Presentation::Animation(_), Presentation::Animation(_))
    )
}
fn signatures_differ(
    a: &PresentationSignature,
    b: &PresentationSignature,
    cancel: impl Fn() -> bool,
) -> Result<bool, ContainerError> {
    if a.key == b.key {
        return Ok(false);
    }
    a.source.verify(&cancel)?;
    b.source.verify(&cancel)?;
    if cancel() {
        return Err(ContainerError::Cancelled);
    }
    Ok(true)
}

fn prepare_presentation(
    request: &DecodeRequest,
    mode: Presentation,
    source: &ContentSnapshot,
    limits: AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<PreparedPresentation, ContainerError> {
    #[cfg(test)]
    PREPARE_CALLS.with(|count| count.set(count.get() + 1));
    Ok(match mode {
        Presentation::SelectedFrame => {
            let magic = source.read_prefix(4, &cancel)?;
            let mut classified = request.clone();
            classified.memory_budget = Some(budget.clone());
            let sensor = rrrah_decode::image_source_kind(&classified).map_err(FileError::from)?
                == rrrah_decode::ImageSourceKind::Sensor;
            let raster = if !sensor
                && matches!(
                    magic.as_slice(),
                    [b'I', b'I', 42 | 43, 0] | [b'M', b'M', 0, 42 | 43]
                ) {
                crate::pages::decode_tiff_selected(request, limits, budget, &cancel)?
            } else {
                decode_selected_frame(request, limits.max_pixels, budget, &cancel)?
            };
            PreparedPresentation::Selected(raster)
        }
        Presentation::Pages(kind) => {
            PreparedPresentation::Pages(decode_pages(request, kind, limits, budget, cancel)?)
        }
        Presentation::Animation(kind) => {
            PreparedPresentation::Animation(decode_animation(request, kind, limits, budget, cancel)?)
        }
    })
}
struct PreparedLeft {
    id: u64,
    source: ContentSnapshot,
    pixels: PreparedPresentation,
}
#[allow(clippy::too_many_arguments)] // One retained left presentation; right is released after each pair.
fn compare_reusing_left(
    retained: &mut Option<PreparedLeft>,
    id: u64,
    left: &(DecodeRequest, Presentation),
    right: &(DecodeRequest, Presentation),
    limits: AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<bool, ContainerError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [&left.0, &right.0].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    if cancelled() {
        return Err(ContainerError::Cancelled);
    }
    if retained.as_ref().is_some_and(|value| value.id != id) {
        *retained = None;
    }
    if !matches!(
        (left.1, right.1),
        (Presentation::SelectedFrame, Presentation::SelectedFrame)
            | (Presentation::Pages(_), Presentation::Pages(_))
            | (Presentation::Animation(_), Presentation::Animation(_))
    ) {
        return Err(ContainerError::Incompatible);
    }
    let result = (|| {
        let fresh = if let Some(value) = retained.as_ref() {
            value.source.verify(cancelled)?;
            None
        } else {
            Some(ContentSnapshot::read(
                &left.0.path,
                limits.max_file_bytes,
                cancelled,
            )?)
        };
        // Observe both files before either new decode, as in direct comparison.
        let source = ContentSnapshot::read(&right.0.path, limits.max_file_bytes, cancelled)?;
        if let Some(left_source) = fresh {
            let pixels = prepare_presentation(&left.0, left.1, &left_source, limits, budget, cancelled)?;
            left_source.verify(cancelled)?;
            *retained = Some(PreparedLeft {
                id,
                source: left_source,
                pixels,
            });
        }
        let pixels = prepare_presentation(&right.0, right.1, &source, limits, budget, cancelled)?;
        let value = retained.as_ref().ok_or(ContainerError::Detection)?;
        let equal = value.pixels.same(&pixels, cancelled)?;
        value.source.verify(cancelled)?;
        source.verify(cancelled)?;
        Ok(equal)
    })();
    if cancelled() {
        return Err(ContainerError::Cancelled);
    }
    result
}

#[derive(Debug)]
pub struct PresentationIssue {
    pub left: u64,
    pub right: u64,
    pub error: ContainerError,
}

#[derive(Debug)]
pub struct PresentationReport {
    pub equal: Vec<(u64, u64)>,
    pub different: Vec<(u64, u64)>,
    pub issues: Vec<PresentationIssue>,
}

/// Confirm bounded supplied pairs with explicitly selected presentation modes.
/// Every pair is normalized/deduplicated and reported in id order. Full animation
/// timelines and ordered pages share direct-comparison decoding and equality.
/// A bounded left presentation is reused, with source validation for every pair.
/// Modes must describe intended scope; selected-frame pairs retain that scope.
///
/// # Errors
/// Returns invalid ids, raw input limits or cancellation without a partial report.
#[allow(clippy::too_many_lines)] // Keep admission, signature fallback and pair evidence in one transaction.
pub fn confirm_presentations(
    files: impl IntoIterator<Item = (u64, DecodeRequest, Presentation)>,
    pairs: impl IntoIterator<Item = (u64, u64)>,
    max_files: usize,
    max_pairs: usize,
    limits: AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<PresentationReport, crate::scan::ScanError> {
    use crate::scan::ScanError;
    use std::collections::{HashMap, HashSet};
    let mut requests = HashMap::new();
    for (id, request, mode) in files {
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
        requests.insert(id, (request, mode));
    }
    let mut candidates = HashSet::new();
    for (consumed, (a, b)) in pairs.into_iter().enumerate() {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        if consumed >= max_pairs {
            return Err(ScanError::Budget);
        }
        if a == b || !requests.contains_key(&a) || !requests.contains_key(&b) {
            return Err(ScanError::InvalidPair);
        }
        let pair = (a.min(b), a.max(b));
        if !candidates.contains(&pair) {
            candidates.try_reserve(1).map_err(|_| ScanError::Budget)?;
            candidates.insert(pair);
        }
    }
    let mut report = PresentationReport {
        equal: Vec::new(),
        different: Vec::new(),
        issues: Vec::new(),
    };
    let mut ordered_pairs = Vec::new();
    ordered_pairs
        .try_reserve_exact(candidates.len())
        .map_err(|_| ScanError::Budget)?;
    ordered_pairs.extend(candidates);
    ordered_pairs.sort_unstable();
    let mut tokens = Vec::new();
    tokens
        .try_reserve_exact(requests.len())
        .map_err(|_| ScanError::Budget)?;
    tokens.extend(requests.values().filter_map(|(r, _)| r.cancellation.clone()));
    let latch = std::cell::Cell::new(false);
    let caller_cancel = &cancel;
    let cancel = || {
        let value =
            latch.get() || caller_cancel() || tokens.iter().any(rrrah_decode::GenerationToken::is_cancelled);
        latch.set(value);
        value
    };
    let mut signatures = HashMap::new();
    // Only sources participating in admitted pairs are decoded. One signature
    // preparation is released before the next, respecting the existing budget.
    for &(a, b) in &ordered_pairs {
        if !compatible_modes(requests[&a].1, requests[&b].1) {
            continue;
        }
        for id in [a, b] {
            if signatures.contains_key(&id) {
                continue;
            }
            if cancel() {
                return Err(ScanError::Cancelled);
            }
            let (request, mode) = &requests[&id];
            let value = presentation_signature(request, *mode, limits, budget, cancel).ok();
            if cancel() {
                return Err(ScanError::Cancelled);
            }
            signatures.try_reserve(1).map_err(|_| ScanError::Budget)?;
            // Signature failure is inconclusive, not a negative; direct pair
            // confirmation preserves errors and rational-overflow fallback.
            signatures.insert(id, value);
        }
    }
    let mut retained = None;
    for (a, b) in ordered_pairs {
        let left = &requests[&a];
        let right = &requests[&b];
        let compatible = compatible_modes(left.1, right.1);
        let filtered = if compatible {
            match (&signatures[&a], &signatures[&b]) {
                (Some(x), Some(y)) => signatures_differ(x, y, cancel),
                _ => Ok(false),
            }
        } else {
            Ok(false)
        };
        let result = match filtered {
            Ok(true) => Ok(false),
            Ok(false) => compare_reusing_left(&mut retained, a, left, right, limits, budget, cancel),
            Err(error) => Err(error),
        };
        if cancel() || matches!(result, Err(ContainerError::Cancelled)) {
            return Err(ScanError::Cancelled);
        }
        match result {
            Ok(true) => {
                crate::local::reserve_slot(&mut report.equal, max_pairs).map_err(|_| ScanError::Budget)?;
                report.equal.push((a, b));
            }
            Ok(false) => {
                crate::local::reserve_slot(&mut report.different, max_pairs)
                    .map_err(|_| ScanError::Budget)?;
                report.different.push((a, b));
            }
            Err(error) => {
                crate::local::reserve_slot(&mut report.issues, max_pairs).map_err(|_| ScanError::Budget)?;
                report.issues.push(PresentationIssue {
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

/// Exhaustively compare every distinct input pair under a mandatory pair budget.
/// No perceptual/first-frame filter can omit whole-presentation equality. Work
/// grows quadratically; the left presentation is reused within each sorted row,
/// with full source verification before and after each pair.
///
/// # Errors
/// Rejects duplicate ids, input/pair limits or cancellation without partial output.
pub fn scan_presentations(
    files: impl IntoIterator<Item = (u64, DecodeRequest, Presentation)>,
    max_files: usize,
    max_pairs: usize,
    limits: AnimationBudget,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<PresentationReport, crate::scan::ScanError> {
    use crate::scan::ScanError;
    let mut admitted = std::collections::HashMap::new();
    for (id, request, mode) in files {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        if admitted.contains_key(&id) {
            return Err(ScanError::DuplicateId);
        }
        if admitted.len() >= max_files {
            return Err(ScanError::Budget);
        }
        admitted.try_reserve(1).map_err(|_| ScanError::Budget)?;
        admitted.insert(id, (request, mode));
    }
    let count = admitted.len();
    let pairs = count
        .checked_mul(count.saturating_sub(1))
        .and_then(|n| n.checked_div(2))
        .ok_or(ScanError::Budget)?;
    if pairs > max_pairs {
        return Err(ScanError::Budget);
    }
    let mut ids = Vec::new();
    ids.try_reserve_exact(admitted.len())
        .map_err(|_| ScanError::Budget)?;
    ids.extend(admitted.keys().copied());
    ids.sort_unstable();
    let candidates = ids
        .iter()
        .enumerate()
        .flat_map(|(i, &left)| ids[i + 1..].iter().map(move |&right| (left, right)));
    confirm_presentations(
        admitted
            .into_iter()
            .map(|(id, (request, mode))| (id, request, mode)),
        candidates,
        max_files,
        max_pairs,
        limits,
        budget,
        cancel,
    )
}

#[derive(Debug, Clone, Copy)]
pub struct PresentationPolicy {
    pub max_files: usize,
    pub max_pairs: usize,
    pub limits: AnimationBudget,
    pub grouping: crate::groups::GroupBudget,
}

#[derive(Debug)]
pub struct PresentationGroups {
    pub grouping: crate::groups::Grouping,
    pub comparisons: PresentationReport,
    pub source_issues: Vec<(u64, SnapshotError)>,
}

/// Exhaustive full-presentation search and complete-link groups, admitting only
/// edges whose sources retain their initial content/identity across the batch.
/// Invalid sources retain diagnostics and cannot contribute equality edges.
/// Singletons do not prove uniqueness. Filesystem observation is not atomic.
///
/// # Errors
/// Returns input/work/storage budgets, duplicate ids or cancellation.
#[allow(clippy::too_many_lines)] // Keep source observations, comparison and batch-wide invalidation together.
pub fn scan_presentation_groups(
    files: impl IntoIterator<Item = (u64, DecodeRequest, Presentation)>,
    policy: PresentationPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<PresentationGroups, crate::scan::ScanError> {
    scan_presentation_groups_selected(files, policy, budget, cancel, false).map(|report| report.result)
}

/// Candidate-only full-presentation groups with direct collision confirmation.
/// Different signature buckets are omitted, not returned as all negative pairs.
/// Failed signatures expand to all compatible sources; candidate work is bounded
/// by `max_pairs`. Singletons still do not prove uniqueness.
///
/// # Errors
/// Invalid ids, source/work/storage limits or latched cancellation without a partial report.
pub fn scan_indexed_presentation_groups(
    files: impl IntoIterator<Item = (u64, DecodeRequest, Presentation)>,
    policy: PresentationPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<IndexedPresentationGroups, crate::scan::ScanError> {
    scan_presentation_groups_selected(files, policy, budget, cancel, true)
}

#[derive(Debug)]
pub struct IndexedPresentationGroups {
    pub result: PresentationGroups,
    pub candidate_pairs: usize,
    /// Initial retrieval attempts only; excludes key preparation inside confirmation.
    pub signature_preparations: usize,
    /// Signature failures are inconclusive and expand candidate work, never establish uniqueness.
    pub preparation_issues: Vec<(u64, ContainerError)>,
}

#[allow(clippy::too_many_lines)] // Share source admission/invalidation between exhaustive and indexed groups.
fn scan_presentation_groups_selected(
    files: impl IntoIterator<Item = (u64, DecodeRequest, Presentation)>,
    policy: PresentationPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    indexed: bool,
) -> Result<IndexedPresentationGroups, crate::scan::ScanError> {
    use crate::scan::ScanError;
    use std::collections::{HashMap, HashSet};
    let mut admitted = HashMap::new();
    for (id, request, mode) in files {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        if admitted.contains_key(&id) {
            return Err(ScanError::DuplicateId);
        }
        if admitted.len() >= policy.max_files {
            return Err(ScanError::Budget);
        }
        admitted.try_reserve(1).map_err(|_| ScanError::Budget)?;
        admitted.insert(id, (request, mode));
    }
    let mut tokens = Vec::new();
    tokens
        .try_reserve_exact(admitted.len())
        .map_err(|_| ScanError::Budget)?;
    tokens.extend(
        admitted
            .values()
            .filter_map(|(request, _)| request.cancellation.clone()),
    );
    let caller_cancel = &cancel;
    let latch = std::cell::Cell::new(false);
    let cancel = || {
        let value =
            latch.get() || caller_cancel() || tokens.iter().any(rrrah_decode::GenerationToken::is_cancelled);
        latch.set(value);
        value
    };
    let count = admitted.len();
    if !indexed
        && count
            .checked_mul(count.saturating_sub(1))
            .map(|n| n / 2)
            .ok_or(ScanError::Budget)?
            > policy.max_pairs
    {
        return Err(ScanError::Budget);
    }
    let mut ids = Vec::new();
    ids.try_reserve_exact(admitted.len())
        .map_err(|_| ScanError::Budget)?;
    ids.extend(admitted.keys().copied());
    ids.sort_unstable();
    let mut snapshots = Vec::new();
    let mut source_issues = Vec::new();
    for &id in &ids {
        let (request, _) = &admitted[&id];
        match ContentSnapshot::read(&request.path, policy.limits.max_file_bytes, cancel) {
            Ok(snapshot) => {
                crate::local::reserve_slot(&mut snapshots, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                snapshots.push((id, snapshot));
            }
            Err(SnapshotError::Cancelled) => return Err(ScanError::Cancelled),
            Err(error) => {
                crate::local::reserve_slot(&mut source_issues, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                source_issues.push((id, error));
            }
        }
    }
    let mut preparation_issues = Vec::new();
    let (candidate_pairs, signature_preparations, mut comparisons) = if indexed {
        let mut buckets: HashMap<(u8, [u8; 32]), Vec<u64>> = HashMap::new();
        let mut unknown = Vec::new();
        for &id in &ids {
            if cancel() {
                return Err(ScanError::Cancelled);
            }
            let (request, mode) = &admitted[&id];
            match presentation_signature(request, *mode, policy.limits, budget, cancel) {
                Ok(signature) => {
                    let scope = match mode {
                        Presentation::SelectedFrame => 0,
                        Presentation::Pages(_) => 1,
                        Presentation::Animation(_) => 2,
                    };
                    buckets.try_reserve(1).map_err(|_| ScanError::Budget)?;
                    let bucket = buckets.entry((scope, signature.key)).or_default();
                    crate::local::reserve_slot(bucket, policy.max_files).map_err(|_| ScanError::Budget)?;
                    bucket.push(id);
                }
                Err(error) => {
                    if cancel() {
                        return Err(ScanError::Cancelled);
                    }
                    crate::local::reserve_slot(&mut unknown, policy.max_files)
                        .map_err(|_| ScanError::Budget)?;
                    unknown.push(id);
                    crate::local::reserve_slot(&mut preparation_issues, policy.max_files)
                        .map_err(|_| ScanError::Budget)?;
                    preparation_issues.push((id, error));
                }
            }
        }
        let mut candidates = HashSet::new();
        for bucket in buckets.values() {
            for (i, &left) in bucket.iter().enumerate() {
                for &right in &bucket[i + 1..] {
                    if cancel() {
                        return Err(ScanError::Cancelled);
                    }
                    if candidates.len() >= policy.max_pairs {
                        return Err(ScanError::Budget);
                    }
                    candidates.try_reserve(1).map_err(|_| ScanError::Budget)?;
                    candidates.insert((left, right));
                }
            }
        }
        for left in unknown {
            for &right in &ids {
                if cancel() {
                    return Err(ScanError::Cancelled);
                }
                if left == right || !compatible_modes(admitted[&left].1, admitted[&right].1) {
                    continue;
                }
                let pair = (left.min(right), left.max(right));
                if candidates.contains(&pair) {
                    continue;
                }
                if candidates.len() >= policy.max_pairs {
                    return Err(ScanError::Budget);
                }
                candidates.try_reserve(1).map_err(|_| ScanError::Budget)?;
                candidates.insert(pair);
            }
        }
        let candidate_pairs = candidates.len();
        let comparisons = confirm_presentations(
            admitted
                .into_iter()
                .map(|(id, (request, mode))| (id, request, mode)),
            candidates,
            policy.max_files,
            policy.max_pairs,
            policy.limits,
            budget,
            cancel,
        )?;
        (candidate_pairs, ids.len(), comparisons)
    } else {
        let comparisons = scan_presentations(
            admitted
                .into_iter()
                .map(|(id, (request, mode))| (id, request, mode)),
            policy.max_files,
            policy.max_pairs,
            policy.limits,
            budget,
            cancel,
        )?;
        (count.saturating_mul(count.saturating_sub(1)) / 2, 0, comparisons)
    };
    for (id, snapshot) in snapshots {
        match snapshot.verify(cancel) {
            Ok(()) => {}
            Err(SnapshotError::Cancelled) => return Err(ScanError::Cancelled),
            Err(error) => {
                crate::local::reserve_slot(&mut source_issues, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                source_issues.push((id, error));
            }
        }
    }
    source_issues.sort_by_key(|(id, _)| *id);
    let mut invalid = HashSet::new();
    invalid
        .try_reserve(source_issues.len())
        .map_err(|_| ScanError::Budget)?;
    invalid.extend(source_issues.iter().map(|(id, _)| *id));
    comparisons
        .equal
        .retain(|(a, b)| !invalid.contains(a) && !invalid.contains(b));
    comparisons
        .different
        .retain(|(a, b)| !invalid.contains(a) && !invalid.contains(b));
    let grouping = crate::groups::complete_link_groups(
        ids,
        comparisons
            .equal
            .iter()
            .map(|&(left, right)| crate::groups::Pair { left, right }),
        policy.grouping,
        cancel,
    )?;
    Ok(IndexedPresentationGroups {
        result: PresentationGroups {
            grouping,
            comparisons,
            source_issues,
        },
        candidate_pairs,
        signature_preparations,
        preparation_issues,
    })
}

/// Indexed retrieval diagnostics accompany the full directory report.
#[derive(Debug)]
pub struct IndexedDirectoryGroups<T> {
    pub result: T,
    pub candidate_pairs: usize,
    /// Initial retrieval preparations; direct confirmation can decode again.
    pub signature_preparations: usize,
    pub preparation_issues: Vec<(u64, ContainerError)>,
}

type SelectionStats = (usize, usize, Vec<(u64, ContainerError)>);

#[derive(Debug)]
pub struct DirectoryPresentationGroups {
    /// Per-run deterministic ids; not persistent file identities.
    pub files: Vec<(u64, std::path::PathBuf, Presentation)>,
    pub aliases: Vec<crate::exact::Aliases>,
    pub traversal_issues: Vec<crate::exact::Issue>,
    pub result: PresentationGroups,
}

/// Recursively discover physical files and group complete selected presentations.
/// Caller explicitly chooses selected-frame/pages/animation scope per path; no
/// filename filter silently removes unsupported files. Every admitted pair is
/// exhaustively compared under policy limits. Aliases remain separate from copies.
///
/// # Errors
/// Returns traversal cancellation, batch/group limits or duplicate-id failures.
pub fn scan_presentation_roots(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    mode: impl Fn(&std::path::Path) -> Presentation,
    policy: PresentationPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<DirectoryPresentationGroups, crate::scan::ScanError> {
    scan_presentation_roots_selected(roots, traversal, mode, policy, budget, cancel, false)
        .map(|report| report.0)
}

/// Recursively retrieve complete-presentation candidates and directly confirm them.
/// Omitted pairs are inconclusive; preparation errors remain attributed by id.
///
/// # Errors
/// Returns cancellation or traversal, candidate, grouping and storage limits.
pub fn scan_indexed_presentation_roots(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    mode: impl Fn(&std::path::Path) -> Presentation,
    policy: PresentationPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<IndexedDirectoryGroups<DirectoryPresentationGroups>, crate::scan::ScanError> {
    scan_presentation_roots_selected(roots, traversal, mode, policy, budget, cancel, true).map(
        |(result, selection)| IndexedDirectoryGroups {
            result,
            candidate_pairs: selection.0,
            signature_preparations: selection.1,
            preparation_issues: selection.2,
        },
    )
}

fn scan_presentation_roots_selected(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    mode: impl Fn(&std::path::Path) -> Presentation,
    policy: PresentationPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    indexed: bool,
) -> Result<(DirectoryPresentationGroups, SelectionStats), crate::scan::ScanError> {
    use crate::scan::ScanError;
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
    for (index, path) in discovery.files.into_iter().enumerate() {
        if cancel() {
            return Err(ScanError::Cancelled);
        }
        let presentation = mode(&path);
        files.push((
            u64::try_from(index).map_err(|_| ScanError::Budget)?,
            path,
            presentation,
        ));
    }
    let selected = scan_presentation_groups_selected(
        files
            .iter()
            .map(|(id, path, mode)| (*id, DecodeRequest::new(path), *mode)),
        policy,
        budget,
        &cancel,
        indexed,
    )?;
    let selection = (
        selected.candidate_pairs,
        selected.signature_preparations,
        selected.preparation_issues,
    );
    Ok((
        DirectoryPresentationGroups {
            files,
            aliases: discovery.aliases,
            traversal_issues: discovery.issues,
            result: selected.result,
        },
        selection,
    ))
}

#[derive(Debug)]
pub struct AutomaticDirectoryGroups {
    pub files: Vec<(u64, std::path::PathBuf, Option<Presentation>)>,
    pub detection_issues: Vec<(u64, ContainerError)>,
    pub aliases: Vec<crate::exact::Aliases>,
    pub traversal_issues: Vec<crate::exact::Issue>,
    pub result: PresentationGroups,
}

/// Recursively classify by content and group complete presentations. Detection
/// failures remain per-id issues; no filename filter or guessed partial mode is
/// substituted. Source observation spans classification and pair processing.
///
/// # Errors
/// Returns traversal/classification/batch cancellation or work/storage limits.
pub fn scan_presentation_roots_auto(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    max_chunks: usize,
    policy: PresentationPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AutomaticDirectoryGroups, crate::scan::ScanError> {
    scan_presentation_roots_auto_selected(roots, traversal, max_chunks, policy, budget, cancel, false)
        .map(|report| report.0)
}

/// Recursively retrieve complete-presentation candidates and directly confirm them.
/// Omitted pairs are inconclusive; preparation errors remain attributed by id.
///
/// # Errors
/// Returns cancellation or traversal, candidate, grouping and storage limits.
pub fn scan_indexed_presentation_roots_auto(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    max_chunks: usize,
    policy: PresentationPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<IndexedDirectoryGroups<AutomaticDirectoryGroups>, crate::scan::ScanError> {
    scan_presentation_roots_auto_selected(roots, traversal, max_chunks, policy, budget, cancel, true).map(
        |(result, selection)| IndexedDirectoryGroups {
            result,
            candidate_pairs: selection.0,
            signature_preparations: selection.1,
            preparation_issues: selection.2,
        },
    )
}

#[allow(clippy::too_many_lines)] // Keep classification snapshots and final regrouping in one shared path.
fn scan_presentation_roots_auto_selected(
    roots: &[std::path::PathBuf],
    traversal: &crate::exact::Options,
    max_chunks: usize,
    policy: PresentationPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
    indexed: bool,
) -> Result<(AutomaticDirectoryGroups, SelectionStats), crate::scan::ScanError> {
    use crate::scan::ScanError;
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
    let mut admitted = Vec::new();
    let mut detection_issues = Vec::new();
    let mut snapshots = Vec::new();
    for (index, path) in discovery.files.into_iter().enumerate() {
        let id = u64::try_from(index).map_err(|_| ScanError::Budget)?;
        let request = DecodeRequest::new(&path);
        let detected = (|| -> Result<_, ContainerError> {
            let snapshot = ContentSnapshot::read(&path, policy.limits.max_file_bytes, &cancel)?;
            let mode = crate::presentation_kind::detect_presentation(
                &request,
                policy.limits,
                max_chunks,
                budget,
                &cancel,
            )?;
            snapshot.verify(&cancel)?;
            Ok((mode, snapshot))
        })();
        match detected {
            Ok((mode, snapshot)) => {
                crate::local::reserve_slot(&mut snapshots, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                crate::local::reserve_slot(&mut admitted, policy.max_files).map_err(|_| ScanError::Budget)?;
                snapshots.push((id, snapshot));
                files.push((id, path, Some(mode)));
                admitted.push((id, request, mode));
            }
            Err(ContainerError::Cancelled | ContainerError::Source(SnapshotError::Cancelled)) => {
                return Err(ScanError::Cancelled);
            }
            Err(error) => {
                files.push((id, path, None));
                crate::local::reserve_slot(&mut detection_issues, policy.max_files)
                    .map_err(|_| ScanError::Budget)?;
                detection_issues.push((id, error));
            }
        }
        if cancel() {
            return Err(ScanError::Cancelled);
        }
    }
    let selected = scan_presentation_groups_selected(admitted, policy, budget, &cancel, indexed)?;
    let selection = (
        selected.candidate_pairs,
        selected.signature_preparations,
        selected.preparation_issues,
    );
    let mut result = selected.result;
    let mut invalid = std::collections::HashSet::new();
    for (id, snapshot) in snapshots {
        match snapshot.verify(&cancel) {
            Ok(()) => {}
            Err(SnapshotError::Cancelled) => return Err(ScanError::Cancelled),
            Err(error) => {
                invalid.try_reserve(1).map_err(|_| ScanError::Budget)?;
                invalid.insert(id);
                if !result.source_issues.iter().any(|(failed, _)| *failed == id) {
                    crate::local::reserve_slot(&mut result.source_issues, policy.max_files)
                        .map_err(|_| ScanError::Budget)?;
                    result.source_issues.push((id, error));
                }
            }
        }
    }
    result.source_issues.sort_by_key(|(id, _)| *id);
    result
        .comparisons
        .equal
        .retain(|(a, b)| !invalid.contains(a) && !invalid.contains(b));
    result
        .comparisons
        .different
        .retain(|(a, b)| !invalid.contains(a) && !invalid.contains(b));
    result.grouping = crate::groups::complete_link_groups(
        files.iter().map(|(id, _, _)| *id),
        result
            .comparisons
            .equal
            .iter()
            .map(|&(left, right)| crate::groups::Pair { left, right }),
        policy.grouping,
        &cancel,
    )?;
    Ok((
        AutomaticDirectoryGroups {
            files,
            detection_issues,
            aliases: discovery.aliases,
            traversal_issues: discovery.issues,
            result,
        },
        selection,
    ))
}

#[cfg(test)]
std::thread_local! { static KEY_TEST_MODE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
std::thread_local! { static PREPARE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
mod reuse_tests {
    use super::*;
    #[test]
    fn retained_left_reuses_pixels_and_rejects_changed_source_and_cancellation() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("left.gif");
        std::fs::write(&path, std::fs::read(root.join("base.gif")).unwrap()).unwrap();
        let left = (
            DecodeRequest::new(&path),
            Presentation::Animation(AnimationKind::Gif),
        );
        let right = (
            DecodeRequest::new(root.join("split.apng")),
            Presentation::Animation(AnimationKind::Apng),
        );
        let limits = AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        };
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let mut retained = None;
        PREPARE_CALLS.with(|count| count.set(0));
        assert!(compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || false).unwrap());
        let used = budget.used();
        assert!(used > 0);
        assert!(compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || false).unwrap());
        PREPARE_CALLS.with(|count| assert_eq!(count.get(), 3));
        assert_eq!(budget.used(), used);
        assert!(matches!(
            compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || true),
            Err(ContainerError::Cancelled)
        ));
        assert!(compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || false).unwrap());
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[0] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        assert!(matches!(
            compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || false),
            Err(ContainerError::Source(SnapshotError::Changed))
        ));
        drop(retained);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn retained_left_survives_memory_refusal_and_releases_before_row_change() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
        let left = (
            DecodeRequest::new(root.join("base.gif")),
            Presentation::Animation(AnimationKind::Gif),
        );
        let right = (
            DecodeRequest::new(root.join("split.apng")),
            Presentation::Animation(AnimationKind::Apng),
        );
        let limits = AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        };
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let mut retained = None;
        assert!(compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || false).unwrap());
        let used = budget.used();
        let occupied = budget.try_reserve(budget.available_bytes()).unwrap();
        let result = compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || false);
        assert!(
            matches!(
                result,
                Err(ContainerError::Animation(AnimationError::Decode(
                    FileError::Decode(rrrah_decode::RasterDecodeError::Source(
                        rrrah_decode::DecodeError::Memory(_)
                    ))
                )))
            ),
            "{result:?}"
        );
        drop(occupied);
        assert_eq!(budget.used(), used);
        assert!(compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || false).unwrap());
        // Changing the left id releases the old decoded animation even if the
        // new pair is incompatible and consequently performs no decode.
        let incompatible = (DecodeRequest::new("missing.png"), Presentation::SelectedFrame);
        assert!(matches!(
            compare_reusing_left(&mut retained, 1, &incompatible, &right, limits, &budget, || false),
            Err(ContainerError::Incompatible)
        ));
        assert!(retained.is_none());
        assert_eq!(budget.used(), 0);
        assert!(compare_reusing_left(&mut retained, 2, &left, &right, limits, &budget, || false).unwrap());
        drop(retained);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn reused_pair_latches_every_cancellation_checkpoint_and_generation_token() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
        let left = (
            DecodeRequest::new(root.join("base.gif")),
            Presentation::Animation(AnimationKind::Gif),
        );
        let mut right = (
            DecodeRequest::new(root.join("split.apng")),
            Presentation::Animation(AnimationKind::Apng),
        );
        let limits = AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        };
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let mut retained = None;
        assert!(compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || false).unwrap());
        let used = budget.used();
        let calls = std::cell::Cell::new(0);
        assert!(
            compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || {
                calls.set(calls.get() + 1);
                false
            })
            .unwrap()
        );
        for checkpoint in 1..=calls.get() {
            let count = std::cell::Cell::new(0);
            assert!(
                matches!(
                    compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || {
                        count.set(count.get() + 1);
                        count.get() == checkpoint
                    }),
                    Err(ContainerError::Cancelled)
                ),
                "checkpoint {checkpoint}"
            );
            assert_eq!(budget.used(), used);
        }
        let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(8));
        right.0.cancellation = Some(rrrah_decode::GenerationToken::new(generation, 7));
        assert!(matches!(
            compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || false),
            Err(ContainerError::Cancelled)
        ));
        right.0.cancellation = None;
        assert!(compare_reusing_left(&mut retained, 0, &left, &right, limits, &budget, || false).unwrap());
        drop(retained);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn candidate_collisions_and_signature_failures_cannot_replace_direct_evidence() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
        let limits = AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        };
        for injected in [0, 1, 2] {
            KEY_TEST_MODE.with(|value| value.set(injected));
            PREPARE_CALLS.with(|value| value.set(0));
            let requests = [
                ("base.gif", AnimationKind::Gif),
                ("split.apng", AnimationKind::Apng),
                ("changed-pixels.apng", AnimationKind::Apng),
                ("zero-prefix.apng", AnimationKind::Apng),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (name, kind))| {
                (
                    u64::try_from(i).unwrap(),
                    DecodeRequest::new(root.join(name)),
                    Presentation::Animation(kind),
                )
            });
            let budget = MemoryBudget::new(4 * 1024 * 1024);
            let report = scan_presentations(requests, 4, 6, limits, &budget, || false).unwrap();
            assert_eq!(report.equal, [(0, 1), (0, 3), (1, 3)]);
            assert_eq!(report.different, [(0, 2), (1, 2), (2, 3)]);
            assert!(report.issues.is_empty());
            let preparations = PREPARE_CALLS.with(std::cell::Cell::get);
            assert_eq!(preparations, if injected == 0 { 9 } else { 13 });
            assert_eq!(budget.used(), 0);
            KEY_TEST_MODE.with(|value| value.set(0));
        }
    }

    #[test]
    fn stale_different_signatures_are_errors_and_equal_signatures_require_confirmation() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("left.gif");
        std::fs::write(&path, std::fs::read(root.join("base.gif")).unwrap()).unwrap();
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let limits = AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        };
        let a = presentation_signature(
            &DecodeRequest::new(&path),
            Presentation::Animation(AnimationKind::Gif),
            limits,
            &budget,
            || false,
        )
        .unwrap();
        let b = presentation_signature(
            &DecodeRequest::new(root.join("changed-pixels.apng")),
            Presentation::Animation(AnimationKind::Apng),
            limits,
            &budget,
            || false,
        )
        .unwrap();
        assert!(signatures_differ(&a, &b, || false).unwrap());
        assert!(!signatures_differ(&a, &a, || false).unwrap());
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[0] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        assert!(matches!(
            signatures_differ(&a, &b, || false),
            Err(ContainerError::Source(SnapshotError::Changed))
        ));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn whole_signature_pipeline_cancels_at_every_checkpoint_without_report_or_leaks() {
        use crate::scan::ScanError;
        use std::cell::Cell;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
        let files = [
            ("base.gif", AnimationKind::Gif),
            ("split.apng", AnimationKind::Apng),
            ("changed-pixels.apng", AnimationKind::Apng),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (name, kind))| {
            (
                u64::try_from(i).unwrap(),
                DecodeRequest::new(root.join(name)),
                Presentation::Animation(kind),
            )
        })
        .collect::<Vec<_>>();
        let limits = AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        };
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        for injected in [0, 1, 2] {
            KEY_TEST_MODE.with(|value| value.set(injected));
            let calls = Cell::new(0);
            let expected = scan_presentations(files.clone(), 3, 3, limits, &budget, || {
                calls.set(calls.get() + 1);
                false
            })
            .unwrap();
            assert_eq!(expected.equal, [(0, 1)]);
            assert_eq!(expected.different, [(0, 2), (1, 2)]);
            assert!(expected.issues.is_empty());
            assert_eq!(budget.used(), 0);
            for checkpoint in 1..=calls.get() {
                let count = Cell::new(0);
                assert!(
                    matches!(
                        scan_presentations(files.clone(), 3, 3, limits, &budget, || {
                            count.set(count.get() + 1);
                            count.get() == checkpoint
                        }),
                        Err(ScanError::Cancelled)
                    ),
                    "mode {injected}, checkpoint {checkpoint}"
                );
                assert_eq!(budget.used(), 0, "mode {injected}, checkpoint {checkpoint}");
            }
            let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(7));
            let mut generation_files = files.clone();
            generation_files[0].1.cancellation =
                Some(rrrah_decode::GenerationToken::new(generation.clone(), 7));
            let generation_calls = Cell::new(0);
            scan_presentations(generation_files.clone(), 3, 3, limits, &budget, || {
                generation_calls.set(generation_calls.get() + 1);
                false
            })
            .unwrap();
            for checkpoint in [
                1,
                10,
                generation_calls.get() / 2,
                generation_calls.get() - 1,
                generation_calls.get(),
            ] {
                generation.store(7, std::sync::atomic::Ordering::Release);
                let count = Cell::new(0);
                assert!(
                    matches!(
                        scan_presentations(generation_files.clone(), 3, 3, limits, &budget, || {
                            count.set(count.get() + 1);
                            if count.get() == checkpoint {
                                generation.store(8, std::sync::atomic::Ordering::Release);
                            }
                            false
                        }),
                        Err(ScanError::Cancelled)
                    ),
                    "mode {injected}, generation checkpoint {checkpoint}"
                );
                assert_eq!(budget.used(), 0);
            }
            generation.store(7, std::sync::atomic::Ordering::Release);
            let retry = scan_presentations(generation_files, 3, 3, limits, &budget, || false).unwrap();
            assert_eq!(retry.equal, expected.equal);
            assert_eq!(retry.different, expected.different);
            assert!(retry.issues.is_empty());
            assert_eq!(budget.used(), 0);
            eprintln!(
                "whole pipeline mode={injected}: caller_checkpoints={} generation_checkpoints=5 retry=true memory_released=true",
                calls.get()
            );
            KEY_TEST_MODE.with(|value| value.set(0));
        }
    }
    #[test]
    fn signature_preparation_errors_remain_pair_issues_without_unique_classification() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
        let folder = tempfile::tempdir().unwrap();
        let corrupt = folder.path().join("bad.gif");
        std::fs::write(&corrupt, b"corrupt GIF container").unwrap();
        let limits = AnimationBudget {
            max_frames: 10,
            max_pixels: 100,
            max_file_bytes: 100_000,
        };
        let files = vec![
            (
                0,
                DecodeRequest::new(root.join("base.gif")),
                Presentation::Animation(AnimationKind::Gif),
            ),
            (
                1,
                DecodeRequest::new(root.join("split.apng")),
                Presentation::Animation(AnimationKind::Apng),
            ),
            (
                2,
                DecodeRequest::new(root.join("zero-prefix.apng")),
                Presentation::Animation(AnimationKind::Apng),
            ),
        ];
        for bytes in [0, 1] {
            let budget = MemoryBudget::new(bytes);
            let report = scan_presentations(files.clone(), 3, 3, limits, &budget, || false).unwrap();
            assert!(report.equal.is_empty() && report.different.is_empty());
            assert_eq!(
                report
                    .issues
                    .iter()
                    .map(|issue| (issue.left, issue.right))
                    .collect::<Vec<_>>(),
                [(0, 1), (0, 2), (1, 2)]
            );
            for issue in &report.issues {
                assert!(
                    matches!(
                        &issue.error,
                        ContainerError::Animation(AnimationError::Memory(_))
                            | ContainerError::Animation(AnimationError::Decode(FileError::Decode(
                                rrrah_decode::RasterDecodeError::Source(rrrah_decode::DecodeError::Memory(_))
                            )))
                    ),
                    "{:?}",
                    issue.error
                );
            }
            assert_eq!(budget.used(), 0);
        }
        let mut corrupt_files = files;
        corrupt_files[2].1 = DecodeRequest::new(corrupt);
        corrupt_files[2].2 = Presentation::Animation(AnimationKind::Gif);
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let report = scan_presentations(corrupt_files, 3, 3, limits, &budget, || false).unwrap();
        assert_eq!(report.equal, [(0, 1)]);
        assert!(report.different.is_empty());
        assert_eq!(
            report
                .issues
                .iter()
                .map(|issue| (issue.left, issue.right))
                .collect::<Vec<_>>(),
            [(0, 2), (1, 2)]
        );
        assert_eq!(budget.used(), 0);
    }
    fn indexed_fixture() -> (Vec<(u64, DecodeRequest, Presentation)>, PresentationPolicy) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/timeline");
        let files = [
            ("base.gif", AnimationKind::Gif),
            ("split.apng", AnimationKind::Apng),
            ("changed-pixels.apng", AnimationKind::Apng),
            ("zero-prefix.apng", AnimationKind::Apng),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (name, kind))| {
            (
                u64::try_from(i).unwrap(),
                DecodeRequest::new(root.join(name)),
                Presentation::Animation(kind),
            )
        })
        .collect();
        let policy = PresentationPolicy {
            max_files: 4,
            max_pairs: 6,
            limits: AnimationBudget {
                max_frames: 10,
                max_pixels: 100,
                max_file_bytes: 100_000,
            },
            grouping: crate::groups::GroupBudget {
                max_entries: 4,
                max_pairs: 6,
                max_pair_checks: 1000,
            },
        };
        (files, policy)
    }
    #[test]
    fn indexed_groups_keep_labels_under_collisions_failure_and_candidate_caps() {
        use crate::scan::ScanError;
        let (files, policy) = indexed_fixture();
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let reference = scan_presentation_groups(files.clone(), policy, &budget, || false).unwrap();
        assert_eq!(reference.grouping.groups, [vec![0, 1, 3], vec![2]]);
        for injected in [0, 1, 2] {
            KEY_TEST_MODE.with(|value| value.set(injected));
            let report = scan_indexed_presentation_groups(files.clone(), policy, &budget, || false).unwrap();
            assert_eq!(report.result.grouping, reference.grouping);
            assert_eq!(report.result.comparisons.equal, [(0, 1), (0, 3), (1, 3)]);
            assert_eq!(report.candidate_pairs, if injected == 0 { 3 } else { 6 });
            assert_eq!(report.signature_preparations, 4);
            assert_eq!(report.preparation_issues.len(), if injected == 2 { 4 } else { 0 });
            assert!(report.result.comparisons.issues.is_empty() && report.result.source_issues.is_empty());
            let limited = PresentationPolicy {
                max_pairs: 3,
                ..policy
            };
            let result = scan_indexed_presentation_groups(files.clone(), limited, &budget, || false);
            if injected == 0 {
                assert_eq!(result.unwrap().result.grouping, reference.grouping);
            } else {
                assert!(matches!(result, Err(ScanError::Budget)));
            }
            assert_eq!(budget.used(), 0);
            KEY_TEST_MODE.with(|value| value.set(0));
        }
        assert!(matches!(
            scan_presentation_groups(
                files,
                PresentationPolicy {
                    max_pairs: 3,
                    ..policy
                },
                &budget,
                || false
            ),
            Err(ScanError::Budget)
        ));
    }
    #[test]
    fn indexed_groups_remove_changed_sources_without_losing_healthy_edges() {
        use std::cell::Cell;
        let (mut files, policy) = indexed_fixture();
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("left.gif");
        std::fs::write(&path, std::fs::read(&files[0].1.path).unwrap()).unwrap();
        files[0].1 = DecodeRequest::new(&path);
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        PREPARE_CALLS.with(|count| count.set(0));
        let mutated = Cell::new(false);
        let report = scan_indexed_presentation_groups(files, policy, &budget, || {
            if !mutated.get() && PREPARE_CALLS.with(std::cell::Cell::get) > 0 {
                let mut bytes = std::fs::read(&path).unwrap();
                bytes[0] ^= 1;
                std::fs::write(&path, bytes).unwrap();
                mutated.set(true);
            }
            false
        })
        .unwrap();
        assert!(mutated.get());
        assert!(report.result.source_issues.iter().any(|(id, _)| *id == 0));
        assert_eq!(report.result.comparisons.equal, [(1, 3)]);
        assert!(report.result.grouping.groups.contains(&vec![1, 3]));
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn indexed_groups_cancel_without_partial_output_at_every_normal_checkpoint() {
        use crate::scan::ScanError;
        use std::cell::Cell;
        let (files, policy) = indexed_fixture();
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let calls = Cell::new(0);
        let reference = scan_indexed_presentation_groups(files.clone(), policy, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        for checkpoint in 1..=calls.get() {
            let count = Cell::new(0);
            assert!(
                matches!(
                    scan_indexed_presentation_groups(files.clone(), policy, &budget, || {
                        count.set(count.get() + 1);
                        count.get() == checkpoint
                    }),
                    Err(ScanError::Cancelled | ScanError::Group(crate::groups::GroupError::Cancelled))
                ),
                "checkpoint {checkpoint}"
            );
            assert_eq!(budget.used(), 0);
        }
        assert_eq!(
            scan_indexed_presentation_groups(files, policy, &budget, || false)
                .unwrap()
                .result
                .grouping,
            reference.result.grouping
        );
        eprintln!(
            "indexed group caller checkpoints={} memory_released=true retry=true",
            calls.get()
        );
    }
    #[test]
    fn indexed_groups_keep_selected_pages_and_animation_scopes_separate() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let files = vec![
            (
                0,
                DecodeRequest::new(root.join("timeline/base.gif")),
                Presentation::Animation(AnimationKind::Gif),
            ),
            (
                1,
                DecodeRequest::new(root.join("timeline/split.apng")),
                Presentation::Animation(AnimationKind::Apng),
            ),
            (
                2,
                DecodeRequest::new(root.join("timeline/base.gif")),
                Presentation::SelectedFrame,
            ),
            (
                3,
                DecodeRequest::new(root.join("timeline/base.gif")),
                Presentation::SelectedFrame,
            ),
            (
                4,
                DecodeRequest::new(root.join("tiff/classic-little-none-same.tif")),
                Presentation::Pages(crate::pages::PageKind::Tiff),
            ),
            (
                5,
                DecodeRequest::new(root.join("tiff/bigtiff-big-lzw-same.tif")),
                Presentation::Pages(crate::pages::PageKind::Tiff),
            ),
        ];
        let policy = PresentationPolicy {
            max_files: 6,
            max_pairs: 3,
            limits: AnimationBudget {
                max_frames: 10,
                max_pixels: 100,
                max_file_bytes: 100_000,
            },
            grouping: crate::groups::GroupBudget {
                max_entries: 6,
                max_pairs: 3,
                max_pair_checks: 1000,
            },
        };
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        for injected in [0, 1] {
            KEY_TEST_MODE.with(|value| value.set(injected));
            let report = scan_indexed_presentation_groups(files.clone(), policy, &budget, || false).unwrap();
            assert_eq!(report.candidate_pairs, 3);
            assert_eq!(
                report.result.grouping.groups,
                [vec![0, 1], vec![2, 3], vec![4, 5]]
            );
            assert_eq!(report.result.comparisons.equal, [(0, 1), (2, 3), (4, 5)]);
            assert!(report.result.comparisons.issues.is_empty() && report.preparation_issues.is_empty());
            assert_eq!(budget.used(), 0);
            KEY_TEST_MODE.with(|value| value.set(0));
        }
    }
}
