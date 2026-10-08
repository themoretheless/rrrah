//! Guarded selected-frame anchor support under caller-supplied geometry.
use crate::{
    anchor_rank::{AnchorRankEvidence, AnchorRankPolicy, compare_anchor_rank},
    animated::AnimationBudget,
    decode::{CachedError, DecodeSourceSnapshot, decode_selected_frame_bounded},
    exact::SnapshotError,
    geometry::{Correspondence, ProjectiveTransform},
    local_rank::LocalRankError,
};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;

/// Cumulative admission for three proposal and original-pixel anchor attempts.
#[derive(Debug, Clone, Copy)]
pub struct AnchorGeometryFallbackPolicy {
    pub search: crate::local_scan::ProjectiveCandidateUnionFallbackFilePolicy,
    pub anchors: AnchorRankPolicy,
    pub maximum_total_point_checks: u64,
    pub maximum_total_selection_checks: u64,
    pub maximum_total_rank_sites: u64,
    pub maximum_total_pixel_reads: u64,
}

#[derive(Debug)]
pub struct AnchorGeometryFallbackEvidence {
    pub evidence: AnchorGeometrySearchEvidence,
    pub smoothing_radii: [u32; 2],
    pub attempted_recipes: u32,
}

pub(crate) fn validate_anchor_geometry_fallback_policy(
    policy: AnchorGeometryFallbackPolicy,
    factors: &[f64],
    cancel: &impl Fn() -> bool,
) -> Result<(), AnchorRankFileError> {
    if cancel() {
        return Err(AnchorRankFileError::Cancelled);
    }
    crate::anchor_rank::validate_anchor_rank_policy(policy.anchors)?;
    crate::local_scan::validate_candidate_union_fallback_file_policy(policy.search, factors, cancel)?;
    // Every attempt enforces these per-phase maxima; admitting all three
    // maxima before IO therefore bounds their cumulative work conservatively.
    for (per_attempt, total) in [
        (
            policy.anchors.local.maximum_point_checks,
            policy.maximum_total_point_checks,
        ),
        (
            policy.anchors.maximum_selection_checks,
            policy.maximum_total_selection_checks,
        ),
        (
            policy.anchors.local.rank.maximum_sites,
            policy.maximum_total_rank_sites,
        ),
        (
            policy.anchors.local.rank.maximum_pixel_reads,
            policy.maximum_total_pixel_reads,
        ),
    ] {
        if per_attempt.checked_mul(3).ok_or(LocalRankError::Budget)? > total {
            return Err(LocalRankError::Budget.into());
        }
    }
    Ok(())
}

/// Try symmetric, source-smoothed and target-smoothed proposals in that order.
/// Original-pixel anchor support selects the first successful attempt. Refusals
/// stop the entire operation; they are never misses. Unsuccessful attempt buffers
/// are released before the next attempt, and source guards span every attempt.
/// Without support, returns the final attempt as explicit negative evidence.
/// Returned local support never establishes whole-image duplicate identity.
pub fn search_anchor_rank_geometry_fallback_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    policy: AnchorGeometryFallbackPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorGeometryFallbackEvidence, AnchorRankFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        validate_anchor_geometry_fallback_policy(policy, factors, &cancelled)?;
        let search = policy.search;
        let decode = search.search.search.gradient.search.local.decode;
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        for (index, radii) in [
            [search.search.smoothing_radius; 2],
            [search.asymmetric_radius, 0],
            [0, search.asymmetric_radius],
        ]
        .into_iter()
        .enumerate()
        {
            let evidence = search_anchor_rank_geometry_files_with_smoothing(
                left,
                right,
                search.search,
                factors,
                radii,
                policy.anchors,
                budget,
                &cancelled,
            )?;
            first.verify(&cancelled)?;
            second.verify(&cancelled)?;
            if index == 2 || evidence.anchors.is_some_and(|a| a.supported) {
                return Ok(AnchorGeometryFallbackEvidence {
                    evidence,
                    smoothing_radii: radii,
                    attempted_recipes: (index + 1) as u32,
                });
            }
            drop(evidence);
        }
        unreachable!("three recipes always produce a terminal outcome")
    })();
    if cancelled() {
        Err(AnchorRankFileError::Cancelled)
    } else {
        result
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AnchorRankFileError {
    #[error("anchor file comparison cancelled")]
    Cancelled,
    #[error(transparent)]
    Source(#[from] SnapshotError),
    #[error(transparent)]
    Decode(#[from] CachedError),
    #[error(transparent)]
    Rank(#[from] LocalRankError),
    #[error(transparent)]
    Search(#[from] crate::local_scan::LocalFileError),
}

/// Snapshots include external decoder dependencies and enclose both decodes and
/// pixel comparison. Cancellation is sticky, including request generation tokens.
/// All supplied points are preserved; evidence is local support, never identity.
#[allow(clippy::too_many_arguments)]
pub fn compare_anchor_rank_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    model: ProjectiveTransform,
    points: &[Correspondence],
    decode: AnimationBudget,
    policy: AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorRankEvidence, AnchorRankFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        if cancelled() {
            return Err(AnchorRankFileError::Cancelled);
        }
        crate::anchor_rank::validate_anchor_rank_policy(policy)?;
        let n = u64::try_from(points.len()).map_err(|_| LocalRankError::Budget)?;
        let pairs = n
            .checked_mul(n.saturating_sub(1))
            .and_then(|v| v.checked_div(2))
            .ok_or(LocalRankError::Budget)?;
        let size = policy
            .window_radius
            .checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .ok_or(LocalRankError::Budget)?;
        let pad = policy
            .local
            .rank
            .radius
            .checked_add(policy.local.filter_radius)
            .ok_or(LocalRankError::Budget)?;
        let extent = u64::from(size)
            .checked_add(2 * u64::from(pad))
            .ok_or(LocalRankError::Budget)?;
        let sites = n
            .checked_mul(2)
            .and_then(|v| v.checked_mul(extent))
            .and_then(|v| v.checked_mul(extent))
            .ok_or(LocalRankError::Budget)?;
        if points.len() > policy.local.maximum_points
            || pairs.checked_add(n).ok_or(LocalRankError::Budget)? > policy.local.maximum_point_checks
            || pairs
                .checked_mul(3)
                .and_then(|v| v.checked_add(n))
                .ok_or(LocalRankError::Budget)?
                > policy.maximum_selection_checks
            || sites > policy.local.rank.maximum_sites
            || sites.checked_mul(5).ok_or(LocalRankError::Budget)? > policy.local.rank.maximum_pixel_reads
        {
            return Err(LocalRankError::Budget.into());
        }
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let a = decode_selected_frame_bounded(left, decode, budget, &cancelled)?;
        let b = decode_selected_frame_bounded(right, decode, budget, &cancelled)?;
        let av = a
            .view(&cancelled)
            .map_err(crate::decode::FileError::from)
            .map_err(CachedError::from)?;
        let bv = b
            .view(&cancelled)
            .map_err(crate::decode::FileError::from)
            .map_err(CachedError::from)?;
        let evidence = compare_anchor_rank(&av, &bv, model, points, policy, budget, &cancelled)?;
        first.verify(&cancelled)?;
        second.verify(&cancelled)?;
        Ok(evidence)
    })();
    if cancelled() {
        Err(AnchorRankFileError::Cancelled)
    } else {
        result
    }
}

#[derive(Debug)]
pub struct AnchorCandidateSearchEvidence {
    pub search: crate::local_scan::ManagedProjectiveCandidateUnionRegionsFileEvidence,
    pub anchors: Option<AnchorRankEvidence>,
}

/// Fresh candidate extraction and geometry followed by original-pixel anchors.
/// Outer source/dependency snapshots enclose both phases; geometry cannot cross
/// source revisions. Every original union correspondence is supplied unchanged.
/// Returned local support is not a whole-file duplicate decision.
#[allow(clippy::too_many_arguments)]
pub fn search_anchor_rank_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    factors: &[f64],
    policy: AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorCandidateSearchEvidence, AnchorRankFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        if cancelled() {
            return Err(AnchorRankFileError::Cancelled);
        }
        crate::anchor_rank::validate_anchor_rank_policy(policy)?;
        let decode = search.search.gradient.search.local.decode;
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let evidence = crate::local_scan::compare_local_files_projective_candidate_union_regions_managed(
            left, right, search, factors, budget, &cancelled,
        )?;
        let anchors = if let Some(geometry) = &evidence.geometry {
            Some(compare_anchor_rank_files(
                left,
                right,
                geometry.transform,
                &evidence.correspondences,
                decode,
                policy,
                budget,
                &cancelled,
            )?)
        } else {
            None
        };
        first.verify(&cancelled)?;
        second.verify(&cancelled)?;
        Ok(AnchorCandidateSearchEvidence {
            search: evidence,
            anchors,
        })
    })();
    if cancelled() {
        Err(AnchorRankFileError::Cancelled)
    } else {
        result
    }
}

#[derive(Debug)]
pub struct AnchorFallbackSearchEvidence {
    pub search: crate::local_scan::ManagedProjectiveCandidateUnionFallbackFileEvidence,
    pub anchors: Option<AnchorRankEvidence>,
}
/// Existing color-confirmed symmetric/asymmetric fallback chooses geometry;
/// original-pixel anchor comparison then evaluates its complete point buffer.
/// Outer snapshots span all candidate attempts and anchor confirmation. Recipe
/// metadata and original color evidence are retained; no identity decision.
#[allow(clippy::too_many_arguments)]
pub fn search_anchor_rank_fallback_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionFallbackFilePolicy,
    factors: &[f64],
    policy: AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorFallbackSearchEvidence, AnchorRankFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        if cancelled() {
            return Err(AnchorRankFileError::Cancelled);
        }
        crate::anchor_rank::validate_anchor_rank_policy(policy)?;
        let decode = search.search.search.gradient.search.local.decode;
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let evidence =
            crate::local_scan::compare_local_files_projective_candidate_union_fallback_regions_managed(
                left, right, search, factors, budget, &cancelled,
            )?;
        let anchors = if let Some(geometry) = &evidence.evidence.geometry {
            Some(compare_anchor_rank_files(
                left,
                right,
                geometry.transform,
                &evidence.evidence.correspondences,
                decode,
                policy,
                budget,
                &cancelled,
            )?)
        } else {
            None
        };
        first.verify(&cancelled)?;
        second.verify(&cancelled)?;
        Ok(AnchorFallbackSearchEvidence {
            search: evidence,
            anchors,
        })
    })();
    if cancelled() {
        Err(AnchorRankFileError::Cancelled)
    } else {
        result
    }
}

#[derive(Debug)]
pub struct AnchorGeometrySearchEvidence {
    pub search: crate::local_scan::ManagedCandidateUnionGeometryFileEvidence,
    pub anchors: Option<AnchorRankEvidence>,
}
/// Same native proposals and geometry, with anchor pixels as the only local
/// confirmation stage. Legacy regional color fitting is not run or reported.
/// Outer source/dependency guards span both phases; this remains local support.
#[allow(clippy::too_many_arguments)]
pub fn search_anchor_rank_geometry_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    factors: &[f64],
    policy: AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorGeometrySearchEvidence, AnchorRankFileError> {
    search_anchor_rank_geometry_files_with_smoothing(
        left,
        right,
        search,
        factors,
        [search.smoothing_radius; 2],
        policy,
        budget,
        cancel,
    )
}

/// Explicit ordered candidate smoothing; original-pixel anchor policy is unchanged.
/// Source guards span proposal and confirmation. No whole-image identity is implied.
#[allow(clippy::too_many_arguments)]
pub fn search_anchor_rank_geometry_files_with_smoothing(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    factors: &[f64],
    smoothing_radii: [u32; 2],
    policy: AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorGeometrySearchEvidence, AnchorRankFileError> {
    search_anchor_rank_geometry_files_selected(left, right, search, factors,
        smoothing_radii, policy, None, budget, cancel)
}

/// Automatic reflected-source proposals and original-pixel anchor confirmation.
/// Shared source/dependency snapshots and sticky cancellation span all phases.
/// Reflection work is admitted before IO; typed refusals never become misses.
/// This explicit recipe proves finite local support, not whole-image identity.
#[allow(clippy::too_many_arguments)]
pub fn search_anchor_rank_reflected_geometry_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    factors: &[f64],
    smoothing_radii: [u32; 2],
    policy: AnchorRankPolicy,
    maximum_reflection_bins: u64,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorGeometrySearchEvidence, AnchorRankFileError> {
    search_anchor_rank_geometry_files_selected(left, right, search, factors,
        smoothing_radii, policy, Some(maximum_reflection_bins), budget, cancel)
}

#[allow(clippy::too_many_arguments)]
fn search_anchor_rank_geometry_files_selected(
    left: &DecodeRequest,
    right: &DecodeRequest,
    search: crate::local_scan::ProjectiveCandidateUnionRegionsFilePolicy,
    factors: &[f64],
    smoothing_radii: [u32; 2],
    policy: AnchorRankPolicy,
    reflection_bins: Option<u64>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorGeometrySearchEvidence, AnchorRankFileError> {
    let latch = std::cell::Cell::new(false);
    let cancelled = || {
        let value = latch.get()
            || cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            });
        latch.set(value);
        value
    };
    let result = (|| {
        if cancelled() {
            return Err(AnchorRankFileError::Cancelled);
        }
        crate::anchor_rank::validate_anchor_rank_policy(policy)?;
        if let Some(limit) = reflection_bins {
            crate::local_scan::validate_candidate_union_regions_file_policy(search, factors, &cancelled)?;
            crate::local_scan::validate_candidate_reflection_bins(search, limit)?;
        }
        let decode = search.search.gradient.search.local.decode;
        let first = DecodeSourceSnapshot::read(&left.path, decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, decode.max_file_bytes, &cancelled)?;
        let evidence = if let Some(limit) = reflection_bins {
            crate::local_scan::find_projective_candidate_union_reflected_geometry_files_managed(
                left, right, search, factors, smoothing_radii, limit, budget, &cancelled,
            )?
        } else {
            crate::local_scan::find_projective_candidate_union_geometry_files_managed(
                left, right, search, factors, smoothing_radii, budget, &cancelled,
            )?
        };
        let anchors = if let Some(geometry) = &evidence.geometry {
            Some(compare_anchor_rank_files(
                left,
                right,
                geometry.transform,
                &evidence.correspondences,
                decode,
                policy,
                budget,
                &cancelled,
            )?)
        } else {
            None
        };
        first.verify(&cancelled)?;
        second.verify(&cancelled)?;
        Ok(AnchorGeometrySearchEvidence {
            search: evidence,
            anchors,
        })
    })();
    if cancelled() {
        Err(AnchorRankFileError::Cancelled)
    } else {
        result
    }
}
