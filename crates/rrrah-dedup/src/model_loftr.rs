//! Local-weight CPU LoFTR through the guarded model-comparison lifecycle.
//!
//! Requires the `loftr` Cargo feature and libtorch 2.10 (tch 0.23). No weights
//! are downloaded. Each public operation starts with no model and loads the
//! supplied weights only when inference is needed. Multi-attempt operations
//! reuse that model within their guarded source/weight lifetime, never globally.
//! Tensor/workspace allocations remain outside the managed payload budget.
//! Cancellation is cooperative; a running libtorch kernel is not preempted.
use crate::{
    model_compare::{
        JointModelComparisonPolicy, ModelComparisonError, ModelComparisonEvidence, ModelComparisonPolicy,
        ProposalProviderError, compare_model_files_with, compare_model_joint_files_with,
        compare_model_joint_regions_files_with, compare_model_regions_files_with,
    },
    model_input::GrayModelInput,
    proposals::ScoredProposal,
};
use loftr::{LoftrConfig, LoftrError, LoftrMatches, LoftrModel};
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::DecodeRequest;
use std::path::Path;
use tch::{Device, Kind, Tensor};

/// Extract full-coordinate CPU LoFTR correspondences for explicit rectangles.
/// The generic extractor guards files and weights throughout this call and
/// admits input/proposal work before inference. Returned points retain managed
/// credit until their last owner drops. They are proposals, not copy evidence.
/// Combining multiple calls requires an outer source/weights guard and separate
/// geometry/original-pixel confirmation. Torch allocations/RSS remain external.
///
/// # Errors
/// Invalid regions/policy, resource refusal, source/model/provider failure or
/// cancellation abort extraction without publishing partial proposals.
#[allow(clippy::too_many_arguments)]
pub fn extract_loftr_region_points_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    left_rectangle: [u32; 4],
    right_rectangle: [u32; 4],
    policy: ModelComparisonPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::model_compare::ModelPointBank, ModelComparisonError> {
    let cap = policy.proposals.max_proposals;
    crate::model_compare::extract_model_region_points_files_with(
        left,
        right,
        weights,
        left_rectangle,
        right_rectangle,
        policy,
        budget,
        |weights, left, right, budget, cancel| proposals(weights, left, right, cap, budget, cancel),
        cancel,
    )
}

/// Compare selected file frames using local outdoor LoFTR weights, native
/// geometry and fresh original-pixel confirmation. Scores alone never support
/// a match. Policy preflight, source/model snapshots and sticky global/both
/// request cancellation are provided by `compare_model_files_with`.
///
/// Does not modify global libtorch thread/seed settings. Callers own runtime
/// configuration. Managed credit covers prepared pixels and proposals, not
/// the model, libtorch scratch or total process RSS.
///
/// # Errors
/// Returns typed policy, source, model/provider, geometry, pixel or cancellation
/// errors. Missing/malformed weights and budget refusals never become misses.
pub fn compare_loftr_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: ModelComparisonPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ModelComparisonEvidence, ModelComparisonError> {
    let cap = policy.proposals.max_proposals;
    compare_model_files_with(
        left,
        right,
        weights,
        policy,
        budget,
        |weights, left, right, budget, cancel| proposals(weights, left, right, cap, budget, cancel),
        cancel,
    )
}

/// Compare explicit projected HDR/alpha inputs with local CPU LoFTR and verify
/// the retained original linear RGBA grids in both directions. Scores and
/// projected-byte equality never suffice. The caller supplies pixel tolerance,
/// coverage and agreement thresholds; this does not establish file identity.
///
/// # Errors
/// Policy, source/weights mutation, provider/geometry/pixel failure, work/memory
/// refusal or cooperative cancellation. Torch tensor/workspace RSS is external.
pub fn compare_loftr_projected_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::model_projected_compare::ProjectedModelPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::model_projected_compare::ProjectedModelEvidence,
    crate::model_projected_compare::ProjectedModelError,
> {
    let cap = policy.proposals.max_proposals;
    crate::model_projected_compare::compare_projected_model_files_with(
        left,
        right,
        weights,
        policy,
        budget,
        |weights, left, right, budget, cancel| proposals(weights, left, right, cap, budget, cancel),
        cancel,
    )
}

/// Automatically search projected HDR/alpha full-image/quadrant pairs with
/// operation-scoped local CPU LoFTR reuse and full-original RGBA confirmation.
/// Cumulative work and payload limits are checked before source I/O. Scores and
/// projected equality never establish support; no model is cached globally.
/// # Errors
/// Policy/work/memory, source/model/provider/geometry/pixel errors or cooperative
/// cancellation. Model tensors/workspaces and total RSS remain external.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_projected_tiles_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::model_projected_compare::ProjectedModelPolicy,
    work: crate::model_projected_tiles::ProjectedTileWorkLimits,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::model_projected_tiles::ProjectedTileEvidence,
    crate::model_projected_tiles::ProjectedTileError,
> {
    let cap = policy.proposals.max_proposals;
    let mut model = None;
    crate::model_projected_tiles::search_projected_model_tiles_files_with(
        left,
        right,
        weights,
        policy,
        work,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Compare explicit original-image crops with local outdoor LoFTR, then fit and
/// confirm the resulting correspondences on the original complete files.
/// No cropped files, hidden thresholds or global runtime changes are introduced.
/// Full source/dependency/weight guards and sticky request cancellation apply.
///
/// # Errors
/// Invalid crop/policy, source/model changes, provider/geometry/pixel failure,
/// resource refusal or cancellation. Tensor/RSS admission remains external.
#[allow(clippy::too_many_arguments)]
pub fn compare_loftr_regions_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    left_rectangle: [u32; 4],
    right_rectangle: [u32; 4],
    policy: ModelComparisonPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ModelComparisonEvidence, ModelComparisonError> {
    let cap = policy.proposals.max_proposals;
    compare_model_regions_files_with(
        left,
        right,
        weights,
        left_rectangle,
        right_rectangle,
        policy,
        budget,
        |weights, left, right, budget, cancel| proposals(weights, left, right, cap, budget, cancel),
        cancel,
    )
}

/// Automatically search full-image/quadrant combinations with the same local
/// CPU LoFTR provider and original-pixel confirmation. No manual crop selection
/// is needed. Full cumulative work and source/request guards are enforced by
/// the fixed tile search; total inference RSS remains outside managed credit.
///
/// # Errors
/// Policy/work/memory refusal, source/model/provider/geometry/pixel failure or
/// cancellation aborts the sequence without returning partial attempts.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_model_tiles_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: ModelComparisonPolicy,
    work: crate::model_tiles::ModelTileWorkLimits,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::model_tiles::ModelTileEvidence, crate::model_tiles::ModelTileError> {
    let cap = policy.proposals.max_proposals;
    let mut model = None;
    crate::model_tiles::search_model_tiles_files_with(
        left,
        right,
        weights,
        policy,
        work,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Refusals while adopting a single-pair model result.
#[derive(Debug, thiserror::Error)]
pub enum LoftrProposalError {
    #[error("LoFTR returned {count} proposals, limit {limit}")]
    Count { count: usize, limit: usize },
    #[error("LoFTR result has inconsistent tensor shapes")]
    Shape,
    #[error("LoFTR single-pair result contains batch {batch}")]
    Batch { batch: i64 },
}

fn tensor(input: &GrayModelInput) -> Result<Tensor, tch::TchError> {
    Tensor::f_from_slice(&input.pixels)?
        .f_to_kind(Kind::Float)?
        .f_div_scalar(255.)?
        .f_reshape([
            1,
            1,
            i64::from(input.dimensions[1]),
            i64::from(input.dimensions[0]),
        ])
}

fn checkpoint(cancel: &dyn Fn() -> bool) -> Result<(), LoftrError> {
    if cancel() {
        Err(LoftrError::Cancelled)
    } else {
        Ok(())
    }
}

pub(crate) fn proposals(
    weights: &Path,
    left: &GrayModelInput,
    right: &GrayModelInput,
    cap: usize,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
    proposals_with_model(&mut None, weights, left, right, cap, budget, cancel)
}

// Private, operation-scoped reuse only. Every caller fixes one weights path and
// its outer lifecycle checks the complete weight snapshot before/after attempts.
// A new public call owns a new empty slot, and errors discard the whole operation.
#[allow(clippy::too_many_arguments)]
fn proposals_with_model(
    slot: &mut Option<LoftrModel>,
    weights: &Path,
    left: &GrayModelInput,
    right: &GrayModelInput,
    cap: usize,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
    checkpoint(cancel)?;
    if slot.is_none() {
        let mut model = LoftrModel::new(Device::Cpu, LoftrConfig::outdoor())?;
        checkpoint(cancel)?;
        model.load_weights(weights)?;
        checkpoint(cancel)?;
        *slot = Some(model);
    }
    checkpoint(cancel)?;
    let model = slot.as_mut().ok_or("missing initialized LoFTR model")?;
    let _inference = tch::no_grad_guard();
    let raw = model.forward_cancellable(&tensor(left)?, &tensor(right)?, cancel)?;
    checkpoint(cancel)?;
    adopt(raw, cap, budget, cancel)
}

// Preserve the qualified provider's Vec<f32> conversion semantics even when
// libtorch returns double coordinates. Read scalars without scratch vectors.
fn scalar_f32(tensor: &Tensor, index: &[i64]) -> Result<f64, tch::TchError> {
    Ok(f64::from(tensor.f_double_value(index)? as f32))
}

fn adopt(
    raw: LoftrMatches,
    cap: usize,
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
    checkpoint(cancel)?;
    let count = raw.confidence.numel();
    let rows = i64::try_from(count)?;
    if count > cap {
        return Err(LoftrProposalError::Count { count, limit: cap }.into());
    }
    if raw.keypoints0.size() != [rows, 2]
        || raw.keypoints1.size() != [rows, 2]
        || raw.confidence.size() != [rows]
        || raw.batch_indexes.size() != [rows]
    {
        return Err(LoftrProposalError::Shape.into());
    }
    let mut output = budget.try_buffer(
        count,
        ScoredProposal {
            source: [0.; 2],
            target: [0.; 2],
            confidence: 0.,
        },
    )?;
    for (index, point) in output.iter_mut().enumerate() {
        checkpoint(cancel)?;
        let row = i64::try_from(index)?;
        let batch = raw.batch_indexes.f_int64_value(&[row])?;
        if batch != 0 {
            return Err(LoftrProposalError::Batch { batch }.into());
        }
        *point = ScoredProposal {
            source: [
                scalar_f32(&raw.keypoints0, &[row, 0])?,
                scalar_f32(&raw.keypoints0, &[row, 1])?,
            ],
            target: [
                scalar_f32(&raw.keypoints1, &[row, 0])?,
                scalar_f32(&raw.keypoints1, &[row, 1])?,
            ],
            confidence: scalar_f32(&raw.confidence, &[row])?,
        };
    }
    checkpoint(cancel)?;
    Ok(output.freeze())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn raw() -> Result<LoftrMatches, tch::TchError> {
        Ok(LoftrMatches {
            keypoints0: Tensor::f_from_slice(&[1_f32, 2., 3., 4.])?.f_reshape([2, 2])?,
            keypoints1: Tensor::f_from_slice(&[5_f32, 6., 7., 8.])?.f_reshape([2, 2])?,
            confidence: Tensor::f_from_slice(&[0.75_f32, 0.5])?,
            batch_indexes: Tensor::f_from_slice(&[0_i64, 0])?,
        })
    }

    #[test]
    fn double_results_preserve_qualified_float32_conversion() -> Result<(), ProposalProviderError> {
        let mut result = raw()?;
        result.keypoints0 = Tensor::f_from_slice(&[0.1_f64, 0.2, 0.3, 0.4])?.f_reshape([2, 2])?;
        result.confidence = Tensor::f_from_slice(&[0.7_f64, 0.8])?;
        let expected = Vec::<Vec<f32>>::try_from(&result.keypoints0)?;
        let budget = MemoryBudget::new(1024);
        let points = adopt(result, 2, &budget, &|| false)?;
        for (point, row) in points.iter().zip(expected) {
            assert_eq!(point.source, [f64::from(row[0]), f64::from(row[1])]);
        }
        assert_eq!(points[0].confidence, f64::from(0.7_f32));
        drop(points);
        assert_eq!(budget.used(), 0);
        Ok(())
    }

    #[test]
    fn adoption_retains_credit_until_last_owner() -> Result<(), ProposalProviderError> {
        let bytes = u64::try_from(2 * std::mem::size_of::<ScoredProposal>())?;
        let budget = MemoryBudget::new(bytes);
        let points = adopt(raw()?, 2, &budget, &|| false)?;
        assert_eq!(points[0].source, [1., 2.]);
        assert_eq!(points[1].target, [7., 8.]);
        assert_eq!(points[0].confidence, 0.75);
        let owner = points.clone();
        drop(points);
        assert_eq!(budget.used(), bytes);
        drop(owner);
        assert_eq!(budget.used(), 0);
        let insufficient = MemoryBudget::new(bytes - 1);
        assert!(adopt(raw()?, 2, &insufficient, &|| false).is_err());
        assert_eq!(insufficient.used(), 0);
        Ok(())
    }

    #[test]
    fn malformed_results_are_errors_and_release_credit() -> Result<(), ProposalProviderError> {
        let budget = MemoryBudget::new(1024);
        let error = adopt(raw()?, 1, &budget, &|| false)
            .err()
            .ok_or("expected cap refusal")?;
        assert!(matches!(
            error.downcast_ref::<LoftrProposalError>(),
            Some(LoftrProposalError::Count { count: 2, limit: 1 })
        ));
        let mut malformed = raw()?;
        malformed.keypoints1 = malformed.keypoints1.f_reshape([4])?;
        let error = adopt(malformed, 2, &budget, &|| false)
            .err()
            .ok_or("expected shape refusal")?;
        assert!(matches!(
            error.downcast_ref::<LoftrProposalError>(),
            Some(LoftrProposalError::Shape)
        ));
        let mut malformed = raw()?;
        malformed.batch_indexes = Tensor::f_from_slice(&[0_i64, 1])?;
        let error = adopt(malformed, 2, &budget, &|| false)
            .err()
            .ok_or("expected batch refusal")?;
        assert!(matches!(
            error.downcast_ref::<LoftrProposalError>(),
            Some(LoftrProposalError::Batch { batch: 1 })
        ));
        assert_eq!(budget.used(), 0);
        Ok(())
    }

    #[test]
    fn every_adoption_checkpoint_cancels_without_publication() -> Result<(), ProposalProviderError> {
        // Entry, each of two points, and final publication.
        for stop in 1..=4 {
            let budget = MemoryBudget::new(1024);
            let calls = Cell::new(0);
            let error = adopt(raw()?, 2, &budget, &|| {
                calls.set(calls.get() + 1);
                calls.get() == stop
            })
            .err()
            .ok_or("expected cancellation")?;
            assert!(matches!(
                error.downcast_ref::<LoftrError>(),
                Some(LoftrError::Cancelled)
            ));
            assert_eq!(calls.get(), stop);
            assert_eq!(budget.used(), 0);
        }
        Ok(())
    }
}

/// Run automatic native/LoFTR search in the explicitly configured order.
/// Uses the same local-weight CPU provider and resource limitations as
/// [`compare_loftr_files`]. See the portfolio API for cumulative admission,
/// source guards and the distinction between local support and identity.
///
/// # Errors
/// Policy, cumulative work, source/model/provider, native search or cancellation
/// failures stop the search rather than being treated as a miss.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_portfolio_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::search_portfolio::SearchPortfolioPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::search_portfolio::SearchPortfolioEvidence, crate::search_portfolio::SearchPortfolioError> {
    let cap = policy.model.proposals.max_proposals;
    let mut model = None;
    crate::search_portfolio::search_portfolio_files_with(
        left,
        right,
        weights,
        policy,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Examine every unordered collection pair with guarded native/LoFTR search.
/// See [`crate::portfolio_collection::scan_portfolio_collection_with`] for
/// exhaustive enumeration, cumulative work admission and batch publication.
/// The same local-weight CPU runtime and tensor/RSS limitations apply as in
/// [`compare_loftr_files`]. No model download or global runtime change occurs.
///
/// # Errors
/// File/pair/work/memory or constituent failures, changed sources/weights and
/// cancellation discard the entire batch, including earlier positive pairs.
#[allow(clippy::too_many_arguments)]
pub fn scan_loftr_portfolio_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: crate::portfolio_collection::PortfolioCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::portfolio_collection::PortfolioCollectionReport,
    crate::portfolio_collection::PortfolioCollectionError,
> {
    let cap = policy.search.model.proposals.max_proposals;
    let mut model = None;
    crate::portfolio_collection::scan_portfolio_collection_with(
        files,
        weights,
        policy,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Exhaustive native/LoFTR collection with explicit insufficient-geometry
/// outcomes and one operation-local lazy CPU model. See
/// [`crate::portfolio_collection::scan_portfolio_outcome_collection_with`].
/// Provider tensors and workspaces remain outside managed memory admission.
/// # Errors
/// Fatal policy/source/model/provider/cancellation failures discard the report.
#[allow(clippy::too_many_arguments)]
pub fn scan_loftr_portfolio_outcome_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: crate::portfolio_collection::PortfolioCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::portfolio_collection::PortfolioOutcomeCollectionReport,
    crate::portfolio_collection::PortfolioCollectionError,
> {
    let cap = policy.search.model.proposals.max_proposals;
    let mut model = None;
    crate::portfolio_collection::scan_portfolio_outcome_collection_with(
        files,
        weights,
        policy,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// CPU LoFTR with proposal-first exhaustive fallback and explicit unknowns.
/// One operation-local lazy model is reused across all scheduled pairs.
/// Source/final guards and complete-work admission remain active; tensors and
/// provider workspace remain outside managed memory admission.
/// # Errors
/// Schedule mismatch, policy/source/model/provider failures and cancellation
/// discard the full report. No automatic weight download occurs.
#[allow(clippy::too_many_arguments)]
pub fn scan_loftr_portfolio_scheduled_outcome_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: &crate::pair_schedule::CompletePairSchedule,
    weights: &Path,
    policy: crate::portfolio_collection::PortfolioCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::portfolio_collection::PortfolioOutcomeCollectionReport,
    crate::portfolio_collection::PortfolioCollectionError,
> {
    let cap = policy.search.model.proposals.max_proposals;
    let mut model = None;
    crate::portfolio_collection::scan_portfolio_scheduled_outcome_collection_with(
        files,
        schedule,
        weights,
        policy,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Exhaustive HDR/alpha collection search with one operation-local CPU LoFTR
/// model reused across regions and pairs. All collection lifecycle and admission
/// rules are provided by `scan_projected_model_collection_with`. No global
/// thread/seed settings change; Torch tensors/workspaces are outside managed RSS.
/// # Errors
/// Typed collection policy, source/weights, provider, geometry, pixel, memory or
/// cancellation errors; no partial report or automatic model download.
pub fn scan_loftr_projected_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: crate::projected_collection::ProjectedCollectionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::projected_collection::ProjectedCollectionReport,
    crate::projected_collection::ProjectedCollectionError,
> {
    let cap = policy.model.proposals.max_proposals;
    let mut model = None;
    crate::projected_collection::scan_projected_model_collection_with(
        files,
        weights,
        policy,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Guarded native/projected pair search with one lazily loaded local CPU LoFTR
/// model. See the generic portfolio API for typed inconclusive semantics.
/// # Errors
/// Constituent, source/weights, work/memory or cancellation failures; no download
/// or global thread/seed changes. Torch workspace/RSS is outside managed credit.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_projected_portfolio_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::projected_portfolio::ProjectedPortfolioPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::projected_portfolio::ProjectedPortfolioEvidence,
    crate::projected_portfolio::ProjectedPortfolioError,
> {
    let cap = policy.model.proposals.max_proposals;
    let mut model = None;
    crate::projected_portfolio::search_projected_portfolio_files_with(
        left,
        right,
        weights,
        policy,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}
/// File-byte confirmation followed by native/projected search with one lazily
/// loaded local CPU LoFTR model. Exact copies do not load or read model weights.
/// The returned enum distinguishes file bytes from regional visual evidence.
/// # Errors
/// See the exact-first generic API. No downloads or global runtime changes;
/// Torch tensor/workspace RSS remains outside managed memory admission.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_exact_projected_portfolio_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::projected_portfolio::ProjectedPortfolioPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::exact_portfolio::ExactPortfolioEvidence, crate::projected_portfolio::ProjectedPortfolioError>
{
    let cap = policy.model.proposals.max_proposals;
    let mut model = None;
    crate::exact_portfolio::search_exact_projected_portfolio_files_with(
        left,
        right,
        weights,
        policy,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}
/// Complete native/projected collection enumeration with explicit unknowns and
/// one local CPU LoFTR model reused across regions and pairs. No global runtime
/// setting changes; provider tensor/workspace RSS remains outside managed credit.
/// # Errors
/// Typed collection failures; native inconclusive outcomes are retained distinctly
/// in complete reports. Mutations/cancellation never become unknown records.
pub fn scan_loftr_projected_portfolio_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: crate::projected_portfolio_collection::ProjectedPortfolioCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::projected_portfolio_collection::ProjectedPortfolioCollectionReport,
    crate::projected_portfolio_collection::ProjectedPortfolioCollectionError,
> {
    let cap = policy.search.model.proposals.max_proposals;
    let mut model = None;
    crate::projected_portfolio_collection::scan_projected_portfolio_collection_with(
        files,
        weights,
        policy,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Exact-byte confirmation plus complete native/projected collection search with
/// one local CPU LoFTR model reused across regions and pairs. No global runtime
/// setting changes; provider tensor/workspace RSS remains outside managed credit.
/// # Errors
/// Exact copies need no model weights; byte-different pairs activate complete
/// visual guards. Typed native inconclusive outcomes are retained distinctly
/// in complete reports. Mutations/cancellation never become unknown records.
pub fn scan_loftr_exact_projected_portfolio_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: crate::exact_portfolio_collection::ExactPortfolioCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::exact_portfolio_collection::ExactPortfolioCollectionReport,
    crate::exact_portfolio_collection::ExactPortfolioCollectionError,
> {
    let cap = policy.search.model.proposals.max_proposals;
    let mut model = None;
    crate::exact_portfolio_collection::scan_exact_projected_portfolio_collection_with(
        files,
        weights,
        policy,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Primary CPU LoFTR/native search followed by opposite-native recovery.
/// See [`crate::search_portfolio::search_recovery_files_with`] for admission and
/// guarded publication. Local weights are required; tensor/workspace allocations
/// remain outside managed memory. The operation loads one model lazily.
/// # Errors
/// Typed policy/work/source/model/provider/pixel failures and cancellation.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_recovery_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::search_portfolio::SearchRecoveryPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::search_portfolio::SearchRecoveryOutcome, crate::search_portfolio::SearchPortfolioError> {
    let cap = policy.primary.model.proposals.max_proposals;
    let mut model = None;
    crate::search_portfolio::search_recovery_files_with(
        left,
        right,
        weights,
        policy,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Exhaustive collection recovery using one lazily loaded CPU LoFTR model.
/// A complete optional schedule prioritizes proposals and retains all fallback
/// pairs. See [`crate::portfolio_collection::scan_recovery_collection_with`].
/// Provider tensors/workspaces and total process RSS are outside managed credit.
/// # Errors
/// Collection/pair policy, schedule, work/memory, source/weights/provider failures
/// and cancellation discard the entire report.
#[allow(clippy::too_many_arguments)]
pub fn scan_loftr_recovery_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: Option<&crate::pair_schedule::CompletePairSchedule>,
    weights: &Path,
    policy: crate::portfolio_collection::RecoveryCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::portfolio_collection::RecoveryCollectionReport,
    crate::portfolio_collection::PortfolioCollectionError,
> {
    let cap = policy.search.primary.model.proposals.max_proposals;
    let mut model = None;
    crate::portfolio_collection::scan_recovery_collection_with(
        files,
        schedule,
        weights,
        policy,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Primary CPU LoFTR/native search followed by joint opposite-native recovery.
/// See [`crate::search_portfolio::search_joint_recovery_files_with`] for admission and
/// guarded publication. Local weights are required; tensor/workspace allocations
/// remain outside managed memory. The operation loads one model lazily.
/// # Errors
/// Typed policy/work/source/model/provider/pixel failures and cancellation.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_joint_recovery_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::search_portfolio::SearchRecoveryPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    maximum_projection_checks: u64,
    cancel: impl Fn() -> bool,
) -> Result<crate::search_portfolio::SearchRecoveryOutcome, crate::search_portfolio::SearchPortfolioError> {
    let cap = policy.primary.model.proposals.max_proposals;
    let mut model = None;
    crate::search_portfolio::search_joint_recovery_files_with(
        left,
        right,
        weights,
        policy,
        factors,
        budget,
        maximum_projection_checks,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Exhaustive collection recovery using one lazily loaded CPU LoFTR model.
/// A complete optional schedule prioritizes proposals and retains all fallback
/// pairs. See [`crate::portfolio_collection::scan_joint_recovery_collection_with`].
/// Provider tensors/workspaces and total process RSS are outside managed credit.
/// # Errors
/// Collection/pair policy, schedule, work/memory, source/weights/provider failures
/// and cancellation discard the entire report.
#[allow(clippy::too_many_arguments)]
pub fn scan_loftr_joint_recovery_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: Option<&crate::pair_schedule::CompletePairSchedule>,
    weights: &Path,
    policy: crate::portfolio_collection::RecoveryCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    maximum_pair_projection_checks: u64,
    maximum_total_projection_checks: u64,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::portfolio_collection::RecoveryCollectionReport,
    crate::portfolio_collection::PortfolioCollectionError,
> {
    let cap = policy.search.primary.model.proposals.max_proposals;
    let mut model = None;
    crate::portfolio_collection::scan_joint_recovery_collection_with(
        files,
        schedule,
        weights,
        policy,
        factors,
        budget,
        maximum_pair_projection_checks,
        maximum_total_projection_checks,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Local CPU LoFTR with explicit prepaid bidirectional geometry.
/// Original pixels and lifecycle guards remain mandatory; Torch RSS is external.
pub fn compare_loftr_joint_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: JointModelComparisonPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ModelComparisonEvidence, ModelComparisonError> {
    let cap = policy.comparison.proposals.max_proposals;
    compare_model_joint_files_with(
        left,
        right,
        weights,
        policy,
        budget,
        |weights, left, right, budget, cancel| proposals(weights, left, right, cap, budget, cancel),
        cancel,
    )
}

/// Local CPU LoFTR with explicit prepaid bidirectional geometry.
/// Original pixels and lifecycle guards remain mandatory; Torch RSS is external.
pub fn compare_loftr_joint_regions_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    left_rectangle: [u32; 4],
    right_rectangle: [u32; 4],
    policy: JointModelComparisonPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<ModelComparisonEvidence, ModelComparisonError> {
    let cap = policy.comparison.proposals.max_proposals;
    compare_model_joint_regions_files_with(
        left,
        right,
        weights,
        left_rectangle,
        right_rectangle,
        policy,
        budget,
        |weights, left, right, budget, cancel| proposals(weights, left, right, cap, budget, cancel),
        cancel,
    )
}

/// CPU joint tile search with one operation-scoped lazy model and explicit
///25-window projection admission. Tensor/workspace RSS remains external.
pub fn search_loftr_joint_model_tiles_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: JointModelComparisonPolicy,
    maximum_total_projection_checks: u64,
    work: crate::model_tiles::ModelTileWorkLimits,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::model_tiles::ModelTileEvidence, crate::model_tiles::ModelTileError> {
    let cap = policy.comparison.proposals.max_proposals;
    let mut model = None;
    crate::model_tiles::search_joint_model_tiles_files_with(
        left,
        right,
        weights,
        policy,
        maximum_total_projection_checks,
        work,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Joint model geometry in guarded native/model portfolio, with one local CPU
/// model per operation and explicit projection preflight; no default changes.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_joint_model_portfolio_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::search_portfolio::SearchPortfolioPolicy,
    maximum_model_projection_checks: u64,
    maximum_total_model_projection_checks: u64,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::search_portfolio::SearchPortfolioEvidence, crate::search_portfolio::SearchPortfolioError> {
    let cap = policy.model.proposals.max_proposals;
    let mut model = None;
    crate::search_portfolio::search_joint_model_portfolio_files_with(
        left,
        right,
        weights,
        policy,
        maximum_model_projection_checks,
        maximum_total_model_projection_checks,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Exhaustive collection with explicit joint model geometry and one lazy CPU
/// model across all pairs. Projection ceilings admit windows/pairs before input
/// iteration. Tensor workspaces remain outside managed payload admission.
#[allow(clippy::too_many_arguments)]
pub fn scan_loftr_joint_model_portfolio_outcome_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: crate::portfolio_collection::PortfolioCollectionPolicy,
    maximum_model_projection_checks: u64,
    maximum_pair_projection_checks: u64,
    maximum_total_projection_checks: u64,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::portfolio_collection::PortfolioOutcomeCollectionReport,
    crate::portfolio_collection::PortfolioCollectionError,
> {
    let cap = policy.search.model.proposals.max_proposals;
    let mut model = None;
    crate::portfolio_collection::scan_joint_model_portfolio_outcome_collection_with(
        files,
        weights,
        policy,
        maximum_model_projection_checks,
        maximum_pair_projection_checks,
        maximum_total_projection_checks,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Whole/quadrant point-bank recovery with one lazily loaded CPU LoFTR model.
/// Complete file/model observations and original-pixel confirmation span all
/// windows. Torch tensors/workspaces remain outside managed payload admission.
/// # Errors
/// Policy/work/memory/source/provider failures or cancellation discard evidence.
pub fn search_loftr_model_banks_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::model_banks::ModelBankSearchPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::model_banks::ModelBankSearchEvidence, crate::model_tiles::ModelTileError> {
    let cap = policy.comparison.proposals.max_proposals;
    let mut model = None;
    crate::model_banks::search_model_banks_files_with(
        left,
        right,
        weights,
        policy,
        budget,
        &mut |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Cached CPU LoFTR with bounded caller-ordered pixel coverage fallback.
/// Inference/geometry are shared across radii; original thresholds and guards
/// remain mandatory. Tensor/workspace RSS is outside managed payload credit.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_model_banks_coverage_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::model_banks::ModelBankSearchPolicy,
    radii: &[u32],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::model_banks::ModelBankSearchEvidence, crate::model_tiles::ModelTileError> {
    let cap = policy.comparison.proposals.max_proposals;
    let mut model = None;
    crate::model_banks::search_model_banks_coverage_files_with(
        left,
        right,
        weights,
        policy,
        radii,
        budget,
        &mut |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Cached CPU LoFTR with explicit original-pixel window/rank contexts.
/// The shared extraction loads one model; generic preflight, source guards and
/// cancellation cover every context. Tensor/workspace RSS remains external.
///
/// # Errors
/// Invalid recipes, cumulative work/memory refusal, source/model/provider/pixel
/// failure or cancellation abort the operation without a partial result.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_model_banks_pixel_coverage_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::model_banks::ModelBankSearchPolicy,
    recipes: &[crate::model_banks::ModelBankPixelRecipe],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::model_banks::ModelBankSearchEvidence, crate::model_tiles::ModelTileError> {
    let cap = policy.comparison.proposals.max_proposals;
    let mut model = None;
    crate::model_banks::search_model_banks_pixel_coverage_files_with(
        left,
        right,
        weights,
        policy,
        recipes,
        budget,
        &mut |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Explicit nine-overlapping-region calibrated recovery with one cached CPU
/// model load. Whole-operation guards and cumulative admission live in the
/// generic entry point. Torch allocations remain outside managed admission.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_model_banks_overlap_exposure_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::model_banks::ModelBankSearchPolicy,
    recipes: &[crate::model_banks::ModelBankPixelRecipe],
    maximum_calibration_pixel_reads: u64,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::model_banks::ExposureModelBankSearchEvidence, crate::model_tiles::ModelTileError> {
    let cap = policy.comparison.proposals.max_proposals;
    let mut model = None;
    crate::model_banks::search_model_banks_overlap_exposure_files_with(
        left,
        right,
        weights,
        policy,
        recipes,
        maximum_calibration_pixel_reads,
        budget,
        &mut |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Ordinary bank evidence first, explicit exposure recovery only after a miss.
/// Both stages must declare the same proposal cap. One cached CPU model spans
/// both stages; generic whole-operation guards include
/// source/dependency and weights even when recovery is skipped. Tensor/workspace
/// allocations remain outside managed payload admission.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_model_banks_exposure_recovery_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::model_banks::ModelBankExposureRecoveryPolicy,
    primary_recipes: &[crate::model_banks::ModelBankPixelRecipe],
    recovery_recipes: &[crate::model_banks::ModelBankPixelRecipe],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::model_banks::ModelBankExposureRecoveryEvidence, crate::model_tiles::ModelTileError> {
    let cap = policy.primary.comparison.proposals.max_proposals;
    if cap != policy.recovery.comparison.proposals.max_proposals {
        if cancel()
            || [left, right].iter().any(|r| {
                r.cancellation
                    .as_ref()
                    .is_some_and(rrrah_decode::GenerationToken::is_cancelled)
            })
        {
            return Err(crate::model_tiles::ModelTileError::Cancelled);
        }
        return Err(crate::model_compare::ModelComparisonError::Policy.into());
    }
    let mut model = None;
    crate::model_banks::search_model_banks_exposure_recovery_files_with(
        left,
        right,
        weights,
        policy,
        primary_recipes,
        recovery_recipes,
        budget,
        &mut |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Native/learned portfolio with cached CPU LoFTR bank aggregation and coverage.
/// Whole operation guards and original-pixel confirmation span both stages.
/// Torch tensors/workspaces remain outside managed payload admission.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_model_bank_portfolio_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::search_portfolio::SearchPortfolioPolicy,
    banks: crate::joint_witnesses::JointBankPolicy,
    work: crate::model_tiles::ModelTileWorkLimits,
    radii: &[u32],
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::search_portfolio::SearchPortfolioEvidence, crate::search_portfolio::SearchPortfolioError> {
    let cap = policy.model.proposals.max_proposals;
    let mut model = None;
    crate::search_portfolio::search_model_bank_portfolio_files_with(
        left,
        right,
        weights,
        policy,
        banks,
        work,
        radii,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Exhaustive or complete scheduled bank collection with one cached CPU model.
/// Unknown outcomes and atomic collection guards are preserved. Tensor/workspace
/// allocations remain outside managed payload admission.
#[allow(clippy::too_many_arguments)]
pub fn scan_loftr_model_bank_portfolio_outcome_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: Option<&crate::pair_schedule::CompletePairSchedule>,
    weights: &Path,
    policy: crate::portfolio_collection::PortfolioCollectionPolicy,
    banks: crate::joint_witnesses::JointBankPolicy,
    work: crate::model_tiles::ModelTileWorkLimits,
    radii: &[u32],
    total: crate::portfolio_collection::ModelBankCollectionWorkLimits,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::portfolio_collection::PortfolioOutcomeCollectionReport,
    crate::portfolio_collection::PortfolioCollectionError,
> {
    let cap = policy.search.model.proposals.max_proposals;
    let mut model = None;
    crate::portfolio_collection::scan_model_bank_portfolio_outcome_collection_with(
        files,
        schedule,
        weights,
        policy,
        banks,
        work,
        radii,
        total,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Cached CPU bank coverage plus joint opposite recovery under shared guards.
/// Tensor/workspace allocations remain outside managed payload admission.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_model_bank_joint_recovery_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::search_portfolio::SearchRecoveryPolicy,
    banks: crate::joint_witnesses::JointBankPolicy,
    work: crate::model_tiles::ModelTileWorkLimits,
    radii: &[u32],
    maximum_projection_checks: u64,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::search_portfolio::SearchRecoveryOutcome, crate::search_portfolio::SearchPortfolioError> {
    let cap = policy.primary.model.proposals.max_proposals;
    let mut model = None;
    crate::search_portfolio::search_model_bank_joint_recovery_files_with(
        left,
        right,
        weights,
        policy,
        banks,
        work,
        radii,
        maximum_projection_checks,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Bank primary and joint opposite recovery for every collection pair, sharing
/// one lazily loaded CPU model. Collection guards and typed unknowns are retained.
/// Torch tensors/workspaces remain outside managed payload admission.
#[allow(clippy::too_many_arguments)]
pub fn scan_loftr_model_bank_joint_recovery_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: Option<&crate::pair_schedule::CompletePairSchedule>,
    weights: &Path,
    policy: crate::portfolio_collection::RecoveryCollectionPolicy,
    banks: crate::joint_witnesses::JointBankPolicy,
    work: crate::model_tiles::ModelTileWorkLimits,
    radii: &[u32],
    total: crate::portfolio_collection::ModelBankCollectionWorkLimits,
    maximum_pair_projection_checks: u64,
    maximum_total_projection_checks: u64,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::portfolio_collection::RecoveryCollectionReport,
    crate::portfolio_collection::PortfolioCollectionError,
> {
    let cap = policy.search.primary.model.proposals.max_proposals;
    let mut model = None;
    crate::portfolio_collection::scan_model_bank_joint_recovery_collection_with(
        files,
        schedule,
        weights,
        policy,
        banks,
        work,
        radii,
        total,
        maximum_pair_projection_checks,
        maximum_total_projection_checks,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Fixed native/opposite/bank cascade; the cached CPU model is created only if
/// both native stages miss. Full original-source/model guards remain mandatory.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_native_bank_recovery_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: crate::search_portfolio::SearchRecoveryPolicy,
    banks: crate::joint_witnesses::JointBankPolicy,
    work: crate::model_tiles::ModelTileWorkLimits,
    radii: &[u32],
    maximum_projection_checks: u64,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::search_portfolio::NativeBankRecoveryEvidence, crate::search_portfolio::SearchPortfolioError>
{
    let cap = policy.primary.model.proposals.max_proposals;
    let mut model = None;
    crate::search_portfolio::search_native_bank_recovery_files_with(
        left,
        right,
        weights,
        policy,
        banks,
        work,
        radii,
        maximum_projection_checks,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Complete native-first collection with one lazily cached CPU model.
#[allow(clippy::too_many_arguments)]
pub fn scan_loftr_native_bank_recovery_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: Option<&crate::pair_schedule::CompletePairSchedule>,
    weights: &Path,
    policy: crate::portfolio_collection::RecoveryCollectionPolicy,
    banks: crate::joint_witnesses::JointBankPolicy,
    work: crate::model_tiles::ModelTileWorkLimits,
    radii: &[u32],
    total: crate::portfolio_collection::ModelBankCollectionWorkLimits,
    maximum_pair_projection_checks: u64,
    maximum_total_projection_checks: u64,
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::collection::CollectionReport<crate::search_portfolio::NativeBankRecoveryEvidence>,
    crate::portfolio_collection::PortfolioCollectionError,
> {
    let cap = policy.search.primary.model.proposals.max_proposals;
    let mut model = None;
    crate::portfolio_collection::scan_native_bank_recovery_collection_with(
        files,
        schedule,
        weights,
        policy,
        banks,
        work,
        radii,
        total,
        maximum_pair_projection_checks,
        maximum_total_projection_checks,
        factors,
        budget,
        |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Explicit exhaustive/scheduled ordinary-bank then exposure-recovery collection.
/// One cached CPU model spans every pair and both stages. Equal proposal caps are
/// required; generic batch guards and prepaid work remain authoritative. Tensor
/// allocations and provider workspace are outside managed payload admission.
#[allow(clippy::too_many_arguments)]
pub fn scan_loftr_model_banks_exposure_recovery_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: Option<&crate::pair_schedule::CompletePairSchedule>,
    weights: &Path,
    policy: crate::model_banks::ModelBankExposureCollectionPolicy,
    primary_recipes: &[crate::model_banks::ModelBankPixelRecipe],
    recovery_recipes: &[crate::model_banks::ModelBankPixelRecipe],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::model_banks::ModelBankExposureCollectionReport,
    crate::model_banks::ModelBankExposureCollectionError,
> {
    let cap = policy.search.primary.comparison.proposals.max_proposals;
    if cap != policy.search.recovery.comparison.proposals.max_proposals {
        return Err(if cancel() {
            crate::collection::CollectionError::Cancelled
        } else {
            crate::collection::CollectionError::Policy(
                crate::model_compare::ModelComparisonError::Policy.into(),
            )
        });
    }
    let mut model = None;
    crate::model_banks::scan_model_banks_exposure_recovery_collection_with(
        files,
        schedule,
        weights,
        policy,
        primary_recipes,
        recovery_recipes,
        budget,
        &mut |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Bidirectional collection with one lazy CPU model shared across both directions.
#[allow(clippy::too_many_arguments)]
pub fn scan_loftr_model_banks_bidirectional_exposure_recovery_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: Option<&crate::pair_schedule::CompletePairSchedule>,
    weights: &Path,
    policy: crate::model_banks::ModelBankExposureCollectionPolicy,
    primary_recipes: &[crate::model_banks::ModelBankPixelRecipe],
    recovery_recipes: &[crate::model_banks::ModelBankPixelRecipe],
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::model_banks::BidirectionalExposureCollectionReport,
    crate::model_banks::ModelBankExposureCollectionError,
> {
    let cap = policy.search.primary.comparison.proposals.max_proposals;
    if cap != policy.search.recovery.comparison.proposals.max_proposals {
        return Err(if cancel() {
            crate::collection::CollectionError::Cancelled
        } else {
            crate::collection::CollectionError::Policy(
                crate::model_compare::ModelComparisonError::Policy.into(),
            )
        });
    }
    let mut model = None;
    crate::model_banks::scan_model_banks_bidirectional_exposure_recovery_collection_with(
        files,
        schedule,
        weights,
        policy,
        primary_recipes,
        recovery_recipes,
        budget,
        &mut |weights, left, right, budget, cancel| {
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Explicit two-direction confirmation with one operation-local lazy CPU model.
/// Proposal caps are selected per direction; native support can skip inference.
/// Original-source/dependency/weights guards span both directions. Tensor/RSS
/// allocations and kernel preemption remain outside managed payload admission.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_native_bank_consensus_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    forward: crate::search_portfolio::NativeBankDirectionPolicy<'_>,
    reverse: crate::search_portfolio::NativeBankDirectionPolicy<'_>,
    work: crate::search_portfolio::PortfolioWorkLimits,
    maximum_projection_checks: u64,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<crate::search_portfolio::NativeBankConsensusEvidence, crate::search_portfolio::SearchPortfolioError>
{
    search_loftr_native_bank_consensus_files_with_trace(
        left,
        right,
        weights,
        forward,
        reverse,
        work,
        maximum_projection_checks,
        budget,
        cancel,
    )
    .map(|trace| trace.summary())
}

/// Two-direction CPU confirmation retaining compact diagnostics for unknowns.
/// Uses the same guarded lifetime, lazy model and directional proposal caps as
/// the summary API. Diagnostics retain no correspondence or decoded buffers.
/// Tensor/RSS allocations remain outside managed payload admission.
#[allow(clippy::too_many_arguments)]
pub fn search_loftr_native_bank_consensus_files_with_trace(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    forward: crate::search_portfolio::NativeBankDirectionPolicy<'_>,
    reverse: crate::search_portfolio::NativeBankDirectionPolicy<'_>,
    work: crate::search_portfolio::PortfolioWorkLimits,
    maximum_projection_checks: u64,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::search_portfolio::NativeBankConsensusTraceEvidence,
    crate::search_portfolio::SearchPortfolioError,
> {
    let mut model = None;
    crate::search_portfolio::search_native_bank_consensus_files_directional_traced(
        left,
        right,
        weights,
        forward,
        reverse,
        work,
        maximum_projection_checks,
        budget,
        |is_forward, weights, left, right, budget, cancel| {
            let cap = if is_forward { forward } else { reverse }
                .search
                .primary
                .model
                .proposals
                .max_proposals;
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}

/// Exhaustive or completely scheduled two-direction collection confirmation.
/// One lazy CPU model is shared across all pairs and directions; each direction
/// retains its own proposal cap. Batch source/weights/generation guards and
/// cumulative admission remain authoritative. Tensor allocations are not a
/// hard process RSS bound. Outcomes establish local support, not file identity.
///
/// # Errors
/// Policy/work refusal, source/model/provider failure or cancellation discards
/// the report under the generic collection's publication rules.
#[allow(clippy::too_many_arguments)]
pub fn scan_loftr_native_bank_consensus_collection(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: Option<&crate::pair_schedule::CompletePairSchedule>,
    weights: &Path,
    policy: crate::portfolio_collection::NativeBankConsensusCollectionPolicy<'_>,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<
    crate::collection::CollectionReport<crate::search_portfolio::NativeBankConsensusEvidence>,
    crate::portfolio_collection::PortfolioCollectionError,
> {
    let mut model = None;
    crate::portfolio_collection::scan_native_bank_consensus_collection_directional(
        files,
        schedule,
        weights,
        policy,
        budget,
        |is_forward, weights, left, right, budget, cancel| {
            let cap = if is_forward {
                policy.forward
            } else {
                policy.reverse
            }
            .search
            .primary
            .model
            .proposals
            .max_proposals;
            proposals_with_model(&mut model, weights, left, right, cap, budget, cancel)
        },
        cancel,
    )
}
