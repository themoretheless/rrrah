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
