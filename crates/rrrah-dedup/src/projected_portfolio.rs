//! Native six-recipe and projected HDR/alpha region search in one guarded pair.
use crate::{
    anchor_rank_file::{
        AnchorRankFileError, AnchorReflectionFallbackEvidence, AnchorReflectionFallbackPolicy,
        NativePixelRefusal, search_anchor_rank_reflection_fallback_files,
        validate_anchor_reflection_fallback_policy,
    },
    exact::SnapshotError,
    guarded_pair::{PairContext, PairError, guarded_pair},
    local_rank::LocalRankError,
    model_compare::{ProposalProviderError, Provider},
    model_input::GrayModelInput,
    model_projected_compare::ProjectedModelPolicy,
    model_projected_tiles::{
        ProjectedTileError, ProjectedTileEvidence, ProjectedTileWorkLimits,
        search_projected_model_tiles_files_with,
    },
    proposals::ScoredProposal,
    search_portfolio::SearchOrder,
};
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::DecodeRequest;
use std::path::Path;

/// Named overlapping maxima across both stages; constituent policies still
/// enforce their other work. Torch tensors/workspaces and total RSS are excluded.
#[derive(Debug, Clone, Copy)]
pub struct ProjectedPortfolioWorkLimits {
    pub point_checks: u64,
    pub selection_checks: u64,
    pub rank_sites: u64,
    pub rank_pixel_reads: u64,
    pub geometry_hypotheses: u64,
    pub model: ProjectedTileWorkLimits,
}
impl ProjectedPortfolioWorkLimits {
    /// Prepay every declared collection pair without reducing either recipe.
    /// # Errors
    /// Aggregate work multiplication overflow.
    pub fn for_pairs(self, count: usize) -> Result<Self, ProjectedPortfolioError> {
        let model = self
            .model
            .for_pairs(count)
            .map_err(|_| ProjectedPortfolioError::Work)?;
        let count = u64::try_from(count).map_err(|_| ProjectedPortfolioError::Work)?;
        let times = |value: u64| value.checked_mul(count).ok_or(ProjectedPortfolioError::Work);
        Ok(Self {
            point_checks: times(self.point_checks)?,
            selection_checks: times(self.selection_checks)?,
            rank_sites: times(self.rank_sites)?,
            rank_pixel_reads: times(self.rank_pixel_reads)?,
            geometry_hypotheses: times(self.geometry_hypotheses)?,
            model,
        })
    }
    pub(crate) fn covers(self, r: Self) -> bool {
        [
            (r.point_checks, self.point_checks),
            (r.selection_checks, self.selection_checks),
            (r.rank_sites, self.rank_sites),
            (r.rank_pixel_reads, self.rank_pixel_reads),
            (r.geometry_hypotheses, self.geometry_hypotheses),
            (r.model.model_input_reads, self.model.model_input_reads),
            (
                r.model.proposal_ranking_checks,
                self.model.proposal_ranking_checks,
            ),
            (
                r.model.proposal_separation_checks,
                self.model.proposal_separation_checks,
            ),
            (r.model.geometry_hypotheses, self.model.geometry_hypotheses),
            (r.model.original_pixel_work, self.model.original_pixel_work),
            (r.model.identity_input_checks, self.model.identity_input_checks),
            (r.model.identity_point_checks, self.model.identity_point_checks),
        ]
        .iter()
        .all(|(required, total)| required <= total)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct ProjectedPortfolioPolicy {
    pub native: AnchorReflectionFallbackPolicy,
    pub model: ProjectedModelPolicy,
    pub model_work: ProjectedTileWorkLimits,
    pub order: SearchOrder,
    pub work: ProjectedPortfolioWorkLimits,
}
impl ProjectedPortfolioPolicy {
    /// Compute checked complete maxima without changing either stage's policy.
    /// # Errors
    /// Cumulative count overflow. Constituent validation occurs at entry.
    pub fn required_work(&self) -> Result<ProjectedPortfolioWorkLimits, ProjectedPortfolioError> {
        let n = self.native.search.anchors;
        let times = |v: u64| v.checked_mul(6).ok_or(ProjectedPortfolioError::Work);
        let native_geometry = times(
            self.native
                .search
                .search
                .search
                .search
                .gradient
                .search
                .sampling
                .trials,
        )?;
        Ok(ProjectedPortfolioWorkLimits {
            point_checks: times(n.local.maximum_point_checks)?,
            selection_checks: times(n.maximum_selection_checks)?,
            rank_sites: times(n.local.rank.maximum_sites)?,
            rank_pixel_reads: times(n.local.rank.maximum_pixel_reads)?,
            geometry_hypotheses: native_geometry
                .checked_add(self.model_work.geometry_hypotheses)
                .ok_or(ProjectedPortfolioError::Work)?,
            model: self.model_work,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInconclusive {
    PixelDomain(NativePixelRefusal),
    InsufficientAnchorGeometry,
}
#[derive(Debug, Clone, Copy)]
pub struct ProjectedStageMiss {
    pub attempts: usize,
    pub model_digest: [u8; 32],
    pub raw_proposals: usize,
    pub selected_points: usize,
    pub pixel_work: u64,
}
impl ProjectedStageMiss {
    fn from_evidence(e: &ProjectedTileEvidence) -> Self {
        let last = e.attempts().last().expect("nonempty projected recipe");
        Self {
            attempts: e.attempts().len(),
            model_digest: last.evidence.model_digest,
            raw_proposals: last.evidence.raw_proposals,
            selected_points: last.evidence.selected_points,
            pixel_work: e.pixel_work(),
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum ProjectedPortfolioMiss {
    Native { recipes: u32, candidates: usize },
    Model(ProjectedStageMiss),
    NativeInconclusive(NativeInconclusive),
}
#[derive(Debug)]
pub enum ProjectedPortfolioStage {
    Native(AnchorReflectionFallbackEvidence),
    Model(ProjectedTileEvidence),
}
impl ProjectedPortfolioStage {
    pub fn supported(&self) -> bool {
        match self {
            Self::Native(e) => e.evidence.anchors.is_some_and(|a| a.supported),
            Self::Model(e) => e.supported(),
        }
    }
}
#[derive(Debug)]
pub struct ProjectedPortfolioEvidence {
    pub order: SearchOrder,
    pub attempted_stages: u8,
    pub first_miss: Option<ProjectedPortfolioMiss>,
    pub terminal: ProjectedPortfolioStage,
}
impl ProjectedPortfolioEvidence {
    pub fn supported(&self) -> bool {
        self.terminal.supported()
    }
}
#[derive(Debug, thiserror::Error)]
pub enum ProjectedPortfolioError {
    #[error("projected portfolio cancelled")]
    Cancelled,
    #[error("projected portfolio cumulative work refusal")]
    Work,
    #[error("native stage inconclusive ({reason:?}); projected stage exhausted ({projected:?})")]
    InconclusiveNative {
        reason: NativeInconclusive,
        projected: ProjectedStageMiss,
    },
    #[error(transparent)]
    Source(#[from] SnapshotError),
    #[error(transparent)]
    Native(#[from] AnchorRankFileError),
    #[error(transparent)]
    Model(#[from] ProjectedTileError),
}
impl PairError for ProjectedPortfolioError {
    fn cancelled() -> Self {
        Self::Cancelled
    }
    fn source(error: SnapshotError) -> Self {
        Self::Source(error)
    }
}
pub(crate) fn validate_projected_portfolio(
    policy: ProjectedPortfolioPolicy,
    factors: &[f64],
    cancel: &dyn Fn() -> bool,
) -> Result<(), ProjectedPortfolioError> {
    validate_anchor_reflection_fallback_policy(policy.native, factors, &cancel)?;
    policy.model_work.validate(policy.model)?;
    if !policy.work.covers(policy.required_work()?) {
        return Err(ProjectedPortfolioError::Work);
    }
    Ok(())
}
/// Try native six-recipe and projected full/quadrant search in either order.
/// Only original-pixel-supported results stop early. Native pixel-domain refusal
/// and insufficient anchor geometry permit an independent projected attempt.
/// If it also misses, the final outcome remains typed inconclusive. Other
/// native failures and every projected failure remain terminal, never misses.
///
/// Both stages and cumulative work are admitted before IO. Source/dependency,
/// request-generation and local-weight observations span the whole sequence;
/// final publication revalidates without further caller callbacks. Unsupported
/// stage buffers release before fallback. Native is actually attempted, with no
/// whole-image eligibility filter that could suppress a valid regional witness.
/// Local weights are required even for native early support. This is local visual
/// evidence, not file identity, universal recall or total provider RSS admission.
/// # Errors
/// Invalid policy, named work/memory refusal, typed constituent failures,
/// changed sources/dependencies/weights, sticky cancellation or final inconclusive.
#[allow(clippy::too_many_arguments)]
pub fn search_projected_portfolio_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: ProjectedPortfolioPolicy,
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
) -> Result<ProjectedPortfolioEvidence, ProjectedPortfolioError> {
    let native = policy
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
        native.max_file_bytes.min(policy.model.decode.max_file_bytes),
        policy.model.max_model_bytes,
        |cancel| validate_projected_portfolio(policy, factors, cancel),
        |context| {
            run(
                left,
                right,
                weights,
                policy,
                factors,
                budget,
                &mut provider,
                context,
            )
        },
        cancel,
    )?
    .map_err(|(reason, projected)| ProjectedPortfolioError::InconclusiveNative { reason, projected })
}
#[allow(clippy::too_many_arguments)]
fn run(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: ProjectedPortfolioPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    provider: &mut Provider<'_>,
    context: &PairContext<'_, ProjectedPortfolioError>,
) -> Result<
    Result<ProjectedPortfolioEvidence, (NativeInconclusive, ProjectedStageMiss)>,
    ProjectedPortfolioError,
> {
    let mut first_miss = None;
    let mut inconclusive = None;
    let mut projected_miss = None;
    for index in 0..2 {
        context.verify()?;
        let native = (policy.order == SearchOrder::NativeFirst) == (index == 0);
        let stage = if native {
            match search_anchor_rank_reflection_fallback_files(
                left,
                right,
                policy.native,
                factors,
                budget,
                context.cancel,
            ) {
                Ok(e) => ProjectedPortfolioStage::Native(e),
                Err(error) => {
                    let reason = if let Some(reason) = error.pixel_refusal() {
                        NativeInconclusive::PixelDomain(reason)
                    } else if matches!(
                        error,
                        AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry)
                            | AnchorRankFileError::Search(crate::local_scan::LocalFileError::Rank(
                                LocalRankError::InsufficientGeometry
                            ))
                    ) {
                        NativeInconclusive::InsufficientAnchorGeometry
                    } else {
                        return Err(ProjectedPortfolioError::Native(error));
                    };
                    context.verify()?;
                    inconclusive = Some(reason);
                    if index == 1 {
                        return Ok(Err((reason, projected_miss.expect("projected first stage miss"))));
                    }
                    first_miss = Some(ProjectedPortfolioMiss::NativeInconclusive(reason));
                    continue;
                }
            }
        } else {
            ProjectedPortfolioStage::Model(search_projected_model_tiles_files_with(
                left,
                right,
                weights,
                policy.model,
                policy.model_work,
                budget,
                &mut *provider,
                context.cancel,
            )?)
        };
        context.verify()?;
        if !stage.supported() {
            if let ProjectedPortfolioStage::Model(e) = &stage {
                projected_miss = Some(ProjectedStageMiss::from_evidence(e));
            }
        }
        if stage.supported() || index == 1 {
            if let Some(reason) = inconclusive {
                if !stage.supported() {
                    return Ok(Err((reason, projected_miss.expect("projected terminal miss"))));
                }
            }
            return Ok(Ok(ProjectedPortfolioEvidence {
                order: policy.order,
                attempted_stages: index + 1,
                first_miss,
                terminal: stage,
            }));
        }
        first_miss = Some(match &stage {
            ProjectedPortfolioStage::Native(e) => ProjectedPortfolioMiss::Native {
                recipes: e.attempted_recipes,
                candidates: e.evidence.search.correspondences.len(),
            },
            ProjectedPortfolioStage::Model(e) => {
                ProjectedPortfolioMiss::Model(ProjectedStageMiss::from_evidence(e))
            }
        });
        drop(stage);
    }
    unreachable!("two stages produce a terminal outcome")
}
