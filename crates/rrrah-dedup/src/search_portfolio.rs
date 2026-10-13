//! Native six-recipe and learned proposal search under one guarded lifecycle.
use crate::{
    anchor_rank_file::{
        AnchorRankFileError, AnchorReflectionFallbackEvidence, AnchorReflectionFallbackPolicy,
        search_anchor_rank_reflection_fallback_files, validate_anchor_reflection_fallback_policy,
    },
    exact::SnapshotError,
    guarded_pair::{PairContext, PairError, guarded_pair},
    local_rank::LocalRankError,
    model_compare::{
        JointModelComparisonPolicy, ModelComparisonError, ModelComparisonEvidence, ModelComparisonPolicy,
        ProposalProviderError, Provider, compare_model_files_with, compare_model_joint_files_with,
        validate_model_comparison_policy,
    },
    model_input::GrayModelInput,
    model_tiles::{
        MAX_MODEL_TILE_ATTEMPTS, ModelTileError, ModelTileEvidence, ModelTileWorkLimits,
        search_joint_model_tiles_files_with, search_model_tiles_files_with, validate_model_tile_work,
    },
    proposals::ScoredProposal,
};
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::DecodeRequest;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOrder {
    NativeFirst,
    ModelFirst,
}

/// Prepaid maxima for overlapping work across six native recipes and either
/// one whole-image model attempt or the optional 25-attempt tile recipe.
/// Other work limits remain enforced by each constituent policy.
/// These are work counts, not a total inference tensor/RSS admission promise.
#[derive(Debug, Clone, Copy)]
pub struct PortfolioWorkLimits {
    pub point_checks: u64,
    pub selection_checks: u64,
    pub rank_sites: u64,
    pub pixel_reads: u64,
    pub geometry_hypotheses: u64,
    pub model_input_reads: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct SearchPortfolioPolicy {
    pub native: AnchorReflectionFallbackPolicy,
    pub model: ModelComparisonPolicy,
    pub order: SearchOrder,
    /// Optional automatic whole-image/quadrant search within the model stage.
    /// Its complete cumulative work must be admitted even if native support
    /// avoids inference. `None` retains the single whole-image model attempt.
    pub model_tiles: Option<ModelTileWorkLimits>,
    pub work: PortfolioWorkLimits,
}
impl SearchPortfolioPolicy {
    /// Calculate checked cumulative maxima for the configured recipe. This does
    /// not mutate limits or validate constituent policies; callers may explicitly
    /// assign the result to `work`, or supply stricter limits to refuse search.
    ///
    /// # Errors
    /// Cumulative arithmetic overflow.
    pub fn required_work(&self) -> Result<PortfolioWorkLimits, SearchPortfolioError> {
        let attempts = if self.model_tiles.is_some() {
            MAX_MODEL_TILE_ATTEMPTS as u64
        } else {
            1
        };
        let native = self.native.search.anchors;
        let model = self.model.anchors;
        let add = |n: u64, m: u64| {
            n.checked_mul(6)
                .and_then(|v| m.checked_mul(attempts).and_then(|m| v.checked_add(m)))
                .ok_or(SearchPortfolioError::Work)
        };
        Ok(PortfolioWorkLimits {
            point_checks: add(
                native.local.maximum_point_checks,
                model.local.maximum_point_checks,
            )?,
            selection_checks: add(native.maximum_selection_checks, model.maximum_selection_checks)?,
            rank_sites: add(native.local.rank.maximum_sites, model.local.rank.maximum_sites)?,
            pixel_reads: add(
                native.local.rank.maximum_pixel_reads,
                model.local.rank.maximum_pixel_reads,
            )?,
            geometry_hypotheses: add(
                self.native
                    .search
                    .search
                    .search
                    .search
                    .gradient
                    .search
                    .sampling
                    .trials,
                self.model.sampling.trials,
            )?,
            model_input_reads: self
                .model
                .input
                .max_pixel_reads
                .checked_mul(2)
                .and_then(|v| v.checked_mul(attempts))
                .ok_or(SearchPortfolioError::Work)?,
        })
    }
}

#[derive(Debug)]
pub enum SearchStageEvidence {
    Native(AnchorReflectionFallbackEvidence),
    Model(ModelComparisonEvidence),
    ModelTiles(ModelTileEvidence),
}
impl SearchStageEvidence {
    pub fn supported(&self) -> bool {
        match self {
            Self::Native(e) => e.evidence.anchors.is_some_and(|a| a.supported),
            Self::Model(e) => e.supported(),
            Self::ModelTiles(e) => e.supported(),
        }
    }
}

/// A stage can retain insufficient anchor support as a
/// distinct inconclusive reason while an independent stage is still tried.
#[derive(Debug, Clone, Copy)]
pub enum InsufficientAnchorStage {
    Native,
    Model {
        model_digest: [u8; 32],
    },
    ModelTiles {
        attempts: usize,
        insufficient_attempts: usize,
        model_digest: [u8; 32],
    },
}

/// Small audit record for the unsupported first stage. Its managed candidate
/// buffers are released before the second stage is admitted.
#[derive(Debug, Clone, Copy)]
pub enum SearchStageMiss {
    InsufficientAnchorGeometry(InsufficientAnchorStage),
    Native {
        recipes: u32,
        candidates: usize,
    },
    Model {
        model_digest: [u8; 32],
        raw_proposals: usize,
        selected_points: usize,
    },
    /// Counts refer to the last unsupported region; `attempts` counts the whole
    /// exhausted model recipe. Earlier attempt buffers are no longer retained.
    ModelTiles {
        attempts: usize,
        model_digest: [u8; 32],
        raw_proposals: usize,
        selected_points: usize,
    },
}
#[derive(Debug)]
pub struct SearchPortfolioEvidence {
    pub order: SearchOrder,
    pub attempted_stages: u8,
    pub first_miss: Option<SearchStageMiss>,
    pub terminal: SearchStageEvidence,
}
impl SearchPortfolioEvidence {
    pub fn supported(&self) -> bool {
        self.terminal.supported()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SearchPortfolioError {
    #[error("search portfolio exhausted with insufficient anchor geometry: {0:?}")]
    InsufficientAnchorGeometry(InsufficientAnchorStage),
    #[error("search portfolio cancelled")]
    Cancelled,
    #[error("search portfolio cumulative work refusal")]
    Work,
    #[error(transparent)]
    Source(#[from] SnapshotError),
    #[error(transparent)]
    Native(#[from] AnchorRankFileError),
    #[error(transparent)]
    Model(#[from] ModelComparisonError),
    #[error(transparent)]
    ModelTiles(#[from] ModelTileError),
}

pub(crate) fn validate_search_portfolio_policy(
    policy: SearchPortfolioPolicy,
    factors: &[f64],
    cancel: &impl Fn() -> bool,
) -> Result<(), SearchPortfolioError> {
    validate_anchor_reflection_fallback_policy(policy.native, factors, cancel)?;
    validate_model_comparison_policy(policy.model)?;
    if let Some(work) = policy.model_tiles {
        validate_model_tile_work(policy.model, work)?;
    }
    let required = policy.required_work()?;
    if [
        (required.point_checks, policy.work.point_checks),
        (required.selection_checks, policy.work.selection_checks),
        (required.rank_sites, policy.work.rank_sites),
        (required.pixel_reads, policy.work.pixel_reads),
        (required.geometry_hypotheses, policy.work.geometry_hypotheses),
        (required.model_input_reads, policy.work.model_input_reads),
    ]
    .iter()
    .any(|(required, limit)| required > limit)
    {
        return Err(SearchPortfolioError::Work);
    }
    Ok(())
}

/// Try the requested order and stop at the first original-pixel-supported result.
/// Preflight covers both stages before I/O; both original files, external decode
/// dependencies and local weights are guarded across the whole sequence. Thus
/// weights must exist even when a native-first search succeeds without inference.
/// Ordinary misses and typed insufficient-anchor geometry advance. If no stage
/// supports and any was inconclusive, the final result remains inconclusive.
/// All other typed failures stop search.
/// Cancellation is sticky for the caller and both request generations, including
/// final publication. Unsupported first-stage buffers release before fallback.
/// Support is local visual evidence, not whole-file identity or universal recall.
///
/// # Errors
/// Constituent policy/source/provider/decode/geometry/pixel failures, aggregate
/// work refusal, changed input/dependency/weights or cancellation. Provider
/// tensor/workspace allocations remain outside the managed payload account.
#[allow(clippy::too_many_arguments)]
pub fn search_portfolio_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: SearchPortfolioPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<SearchPortfolioEvidence, SearchPortfolioError> {
    let native_decode = policy
        .native
        .search
        .search
        .search
        .search
        .gradient
        .search
        .local
        .decode;
    guarded_pair(
        left,
        right,
        weights,
        native_decode
            .max_file_bytes
            .min(policy.model.decode.max_file_bytes),
        policy.model.max_model_bytes,
        |cancel| validate_search_portfolio_policy(policy, factors, &cancel),
        |context| {
            search_portfolio_core(
                left,
                right,
                weights,
                policy,
                factors,
                budget,
                &mut provider,
                context,
                None,
                None,
            )
        },
        cancel,
    )?
    .map_err(SearchPortfolioError::InsufficientAnchorGeometry)
}
impl PairError for SearchPortfolioError {
    fn cancelled() -> Self {
        Self::Cancelled
    }
    fn source(error: SnapshotError) -> Self {
        Self::Source(error)
    }
}
#[allow(clippy::too_many_arguments)]
fn search_portfolio_core(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: SearchPortfolioPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    provider: &mut Provider<'_>,
    context: &PairContext<'_, SearchPortfolioError>,
    joint_model: Option<(u64, u64)>,
    bank_model: Option<(crate::model_banks::ModelBankSearchPolicy, &[u32])>,
) -> Result<Result<SearchPortfolioEvidence, InsufficientAnchorStage>, SearchPortfolioError> {
    let cancelled = context.cancel;
    let mut first_miss = None;
    let mut inconclusive = None;
    for index in 0..2 {
        context.verify()?;
        let native = (policy.order == SearchOrder::NativeFirst) == (index == 0);
        let stage_result = if native {
            search_anchor_rank_reflection_fallback_files(
                left,
                right,
                policy.native,
                factors,
                budget,
                &cancelled,
            )
            .map(SearchStageEvidence::Native)
            .map_err(SearchPortfolioError::Native)
        } else if let Some((banks, radii)) = bank_model {
            crate::model_banks::search_model_banks_coverage_files_with(
                left,
                right,
                weights,
                banks,
                radii,
                budget,
                &mut *provider,
                &cancelled,
            )
            .map_err(SearchPortfolioError::ModelTiles)
            .and_then(|e| {
                if e.supported() {
                    Ok(SearchStageEvidence::Model(e.comparison))
                } else {
                    Err(SearchPortfolioError::InsufficientAnchorGeometry(
                        InsufficientAnchorStage::Model {
                            model_digest: e.comparison.model_digest,
                        },
                    ))
                }
            })
        } else if let Some(work) = policy.model_tiles {
            let result = if let Some((per_attempt, total)) = joint_model {
                search_joint_model_tiles_files_with(
                    left,
                    right,
                    weights,
                    JointModelComparisonPolicy {
                        comparison: policy.model,
                        maximum_projection_checks: per_attempt,
                    },
                    total,
                    work,
                    budget,
                    &mut *provider,
                    &cancelled,
                )
            } else {
                search_model_tiles_files_with(
                    left,
                    right,
                    weights,
                    policy.model,
                    work,
                    budget,
                    &mut *provider,
                    &cancelled,
                )
            };
            result
                .map(SearchStageEvidence::ModelTiles)
                .map_err(SearchPortfolioError::ModelTiles)
        } else {
            let result = if let Some((per_attempt, _)) = joint_model {
                compare_model_joint_files_with(
                    left,
                    right,
                    weights,
                    JointModelComparisonPolicy {
                        comparison: policy.model,
                        maximum_projection_checks: per_attempt,
                    },
                    budget,
                    &mut *provider,
                    &cancelled,
                )
            } else {
                compare_model_files_with(
                    left,
                    right,
                    weights,
                    policy.model,
                    budget,
                    &mut *provider,
                    &cancelled,
                )
            };
            result
                .map(SearchStageEvidence::Model)
                .map_err(SearchPortfolioError::Model)
        };
        let evidence = match stage_result {
            Ok(evidence) => evidence,
            Err(error) => {
                let stage = match error {
                    SearchPortfolioError::InsufficientAnchorGeometry(stage) => stage,
                    SearchPortfolioError::ModelTiles(ModelTileError::InsufficientAnchorGeometry(e)) => {
                        InsufficientAnchorStage::ModelTiles {
                            attempts: e.attempts().len(),
                            insufficient_attempts: e.insufficient_geometry_attempts(),
                            model_digest: e.model_digest(),
                        }
                    }
                    SearchPortfolioError::Model(ModelComparisonError::Pixels(AnchorRankFileError::Rank(
                        LocalRankError::InsufficientGeometry,
                    ))) => InsufficientAnchorStage::Model {
                        model_digest: context.model.digest(),
                    },
                    SearchPortfolioError::Native(AnchorRankFileError::Rank(
                        LocalRankError::InsufficientGeometry,
                    )) => InsufficientAnchorStage::Native,
                    other => return Err(other),
                };
                // Error evidence and its managed buffers have been dropped
                // before the alternate stage. Revalidate before advancing.
                context.verify()?;
                let reason = *inconclusive.get_or_insert(stage);
                if index == 1 {
                    return Ok(Err(reason));
                }
                first_miss = Some(SearchStageMiss::InsufficientAnchorGeometry(stage));
                continue;
            }
        };
        context.verify()?;
        if evidence.supported() || index == 1 {
            return Ok(match inconclusive {
                Some(reason) if !evidence.supported() => Err(reason),
                _ => Ok(SearchPortfolioEvidence {
                    order: policy.order,
                    attempted_stages: index + 1,
                    first_miss,
                    terminal: evidence,
                }),
            });
        }
        first_miss = Some(match &evidence {
            SearchStageEvidence::Native(e) => SearchStageMiss::Native {
                recipes: e.attempted_recipes,
                candidates: e.evidence.search.correspondences.len(),
            },
            SearchStageEvidence::Model(e) => SearchStageMiss::Model {
                model_digest: e.model_digest,
                raw_proposals: e.raw_proposals,
                selected_points: e.selected_points,
            },
            SearchStageEvidence::ModelTiles(e) => SearchStageMiss::ModelTiles {
                attempts: e.attempts().len(),
                model_digest: e.model_digest(),
                raw_proposals: e
                    .terminal()
                    .evidence
                    .comparison()
                    .ok_or(SearchPortfolioError::Work)?
                    .raw_proposals,
                selected_points: e
                    .terminal()
                    .evidence
                    .comparison()
                    .ok_or(SearchPortfolioError::Work)?
                    .selected_points,
            },
        });
        drop(evidence);
    }
    unreachable!("two stages produce a terminal outcome")
}

/// Opt-in recovery adds six opposite-direction proposal recipes. Pixel and
/// anchor acceptance still run in the caller's original direction.
#[derive(Debug, Clone, Copy)]
pub struct SearchRecoveryPolicy {
    pub primary: SearchPortfolioPolicy,
    pub opposite: crate::anchor_rank_file::OppositeAnchorSearchPolicy,
    pub work: PortfolioWorkLimits,
}
impl SearchRecoveryPolicy {
    pub fn required_work(&self) -> Result<PortfolioWorkLimits, SearchPortfolioError> {
        self.with_opposite_work(self.primary.required_work()?)
    }
    fn with_opposite_work(
        &self,
        mut required: PortfolioWorkLimits,
    ) -> Result<PortfolioWorkLimits, SearchPortfolioError> {
        let anchors = self.opposite.search.search.anchors;
        let add = |total: &mut u64, extra: u64| -> Result<(), SearchPortfolioError> {
            *total = extra
                .checked_mul(6)
                .and_then(|v| total.checked_add(v))
                .ok_or(SearchPortfolioError::Work)?;
            Ok(())
        };
        add(&mut required.point_checks, anchors.local.maximum_point_checks)?;
        add(&mut required.selection_checks, anchors.maximum_selection_checks)?;
        add(&mut required.rank_sites, anchors.local.rank.maximum_sites)?;
        add(&mut required.pixel_reads, anchors.local.rank.maximum_pixel_reads)?;
        add(
            &mut required.geometry_hypotheses,
            self.opposite
                .search
                .search
                .search
                .search
                .search
                .gradient
                .search
                .sampling
                .trials,
        )?;
        Ok(required)
    }
}

#[derive(Debug)]
pub enum SearchRecoveryOutcome {
    Primary(SearchPortfolioEvidence),
    /// `primary_insufficient` retains the primary stage's uncertainty without
    /// retaining its unsupported candidate buffers during recovery.
    Opposite {
        primary_insufficient: Option<InsufficientAnchorStage>,
        evidence: crate::anchor_rank_file::OppositeAnchorSearchEvidence,
    },
    Inconclusive {
        primary_insufficient: Option<InsufficientAnchorStage>,
        opposite_insufficient: bool,
    },
}
impl SearchRecoveryOutcome {
    /// `None` means insufficient evidence, never a negative decision.
    pub fn supported(&self) -> Option<bool> {
        match self {
            Self::Primary(e) => Some(e.supported()),
            Self::Opposite { evidence, .. } => Some(evidence.anchors.is_some_and(|a| a.supported)),
            Self::Inconclusive { .. } => None,
        }
    }
}

/// Run primary search, then opposite proposals if needed, under one source and
/// weights guard. Combined work and swapped-point storage are admitted before
/// file I/O. Provider tensor/workspace memory remains outside this admission.
/// Constituent errors stop execution; unresolved insufficient geometry remains
/// inconclusive. Local support does not establish whole-file identity.
#[allow(clippy::too_many_arguments)]
pub fn search_recovery_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: SearchRecoveryPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<SearchRecoveryOutcome, SearchPortfolioError> {
    search_recovery_files_selected(
        left, right, weights, policy, factors, budget, None, None, provider, cancel,
    )
}

/// Primary search followed by joint caller-coordinate opposite recovery.
/// The additional projection ceiling covers all six recovery fits and is
/// validated before source/weights IO. Existing primary work limits still apply.
#[allow(clippy::too_many_arguments)]
pub fn search_joint_recovery_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: SearchRecoveryPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    maximum_projection_checks: u64,
    provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<SearchRecoveryOutcome, SearchPortfolioError> {
    search_recovery_files_selected(
        left,
        right,
        weights,
        policy,
        factors,
        budget,
        Some(maximum_projection_checks),
        None,
        provider,
        cancel,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn search_recovery_files_selected(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: SearchRecoveryPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    joint_projection_checks: Option<u64>,
    bank_model: Option<(
        crate::joint_witnesses::JointBankPolicy,
        ModelTileWorkLimits,
        &[u32],
    )>,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<SearchRecoveryOutcome, SearchPortfolioError> {
    use crate::anchor_rank_file::search_anchor_rank_opposite_files_selected;
    let credit = std::cell::RefCell::new(None);
    let primary_decode = policy
        .primary
        .native
        .search
        .search
        .search
        .search
        .gradient
        .search
        .local
        .decode;
    let opposite_decode = policy
        .opposite
        .search
        .search
        .search
        .search
        .search
        .gradient
        .search
        .local
        .decode;
    guarded_pair(
        left,
        right,
        weights,
        primary_decode
            .max_file_bytes
            .min(opposite_decode.max_file_bytes)
            .min(policy.primary.model.decode.max_file_bytes),
        policy.primary.model.max_model_bytes,
        |cancel| {
            let bytes = if let Some((banks, work, radii)) = bank_model {
                validate_model_bank_recovery_policy(policy, banks, work, radii, factors, budget, &cancel)?
            } else {
                validate_search_recovery_policy(policy, factors, &cancel)?
            };
            if let Some(total) = joint_projection_checks {
                crate::anchor_rank_file::validate_joint_opposite_projection_policy(
                    policy.opposite,
                    total,
                    &cancel,
                )?;
            }
            *credit.borrow_mut() = Some(
                budget
                    .try_reserve(bytes)
                    .map_err(|_| AnchorRankFileError::Rank(LocalRankError::Budget))?,
            );
            Ok(())
        },
        |context| {
            let primary_insufficient = match search_portfolio_core(
                left,
                right,
                weights,
                policy.primary,
                factors,
                budget,
                &mut provider,
                context,
                None,
                bank_model.map(|(banks, work, radii)| {
                    (
                        crate::model_banks::ModelBankSearchPolicy {
                            comparison: policy.primary.model,
                            banks,
                            work,
                        },
                        radii,
                    )
                }),
            )? {
                Ok(evidence) if evidence.supported() => {
                    return Ok(SearchRecoveryOutcome::Primary(evidence));
                }
                Ok(_) => None,
                Err(reason) => Some(reason),
            };
            context.verify()?;
            match search_anchor_rank_opposite_files_selected(
                left,
                right,
                policy.opposite,
                factors,
                budget,
                credit.borrow_mut().take(),
                joint_projection_checks,
                context.cancel,
            ) {
                Ok(evidence)
                    if evidence.anchors.is_some_and(|a| a.supported) || primary_insufficient.is_none() =>
                {
                    Ok(SearchRecoveryOutcome::Opposite {
                        primary_insufficient,
                        evidence,
                    })
                }
                Ok(_) => Ok(SearchRecoveryOutcome::Inconclusive {
                    primary_insufficient,
                    opposite_insufficient: false,
                }),
                Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry)) => {
                    Ok(SearchRecoveryOutcome::Inconclusive {
                        primary_insufficient,
                        opposite_insufficient: true,
                    })
                }
                Err(error) => Err(error.into()),
            }
        },
        cancel,
    )
}

pub(crate) fn validate_search_recovery_policy(
    policy: SearchRecoveryPolicy,
    factors: &[f64],
    cancel: &impl Fn() -> bool,
) -> Result<u64, SearchPortfolioError> {
    validate_search_portfolio_policy(policy.primary, factors, &cancel)?;
    let bytes = crate::anchor_rank_file::validate_opposite_policy(policy.opposite, factors, &cancel)?;
    let required = policy.required_work()?;
    if [
        (required.point_checks, policy.work.point_checks),
        (required.selection_checks, policy.work.selection_checks),
        (required.rank_sites, policy.work.rank_sites),
        (required.pixel_reads, policy.work.pixel_reads),
        (required.geometry_hypotheses, policy.work.geometry_hypotheses),
        (required.model_input_reads, policy.work.model_input_reads),
    ]
    .iter()
    .any(|(required, limit)| required > limit)
    {
        return Err(SearchPortfolioError::Work);
    }
    Ok(bytes)
}

/// Native/model portfolio with explicit joint model geometry and cumulative
/// projection admission before either stage reads sources. Native recipes,
/// strict pixel support and fatal source/cancellation guards remain unchanged.
#[allow(clippy::too_many_arguments)]
pub fn search_joint_model_portfolio_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: SearchPortfolioPolicy,
    maximum_model_projection_checks: u64,
    maximum_total_model_projection_checks: u64,
    factors: &[f64],
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<SearchPortfolioEvidence, SearchPortfolioError> {
    let native_decode = policy
        .native
        .search
        .search
        .search
        .search
        .gradient
        .search
        .local
        .decode;
    guarded_pair(
        left,
        right,
        weights,
        native_decode
            .max_file_bytes
            .min(policy.model.decode.max_file_bytes),
        policy.model.max_model_bytes,
        |cancel| {
            validate_joint_model_portfolio_policy(
                policy,
                factors,
                &cancel,
                maximum_model_projection_checks,
                maximum_total_model_projection_checks,
            )
        },
        |context| {
            search_portfolio_core(
                left,
                right,
                weights,
                policy,
                factors,
                budget,
                &mut provider,
                context,
                Some((
                    maximum_model_projection_checks,
                    maximum_total_model_projection_checks,
                )),
                None,
            )
        },
        cancel,
    )?
    .map_err(SearchPortfolioError::InsufficientAnchorGeometry)
}

pub(crate) fn validate_joint_model_portfolio_policy(
    policy: SearchPortfolioPolicy,
    factors: &[f64],
    cancel: &dyn Fn() -> bool,
    maximum_model_projection_checks: u64,
    maximum_total_model_projection_checks: u64,
) -> Result<(), SearchPortfolioError> {
    validate_search_portfolio_policy(policy, factors, &cancel)?;
    let squared = policy.model.anchors.local.source_tolerance.powi(2);
    if !squared.is_finite() || squared <= 0. {
        return Err(ModelComparisonError::Policy.into());
    }
    let required = u64::try_from(policy.model.geometry.max_points)
        .ok()
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| {
            policy
                .model
                .sampling
                .trials
                .checked_add(1)
                .and_then(|t| n.checked_mul(t))
        })
        .ok_or(SearchPortfolioError::Model(ModelComparisonError::Geometry(
            crate::geometry::GeometryError::Budget,
        )))?;
    let attempts = if policy.model_tiles.is_some() {
        MAX_MODEL_TILE_ATTEMPTS as u64
    } else {
        1
    };
    if required > maximum_model_projection_checks
        || maximum_model_projection_checks
            .checked_mul(attempts)
            .is_none_or(|all| all > maximum_total_model_projection_checks)
    {
        return Err(SearchPortfolioError::Model(ModelComparisonError::Geometry(
            crate::geometry::GeometryError::Budget,
        )));
    }
    Ok(())
}

/// Six native recipes plus a fully admitted25-bank learned coverage stage.
/// The legacy `model_tiles` option is superseded by the supplied bank work.
pub fn required_model_bank_portfolio_work(
    policy: SearchPortfolioPolicy,
    work: ModelTileWorkLimits,
) -> Result<PortfolioWorkLimits, SearchPortfolioError> {
    let n = policy.native.search.anchors;
    let add = |native: u64, model: u64| {
        native
            .checked_mul(6)
            .and_then(|v| v.checked_add(model))
            .ok_or(SearchPortfolioError::Work)
    };
    Ok(PortfolioWorkLimits {
        point_checks: add(n.local.maximum_point_checks, work.point_checks)?,
        selection_checks: add(n.maximum_selection_checks, work.selection_checks)?,
        rank_sites: add(n.local.rank.maximum_sites, work.rank_sites)?,
        pixel_reads: add(n.local.rank.maximum_pixel_reads, work.pixel_reads)?,
        geometry_hypotheses: add(
            policy
                .native
                .search
                .search
                .search
                .search
                .gradient
                .search
                .sampling
                .trials,
            work.geometry_hypotheses,
        )?,
        model_input_reads: work.model_input_reads,
    })
}

/// Native/model order under the original pair observation, with automatic bank
/// aggregation and bounded pixel coverage in the learned stage. Missing learned
/// support stays typed inconclusive while the independent native stage is tried.
/// Both stages and every radius are admitted before IO even if stage1 supports.
#[allow(clippy::too_many_arguments)]
pub fn search_model_bank_portfolio_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: SearchPortfolioPolicy,
    banks: crate::joint_witnesses::JointBankPolicy,
    work: ModelTileWorkLimits,
    radii: &[u32],
    factors: &[f64],
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<SearchPortfolioEvidence, SearchPortfolioError> {
    let model = crate::model_banks::ModelBankSearchPolicy {
        comparison: policy.model,
        banks,
        work,
    };
    let native_decode = policy
        .native
        .search
        .search
        .search
        .search
        .gradient
        .search
        .local
        .decode;
    guarded_pair(
        left,
        right,
        weights,
        native_decode
            .max_file_bytes
            .min(policy.model.decode.max_file_bytes),
        policy.model.max_model_bytes,
        |cancel| validate_model_bank_portfolio_policy(policy, banks, work, radii, factors, budget, &cancel),
        |context| {
            search_portfolio_core(
                left,
                right,
                weights,
                policy,
                factors,
                budget,
                &mut provider,
                context,
                None,
                Some((model, radii)),
            )
        },
        cancel,
    )?
    .map_err(SearchPortfolioError::InsufficientAnchorGeometry)
}

/// Shared whole-operation admission for pair and collection bank routing.
pub(crate) fn validate_model_bank_portfolio_policy(
    policy: SearchPortfolioPolicy,
    banks: crate::joint_witnesses::JointBankPolicy,
    work: ModelTileWorkLimits,
    radii: &[u32],
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<(), SearchPortfolioError> {
    let model = crate::model_banks::ModelBankSearchPolicy {
        comparison: policy.model,
        banks,
        work,
    };
    validate_anchor_reflection_fallback_policy(policy.native, factors, &cancel)?;
    crate::model_banks::validate_model_bank_coverage_policy(model, radii, budget)?;
    let r = required_model_bank_portfolio_work(policy, work)?;
    if [
        (r.point_checks, policy.work.point_checks),
        (r.selection_checks, policy.work.selection_checks),
        (r.rank_sites, policy.work.rank_sites),
        (r.pixel_reads, policy.work.pixel_reads),
        (r.geometry_hypotheses, policy.work.geometry_hypotheses),
        (r.model_input_reads, policy.work.model_input_reads),
    ]
    .iter()
    .any(|(required, limit)| required > limit)
    {
        return Err(SearchPortfolioError::Work);
    }
    Ok(())
}

/// Complete bank primary plus six opposite-native recipes, checked additively.
pub fn required_model_bank_recovery_work(
    policy: SearchRecoveryPolicy,
    work: ModelTileWorkLimits,
) -> Result<PortfolioWorkLimits, SearchPortfolioError> {
    policy.with_opposite_work(required_model_bank_portfolio_work(policy.primary, work)?)
}

pub(crate) fn validate_model_bank_recovery_policy(
    policy: SearchRecoveryPolicy,
    banks: crate::joint_witnesses::JointBankPolicy,
    work: ModelTileWorkLimits,
    radii: &[u32],
    factors: &[f64],
    budget: &MemoryBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<u64, SearchPortfolioError> {
    validate_model_bank_portfolio_policy(policy.primary, banks, work, radii, factors, budget, cancel)?;
    let bytes = crate::anchor_rank_file::validate_opposite_policy(policy.opposite, factors, &cancel)?;
    let r = required_model_bank_recovery_work(policy, work)?;
    if [
        (r.point_checks, policy.work.point_checks),
        (r.selection_checks, policy.work.selection_checks),
        (r.rank_sites, policy.work.rank_sites),
        (r.pixel_reads, policy.work.pixel_reads),
        (r.geometry_hypotheses, policy.work.geometry_hypotheses),
        (r.model_input_reads, policy.work.model_input_reads),
    ]
    .iter()
    .any(|(required, limit)| required > limit)
    {
        return Err(SearchPortfolioError::Work);
    }
    Ok(bytes)
}

/// Bank aggregation/coverage followed by joint opposite-native recovery under
/// one original source/weights guard. All primary, opposite, projection and
/// storage work is admitted before IO. Primary unknown cannot become negative.
#[allow(clippy::too_many_arguments)]
pub fn search_model_bank_joint_recovery_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: SearchRecoveryPolicy,
    banks: crate::joint_witnesses::JointBankPolicy,
    work: ModelTileWorkLimits,
    radii: &[u32],
    maximum_projection_checks: u64,
    factors: &[f64],
    budget: &MemoryBudget,
    provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<SearchRecoveryOutcome, SearchPortfolioError> {
    search_recovery_files_selected(
        left,
        right,
        weights,
        policy,
        factors,
        budget,
        Some(maximum_projection_checks),
        Some((banks, work, radii)),
        provider,
        cancel,
    )
}

/// Terminal evidence for the fixed native/opposite/bank cascade.
#[derive(Debug)]
pub enum NativeBankRecoveryTerminal {
    Native(AnchorReflectionFallbackEvidence),
    Opposite(crate::anchor_rank_file::OppositeAnchorSearchEvidence),
    Model(ModelComparisonEvidence),
    Inconclusive(InsufficientAnchorStage),
}
/// Compact trace retains misses without keeping their managed candidate banks.
#[derive(Debug)]
pub struct NativeBankRecoveryEvidence {
    pub native_miss: Option<SearchStageMiss>,
    /// None means opposite was not attempted; true means insufficient geometry.
    pub opposite_insufficient: Option<bool>,
    /// Compact model search trace, retained on support and on unknown. No input
    /// buffers or candidate banks are retained. None means inference was skipped.
    pub model_search: Option<crate::model_banks::ModelBankSearchEvidence>,
    pub terminal: NativeBankRecoveryTerminal,
}
impl NativeBankRecoveryEvidence {
    pub fn supported(&self) -> Option<bool> {
        match &self.terminal {
            NativeBankRecoveryTerminal::Native(e) => Some(e.evidence.anchors.is_some_and(|a| a.supported)),
            NativeBankRecoveryTerminal::Opposite(e) => Some(e.anchors.is_some_and(|a| a.supported)),
            NativeBankRecoveryTerminal::Model(e) => Some(e.supported()),
            NativeBankRecoveryTerminal::Inconclusive(_) => None,
        }
    }
    pub fn model_attempted(&self) -> bool {
        matches!(
            self.terminal,
            NativeBankRecoveryTerminal::Model(_) | NativeBankRecoveryTerminal::Inconclusive(_)
        )
    }
}

/// Fixed direct-native, joint-opposite, learned-bank order. `primary.order` and
/// legacy `model_tiles` are superseded by this cascade's explicit order/work.
/// All three stages are admitted before IO; native support avoids loading the
/// provider. Every accepted stage independently confirms original pixels.
/// Source/dependency/weights/generation observations span the whole operation.
#[allow(clippy::too_many_arguments)]
pub fn search_native_bank_recovery_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: SearchRecoveryPolicy,
    banks: crate::joint_witnesses::JointBankPolicy,
    work: ModelTileWorkLimits,
    radii: &[u32],
    maximum_projection_checks: u64,
    factors: &[f64],
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<NativeBankRecoveryEvidence, SearchPortfolioError> {
    let credit = std::cell::RefCell::new(None);
    let native_decode = policy
        .primary
        .native
        .search
        .search
        .search
        .search
        .gradient
        .search
        .local
        .decode;
    let opposite_decode = policy
        .opposite
        .search
        .search
        .search
        .search
        .search
        .gradient
        .search
        .local
        .decode;
    guarded_pair(
        left,
        right,
        weights,
        native_decode
            .max_file_bytes
            .min(opposite_decode.max_file_bytes)
            .min(policy.primary.model.decode.max_file_bytes),
        policy.primary.model.max_model_bytes,
        |cancel| {
            let bytes =
                validate_model_bank_recovery_policy(policy, banks, work, radii, factors, budget, &cancel)?;
            crate::anchor_rank_file::validate_joint_opposite_projection_policy(
                policy.opposite,
                maximum_projection_checks,
                &cancel,
            )?;
            *credit.borrow_mut() = Some(
                budget
                    .try_reserve(bytes)
                    .map_err(|_| AnchorRankFileError::Rank(LocalRankError::Budget))?,
            );
            Ok(())
        },
        |context| {
            let native_miss = match search_anchor_rank_reflection_fallback_files(
                left,
                right,
                policy.primary.native,
                factors,
                budget,
                context.cancel,
            ) {
                Ok(e) if e.evidence.anchors.is_some_and(|a| a.supported) => {
                    return Ok(NativeBankRecoveryEvidence {
                        native_miss: None,
                        opposite_insufficient: None,
                        model_search: None,
                        terminal: NativeBankRecoveryTerminal::Native(e),
                    });
                }
                Ok(e) => SearchStageMiss::Native {
                    recipes: e.attempted_recipes,
                    candidates: e.evidence.search.correspondences.len(),
                },
                Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry)) => {
                    SearchStageMiss::InsufficientAnchorGeometry(InsufficientAnchorStage::Native)
                }
                Err(e) => return Err(e.into()),
            };
            context.verify()?;
            let opposite_insufficient =
                match crate::anchor_rank_file::search_anchor_rank_opposite_files_selected(
                    left,
                    right,
                    policy.opposite,
                    factors,
                    budget,
                    credit.borrow_mut().take(),
                    Some(maximum_projection_checks),
                    context.cancel,
                ) {
                    Ok(e) if e.anchors.is_some_and(|a| a.supported) => {
                        return Ok(NativeBankRecoveryEvidence {
                            native_miss: Some(native_miss),
                            opposite_insufficient: Some(false),
                            model_search: None,
                            terminal: NativeBankRecoveryTerminal::Opposite(e),
                        });
                    }
                    Ok(_) => false,
                    Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry)) => true,
                    Err(e) => return Err(e.into()),
                };
            context.verify()?;
            let e = crate::model_banks::search_model_banks_coverage_files_with(
                left,
                right,
                weights,
                crate::model_banks::ModelBankSearchPolicy {
                    comparison: policy.primary.model,
                    banks,
                    work,
                },
                radii,
                budget,
                &mut provider,
                context.cancel,
            )?;
            let terminal = if e.supported() {
                NativeBankRecoveryTerminal::Model(e.comparison)
            } else {
                NativeBankRecoveryTerminal::Inconclusive(InsufficientAnchorStage::Model {
                    model_digest: e.comparison.model_digest,
                })
            };
            Ok(NativeBankRecoveryEvidence {
                native_miss: Some(native_miss),
                opposite_insufficient: Some(opposite_insufficient),
                model_search: Some(e),
                terminal,
            })
        },
        cancel,
    )
}

/// One direction of an explicit two-direction confirmation. Tolerances belong
/// to this direction; reversing requests does not automatically rescale them.
#[derive(Clone, Copy, Debug)]
pub struct NativeBankDirectionPolicy<'a> {
    pub search: SearchRecoveryPolicy,
    pub banks: crate::joint_witnesses::JointBankPolicy,
    pub model_work: ModelTileWorkLimits,
    pub radii: &'a [u32],
    pub maximum_projection_checks: u64,
    pub factors: &'a [f64],
}

/// Compact direction result; managed correspondence banks are released before
/// starting the reverse direction.
#[derive(Clone, Copy, Debug)]
pub struct NativeBankDirectionSummary {
    pub support: Option<bool>,
    pub model_attempted: bool,
}

/// Both directions observed under one source/dependency/weights lifetime.
/// This is local visual evidence, not proof of whole-file identity or uniqueness.
#[derive(Clone, Copy, Debug)]
pub struct NativeBankConsensusEvidence {
    pub forward: NativeBankDirectionSummary,
    pub reverse: NativeBankDirectionSummary,
}
impl NativeBankConsensusEvidence {
    /// Missing directional support stays unknown; it is never a rejection.
    pub fn supported(&self) -> Option<bool> {
        if self.forward.support == Some(true) && self.reverse.support == Some(true) {
            Some(true)
        } else {
            None
        }
    }
}

/// Checked cumulative work for both complete cascades, including skipped stages.
pub fn required_native_bank_consensus_work(
    forward: NativeBankDirectionPolicy<'_>,
    reverse: NativeBankDirectionPolicy<'_>,
) -> Result<PortfolioWorkLimits, SearchPortfolioError> {
    let a = required_model_bank_recovery_work(forward.search, forward.model_work)?;
    let b = required_model_bank_recovery_work(reverse.search, reverse.model_work)?;
    let add = |a: u64, b: u64| a.checked_add(b).ok_or(SearchPortfolioError::Work);
    Ok(PortfolioWorkLimits {
        point_checks: add(a.point_checks, b.point_checks)?,
        selection_checks: add(a.selection_checks, b.selection_checks)?,
        rank_sites: add(a.rank_sites, b.rank_sites)?,
        pixel_reads: add(a.pixel_reads, b.pixel_reads)?,
        geometry_hypotheses: add(a.geometry_hypotheses, b.geometry_hypotheses)?,
        model_input_reads: add(a.model_input_reads, b.model_input_reads)?,
    })
}

/// Opt-in AND confirmation, separate from existing OR/fallback APIs. All work,
/// projection caps and both directional policies are admitted before file IO.
/// Each cascade retains its own limits; `work` and `maximum_projection_checks`
/// additionally bound the sum across directions. Execution errors remain errors.
/// The same provider runs sequentially; each request keeps its interpretation.
/// Reservations are sequential and results compact, not a hard process RSS cap.
#[allow(clippy::too_many_arguments)]
pub fn search_native_bank_consensus_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    forward: NativeBankDirectionPolicy<'_>,
    reverse: NativeBankDirectionPolicy<'_>,
    work: PortfolioWorkLimits,
    maximum_projection_checks: u64,
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<NativeBankConsensusEvidence, SearchPortfolioError> {
    search_native_bank_consensus_files_directional(
        left,
        right,
        weights,
        forward,
        reverse,
        work,
        maximum_projection_checks,
        budget,
        |_, weights, left, right, budget, cancel| provider(weights, left, right, budget, cancel),
        cancel,
    )
}

// Direction is explicit internally so native inference respects independent
// proposal caps while sharing one lazily loaded model across the operation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn search_native_bank_consensus_files_directional(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    forward: NativeBankDirectionPolicy<'_>,
    reverse: NativeBankDirectionPolicy<'_>,
    work: PortfolioWorkLimits,
    maximum_projection_checks: u64,
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        bool,
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<NativeBankConsensusEvidence, SearchPortfolioError> {
    let file_cap = |p: NativeBankDirectionPolicy<'_>| {
        p.search
            .primary
            .native
            .search
            .search
            .search
            .search
            .gradient
            .search
            .local
            .decode
            .max_file_bytes
            .min(
                p.search
                    .opposite
                    .search
                    .search
                    .search
                    .search
                    .search
                    .gradient
                    .search
                    .local
                    .decode
                    .max_file_bytes,
            )
            .min(p.search.primary.model.decode.max_file_bytes)
    };
    guarded_pair(
        left,
        right,
        weights,
        file_cap(forward).min(file_cap(reverse)),
        forward
            .search
            .primary
            .model
            .max_model_bytes
            .min(reverse.search.primary.model.max_model_bytes),
        |cancel| {
            let required = required_native_bank_consensus_work(forward, reverse)?;
            if [
                (required.point_checks, work.point_checks),
                (required.selection_checks, work.selection_checks),
                (required.rank_sites, work.rank_sites),
                (required.pixel_reads, work.pixel_reads),
                (required.geometry_hypotheses, work.geometry_hypotheses),
                (required.model_input_reads, work.model_input_reads),
            ]
            .iter()
            .any(|(required, limit)| required > limit)
                || forward
                    .maximum_projection_checks
                    .checked_add(reverse.maximum_projection_checks)
                    .is_none_or(|sum| sum > maximum_projection_checks)
            {
                return Err(SearchPortfolioError::Work);
            }
            for p in [forward, reverse] {
                let bytes = validate_model_bank_recovery_policy(
                    p.search,
                    p.banks,
                    p.model_work,
                    p.radii,
                    p.factors,
                    budget,
                    cancel,
                )?;
                crate::anchor_rank_file::validate_joint_opposite_projection_policy(
                    p.search.opposite,
                    p.maximum_projection_checks,
                    &cancel,
                )?;
                // Check each sequential reservation before IO; do not retain
                // duplicate credit while the inner cascade reserves its storage.
                drop(
                    budget
                        .try_reserve(bytes)
                        .map_err(|_| AnchorRankFileError::Rank(LocalRankError::Budget))?,
                );
            }
            Ok(())
        },
        |context| {
            let mut run =
                |a: &DecodeRequest, b: &DecodeRequest, p: NativeBankDirectionPolicy<'_>, is_forward: bool| {
                    let evidence = search_native_bank_recovery_files_with(
                        a,
                        b,
                        weights,
                        p.search,
                        p.banks,
                        p.model_work,
                        p.radii,
                        p.maximum_projection_checks,
                        p.factors,
                        budget,
                        |weights, left, right, budget, cancel| {
                            provider(is_forward, weights, left, right, budget, cancel)
                        },
                        context.cancel,
                    )?;
                    Ok::<_, SearchPortfolioError>(NativeBankDirectionSummary {
                        support: evidence.supported(),
                        model_attempted: evidence.model_attempted(),
                    })
                };
            let forward = run(left, right, forward, true)?;
            context.verify()?;
            let reverse = run(right, left, reverse, false)?;
            Ok(NativeBankConsensusEvidence { forward, reverse })
        },
        cancel,
    )
}
