//! Fixed whole-image/quadrant proposal search with original-file confirmation.
use crate::{
    anchor_rank_file::AnchorRankFileError,
    decode::DecodeSourceSnapshot,
    exact::{ContentSnapshot, SnapshotError},
    local_rank::LocalRankError,
    model_compare::{
        JointModelComparisonPolicy, ModelComparisonError, ModelComparisonEvidence, ModelComparisonPolicy,
        ProposalProviderError, compare_prepared_files, validate_model_comparison_policy,
    },
    model_input::GrayModelInput,
    model_input_file::{PreparedModelFile, prepare_file},
    proposals::ScoredProposal,
};
use rrrah_core::{BufferError, MemoryBudget, SharedBuffer};
use rrrah_decode::{DecodeRequest, GenerationToken};
use std::{cell::Cell, path::Path};

pub const MAX_MODEL_TILE_ATTEMPTS: usize = 25;

/// Prepaid maxima across all 25 possible attempts. Source/decode, output-count
/// and other constituent limits also remain enforced on every attempt.
/// These are work counts, not libtorch tensor/workspace or total RSS admission.
#[derive(Debug, Clone, Copy)]
pub struct ModelTileWorkLimits {
    pub model_input_reads: u64,
    pub proposal_ranking_checks: u64,
    pub proposal_separation_checks: u64,
    pub geometry_hypotheses: u64,
    pub point_checks: u64,
    pub selection_checks: u64,
    pub rank_sites: u64,
    pub pixel_reads: u64,
}
impl ModelTileWorkLimits {
    /// Compute checked work maxima for this fixed search, without relaxing any
    /// constituent policy. Invalid policies and arithmetic overflow refuse.
    ///
    /// # Errors
    /// Invalid comparison policy or cumulative work overflow.
    pub fn for_model(policy: ModelComparisonPolicy) -> Result<Self, ModelTileError> {
        validate_model_comparison_policy(policy)?;
        let times = |n: u64| n.checked_mul(25).ok_or(ModelTileError::Work);
        Ok(Self {
            model_input_reads: times(
                policy
                    .input
                    .max_pixel_reads
                    .checked_mul(2)
                    .ok_or(ModelTileError::Work)?,
            )?,
            proposal_ranking_checks: times(policy.proposals.max_ranking_checks)?,
            proposal_separation_checks: times(policy.proposals.max_separation_checks)?,
            geometry_hypotheses: times(policy.sampling.trials)?,
            point_checks: times(policy.anchors.local.maximum_point_checks)?,
            selection_checks: times(policy.anchors.maximum_selection_checks)?,
            rank_sites: times(policy.anchors.local.rank.maximum_sites)?,
            pixel_reads: times(policy.anchors.local.rank.maximum_pixel_reads)?,
        })
    }
}

/// An attempted candidate is either fully evaluated or explicitly inconclusive.
/// Missing anchor geometry is never represented as an ordinary pixel miss.
#[derive(Debug, Clone, Copy)]
pub enum ModelTileOutcome {
    Comparison(ModelComparisonEvidence),
    InsufficientAnchorGeometry,
}
impl ModelTileOutcome {
    pub fn comparison(&self) -> Option<ModelComparisonEvidence> {
        match self {
            Self::Comparison(e) => Some(*e),
            Self::InsufficientAnchorGeometry => None,
        }
    }
    pub fn supported(&self) -> bool {
        self.comparison().is_some_and(|e| e.supported())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModelTileAttempt {
    pub left_rectangle: [u32; 4],
    pub right_rectangle: [u32; 4],
    pub evidence: ModelTileOutcome,
}
#[derive(Debug, Clone)]
pub struct ModelTileEvidence {
    attempts: SharedBuffer<ModelTileAttempt>,
    len: usize,
    model_digest: [u8; 32],
}
impl ModelTileEvidence {
    /// Every attempted pair of rectangles, including misses, in fixed order.
    pub fn attempts(&self) -> &[ModelTileAttempt] {
        &self.attempts[..self.len]
    }
    pub fn terminal(&self) -> &ModelTileAttempt {
        &self.attempts[self.len - 1]
    }
    pub fn model_digest(&self) -> [u8; 32] {
        self.model_digest
    }
    pub fn insufficient_geometry_attempts(&self) -> usize {
        self.attempts()
            .iter()
            .filter(|a| matches!(a.evidence, ModelTileOutcome::InsufficientAnchorGeometry))
            .count()
    }
    pub fn supported(&self) -> bool {
        self.terminal().evidence.supported()
    }
}
#[derive(Debug, thiserror::Error)]
pub enum ModelTileError {
    #[error("model tile search exhausted with insufficient anchor geometry")]
    InsufficientAnchorGeometry(ModelTileEvidence),
    #[error("model tile search cancelled")]
    Cancelled,
    #[error("model tile search cumulative work refusal")]
    Work,
    #[error(transparent)]
    Memory(#[from] BufferError),
    #[error(transparent)]
    Source(#[from] SnapshotError),
    #[error(transparent)]
    Comparison(#[from] ModelComparisonError),
}

pub(crate) fn validate_model_tile_work(
    policy: ModelComparisonPolicy,
    limits: ModelTileWorkLimits,
) -> Result<(), ModelTileError> {
    let r = ModelTileWorkLimits::for_model(policy)?;
    if [
        (r.model_input_reads, limits.model_input_reads),
        (r.proposal_ranking_checks, limits.proposal_ranking_checks),
        (r.proposal_separation_checks, limits.proposal_separation_checks),
        (r.geometry_hypotheses, limits.geometry_hypotheses),
        (r.point_checks, limits.point_checks),
        (r.selection_checks, limits.selection_checks),
        (r.rank_sites, limits.rank_sites),
        (r.pixel_reads, limits.pixel_reads),
    ]
    .iter()
    .any(|(required, limit)| required > limit)
    {
        return Err(ModelTileError::Work);
    }
    Ok(())
}

pub(crate) fn regions([width, height]: [u32; 2]) -> ([[u32; 4]; 5], usize) {
    let full = [0, 0, width, height];
    let mut rectangles = [full; 5];
    let mut len = 1;
    // Division-based endpoints avoid multiplication overflow for u32::MAX.
    let xs = [0, width / 2, width];
    let ys = [0, height / 2, height];
    for y in 0..2 {
        for x in 0..2 {
            let r = [xs[x], ys[y], xs[x + 1] - xs[x], ys[y + 1] - ys[y]];
            if r[2] != 0 && r[3] != 0 && r != full {
                rectangles[len] = r;
                len += 1;
            }
        }
    }
    (rectangles, len)
}
pub(crate) fn owners_cancelled(left: &DecodeRequest, right: &DecodeRequest, latch: &Cell<bool>) -> bool {
    let stopped = latch.get()
        || [left, right].iter().any(|request| {
            request
                .cancellation
                .as_ref()
                .is_some_and(GenerationToken::is_cancelled)
        });
    latch.set(stopped);
    stopped
}
type Provider<'a> = dyn FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>
    + 'a;

/// Try the full images first, then every remaining Cartesian pair of full image
/// and nonempty 2x2 quadrants, in source-major row order. There are at most 25
/// attempts; tiny degenerate partitions omit empty/duplicate-full rectangles.
/// Stop only at original-pixel support, otherwise exhaust the configured recipe.
/// Scores or a fitted transform alone never stop search. No candidate pruning,
/// manual region selection, CLAHE, crop files or transitive duplicate grouping.
///
/// Complete work and managed attempt storage are admitted before source I/O.
/// Each side/rectangle is prepared lazily at most once. Up to ten managed
/// candidate inputs are retained only during this operation; their source and
/// original request observations are rechecked on cache hits. Their allocations
/// use the same budget, and any late admission failure discards all attempts.
/// Every original file, external decode dependency, supplied weights and both
/// request generations are guarded across the entire sequence. Typed failures
/// abort it rather than becoming misses. A final caller checkpoint precedes a
/// read-only revalidation pass without user callbacks; this is not file locking
/// or single-kernel preemption. Retained attempt credit follows shared ownership.
///
/// # Errors
/// Policy/work/memory refusal, changed source/weights, constituent comparison
/// failure or sticky cancellation. No partial evidence is published on error.
#[allow(clippy::too_many_arguments)]
pub fn search_model_tiles_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: ModelComparisonPolicy,
    work: ModelTileWorkLimits,
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<ModelTileEvidence, ModelTileError> {
    search(
        left,
        right,
        weights,
        policy,
        work,
        budget,
        &mut provider,
        &cancel,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn search(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: ModelComparisonPolicy,
    work: ModelTileWorkLimits,
    budget: &MemoryBudget,
    provider: &mut Provider<'_>,
    cancel: &dyn Fn() -> bool,
    joint: Option<(u64, u64)>,
) -> Result<ModelTileEvidence, ModelTileError> {
    let latch = Cell::new(false);
    let cancelled = || {
        let stopped = latch.get() || cancel() || owners_cancelled(left, right, &latch);
        latch.set(stopped);
        stopped
    };
    let result = (|| {
        if cancelled() {
            return Err(ModelTileError::Cancelled);
        }
        validate_model_tile_work(policy, work)?;
        if let Some((per_attempt, total)) = joint {
            let source_tolerance2 = policy.anchors.local.source_tolerance.powi(2);
            if !source_tolerance2.is_finite() || source_tolerance2 <= 0. {
                return Err(ModelComparisonError::Policy.into());
            }
            let required = u64::try_from(policy.geometry.max_points)
                .ok()
                .and_then(|n| n.checked_mul(2))
                .and_then(|n| {
                    policy
                        .sampling
                        .trials
                        .checked_add(1)
                        .and_then(|t| n.checked_mul(t))
                })
                .ok_or(ModelTileError::Work)?;
            if required > per_attempt
                || per_attempt
                    .checked_mul(MAX_MODEL_TILE_ATTEMPTS as u64)
                    .is_none_or(|all| all > total)
            {
                return Err(ModelTileError::Work);
            }
        }
        let empty = ModelTileAttempt {
            left_rectangle: [0; 4],
            right_rectangle: [0; 4],
            evidence: ModelTileOutcome::Comparison(ModelComparisonEvidence {
                original_dimensions: [[0; 2]; 2],
                model_digest: [0; 32],
                raw_proposals: 0,
                selected_points: 0,
                transform: None,
                geometry_inliers: 0,
                hypotheses: 0,
                pixels: None,
            }),
        };
        let mut attempts = budget.try_buffer(MAX_MODEL_TILE_ATTEMPTS, empty)?;
        let first = DecodeSourceSnapshot::read(&left.path, policy.decode.max_file_bytes, &cancelled)?;
        let second = DecodeSourceSnapshot::read(&right.path, policy.decode.max_file_bytes, &cancelled)?;
        let model = ContentSnapshot::read(weights, policy.max_model_bytes, &cancelled)?;
        let verify = || -> Result<(), ModelTileError> {
            first.verify(&cancelled)?;
            second.verify(&cancelled)?;
            model.verify(&cancelled)?;
            Ok(())
        };
        verify()?;
        let mut left_inputs: [Option<PreparedModelFile>; 5] = std::array::from_fn(|_| None);
        let mut right_inputs: [Option<PreparedModelFile>; 5] = std::array::from_fn(|_| None);
        left_inputs[0] = Some(
            prepare_file(left, None, policy.decode, policy.input, budget, &cancelled)
                .map_err(ModelComparisonError::Left)?,
        );
        right_inputs[0] = Some(
            prepare_file(right, None, policy.decode, policy.input, budget, &cancelled)
                .map_err(ModelComparisonError::Right)?,
        );
        let whole_result = compare_prepared_files(
            left,
            right,
            weights,
            left_inputs[0].as_ref().ok_or(ModelTileError::Work)?,
            right_inputs[0].as_ref().ok_or(ModelTileError::Work)?,
            &model,
            policy,
            budget,
            provider,
            &cancelled,
            joint.map(|j| j.0),
        );
        let whole = classify_attempt(
            whole_result,
            left_inputs[0].as_ref().ok_or(ModelTileError::Work)?,
            right_inputs[0].as_ref().ok_or(ModelTileError::Work)?,
            &model,
            &cancelled,
        )?;
        verify()?;
        let ld = left_inputs[0]
            .as_ref()
            .ok_or(ModelTileError::Work)?
            .original_dimensions();
        let rd = right_inputs[0]
            .as_ref()
            .ok_or(ModelTileError::Work)?
            .original_dimensions();
        let (lr, ln) = regions(ld);
        let (rr, rn) = regions(rd);
        attempts[0] = ModelTileAttempt {
            left_rectangle: lr[0],
            right_rectangle: rr[0],
            evidence: whole,
        };
        let mut len = 1;
        if !whole.supported() {
            'pairs: for (i, l) in lr[..ln].iter().enumerate() {
                for (j, r) in rr[..rn].iter().enumerate() {
                    if i == 0 && j == 0 {
                        continue;
                    }
                    verify()?;
                    if left_inputs[i].is_none() {
                        left_inputs[i] = Some(
                            prepare_file(left, Some(*l), policy.decode, policy.input, budget, &cancelled)
                                .map_err(ModelComparisonError::Left)?,
                        );
                    }
                    if right_inputs[j].is_none() {
                        right_inputs[j] = Some(
                            prepare_file(right, Some(*r), policy.decode, policy.input, budget, &cancelled)
                                .map_err(ModelComparisonError::Right)?,
                        );
                    }
                    let comparison = compare_prepared_files(
                        left,
                        right,
                        weights,
                        left_inputs[i].as_ref().ok_or(ModelTileError::Work)?,
                        right_inputs[j].as_ref().ok_or(ModelTileError::Work)?,
                        &model,
                        policy,
                        budget,
                        provider,
                        &cancelled,
                        joint.map(|j| j.0),
                    );
                    let evidence = classify_attempt(
                        comparison,
                        left_inputs[i].as_ref().ok_or(ModelTileError::Work)?,
                        right_inputs[j].as_ref().ok_or(ModelTileError::Work)?,
                        &model,
                        &cancelled,
                    )?;
                    verify()?;
                    let supported = evidence.supported();
                    attempts[len] = ModelTileAttempt {
                        left_rectangle: *l,
                        right_rectangle: *r,
                        evidence,
                    };
                    len += 1;
                    if supported {
                        break 'pairs;
                    }
                }
            }
        }
        Ok((
            ModelTileEvidence {
                attempts: attempts.freeze(),
                len,
                model_digest: model.digest(),
            },
            first,
            second,
            model,
        ))
    })();
    if cancelled() {
        return Err(ModelTileError::Cancelled);
    }
    let (evidence, first, second, model) = result?;
    let owners = || owners_cancelled(left, right, &latch);
    let verified: Result<_, ModelTileError> = (|| {
        first.verify(&owners)?;
        second.verify(&owners)?;
        model.verify(&owners)?;
        Ok(evidence)
    })();
    if owners() {
        Err(ModelTileError::Cancelled)
    } else {
        let evidence = verified?;
        if !evidence.supported() && evidence.insufficient_geometry_attempts() > 0 {
            Err(ModelTileError::InsufficientAnchorGeometry(evidence))
        } else {
            Ok(evidence)
        }
    }
}

fn classify_attempt(
    result: Result<ModelComparisonEvidence, ModelComparisonError>,
    left: &PreparedModelFile,
    right: &PreparedModelFile,
    model: &ContentSnapshot,
    cancel: &dyn Fn() -> bool,
) -> Result<ModelTileOutcome, ModelTileError> {
    match result {
        Ok(e) => Ok(ModelTileOutcome::Comparison(e)),
        Err(ModelComparisonError::Pixels(AnchorRankFileError::Rank(
            LocalRankError::InsufficientGeometry,
        ))) => {
            // A failed confirmation must not bypass observation validation.
            // No other error class is recoverable here.
            left.verify_source(cancel).map_err(ModelComparisonError::Left)?;
            right.verify_source(cancel).map_err(ModelComparisonError::Right)?;
            model.verify(cancel)?;
            Ok(ModelTileOutcome::InsufficientAnchorGeometry)
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    #[test]
    fn odd_tiny_and_maximum_domains_partition_without_empty_or_duplicate_rectangles() {
        for dimensions in [[17, 11], [1, 1], [1, 7], [8, 1], [u32::MAX, u32::MAX]] {
            let (r, n) = regions(dimensions);
            assert!(n <= 5);
            assert_eq!(r[0], [0, 0, dimensions[0], dimensions[1]]);
            for (i, p) in r[..n].iter().enumerate() {
                assert!(p[2] > 0 && p[3] > 0);
                assert!(p[0].checked_add(p[2]).unwrap() <= dimensions[0]);
                assert!(p[1].checked_add(p[3]).unwrap() <= dimensions[1]);
                assert!(!r[..i].contains(p));
            }
            if n > 1 {
                assert_eq!(
                    r[1..n]
                        .iter()
                        .map(|p| u64::from(p[2]) * u64::from(p[3]))
                        .sum::<u64>(),
                    u64::from(dimensions[0]) * u64::from(dimensions[1])
                );
            }
        }
    }
    #[test]
    fn observed_final_request_cancellation_is_sticky_after_counter_reset() {
        for owner in 0..2 {
            let generation = Arc::new(AtomicU64::new(1));
            let mut requests = [DecodeRequest::new("unused"), DecodeRequest::new("unused")];
            requests[owner].cancellation = Some(GenerationToken::new(generation.clone(), 1));
            let latch = Cell::new(false);
            assert!(!owners_cancelled(&requests[0], &requests[1], &latch));
            generation.store(2, Ordering::Release);
            assert!(owners_cancelled(&requests[0], &requests[1], &latch));
            generation.store(1, Ordering::Release);
            assert!(owners_cancelled(&requests[0], &requests[1], &latch));
        }
    }
}

/// Search all fixed model windows with explicit joint geometry. Both per-window
/// and cumulative25-window projection ceilings are admitted before source I/O.
/// Original pixel/source/cancellation guards and typed inconclusive outcomes
/// match the legacy tile lifecycle. No automatic smoothing choice is made.
#[allow(clippy::too_many_arguments)]
pub fn search_joint_model_tiles_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: JointModelComparisonPolicy,
    maximum_total_projection_checks: u64,
    work: ModelTileWorkLimits,
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<ModelTileEvidence, ModelTileError> {
    search(
        left,
        right,
        weights,
        policy.comparison,
        work,
        budget,
        &mut provider,
        &cancel,
        Some((policy.maximum_projection_checks, maximum_total_projection_checks)),
    )
}
