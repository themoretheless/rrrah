//! Complete native/projected pair enumeration with explicit unknown outcomes.
use crate::{
    collection::{CollectionSettings, scan_guarded_collection},
    model_compare::ProposalProviderError,
    model_input::GrayModelInput,
    projected_portfolio::{
        NativeInconclusive, ProjectedPortfolioError, ProjectedPortfolioEvidence, ProjectedPortfolioPolicy,
        ProjectedPortfolioWorkLimits, ProjectedStageMiss, search_projected_portfolio_files_with,
        validate_projected_portfolio,
    },
    proposals::ScoredProposal,
    search_portfolio::SearchOrder,
};
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::DecodeRequest;
use std::path::Path;
#[derive(Debug, Clone, Copy)]
pub struct ProjectedPortfolioCollectionPolicy {
    pub search: ProjectedPortfolioPolicy,
    pub max_files: usize,
    pub max_pairs: usize,
    pub work: ProjectedPortfolioWorkLimits,
}
#[derive(Debug)]
pub enum ProjectedPortfolioPairOutcome {
    Complete(ProjectedPortfolioEvidence),
    Inconclusive {
        order: SearchOrder,
        reason: NativeInconclusive,
        projected: ProjectedStageMiss,
    },
}
impl ProjectedPortfolioPairOutcome {
    /// `None` preserves unknown native capability/geometric support. It must not
    /// be treated as a negative match, uniqueness or whole-file identity.
    pub fn support(&self) -> Option<bool> {
        match self {
            Self::Complete(e) => Some(e.supported()),
            Self::Inconclusive { .. } => None,
        }
    }
}
pub type ProjectedPortfolioCollectionPair = crate::collection::CollectionPair<ProjectedPortfolioPairOutcome>;
pub type ProjectedPortfolioCollectionReport =
    crate::collection::CollectionReport<ProjectedPortfolioPairOutcome>;
pub type ProjectedPortfolioCollectionError = crate::collection::CollectionError<ProjectedPortfolioError>;
/// Examine every unordered pair, retaining supported, unsupported and explicitly
/// inconclusive outcomes. Only the exact final native-inconclusive outcome from
/// the guarded pair API becomes an unknown record; other errors discard the
/// whole collection. Source/dependency/weight mutations and cancellation never
/// become unknowns. No descriptor pruning, transitive grouping or truncation.
///
/// All constituent/aggregate work and slot capacities are admitted before input
/// consumption. IDs are unique and sorted; every pair is present on successful
/// publication. The shared collection engine guards all selected sources and
/// generations through the final read-only publication boundary. Local weights
/// remain required for empty collections and native early support. Managed
/// credit excludes request paths, snapshots and provider tensors/workspaces.
/// This is exhaustive enumeration, not indexed scalability or hard total RSS.
/// # Errors
/// Policy, work/memory/file/pair limits, duplicate IDs, source/model/provider
/// failures or sticky cancellation. No partial report escapes.
#[allow(clippy::too_many_arguments)]
pub fn scan_projected_portfolio_collection_with(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: ProjectedPortfolioCollectionPolicy,
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
) -> Result<ProjectedPortfolioCollectionReport, ProjectedPortfolioCollectionError> {
    scan_guarded_collection(
        files,
        weights,
        |cancel| {
            validate_projected_portfolio(policy.search, factors, cancel)
                .map_err(ProjectedPortfolioCollectionError::Policy)?;
            let required = policy
                .search
                .work
                .for_pairs(policy.max_pairs)
                .map_err(ProjectedPortfolioCollectionError::Policy)?;
            if !policy.work.covers(required) {
                return Err(ProjectedPortfolioCollectionError::Work);
            }
            let native = policy
                .search
                .native
                .search
                .search
                .search
                .search
                .gradient
                .search
                .local
                .decode;
            Ok(CollectionSettings {
                max_files: policy.max_files,
                max_pairs: policy.max_pairs,
                max_model_bytes: policy.search.model.max_model_bytes,
                max_file_bytes: native
                    .max_file_bytes
                    .min(policy.search.model.decode.max_file_bytes),
            })
        },
        budget,
        |left, right, cancel| match search_projected_portfolio_files_with(
            left,
            right,
            weights,
            policy.search,
            factors,
            budget,
            &mut provider,
            cancel,
        ) {
            Ok(e) => Ok(ProjectedPortfolioPairOutcome::Complete(e)),
            Err(ProjectedPortfolioError::InconclusiveNative { reason, projected }) => {
                Ok(ProjectedPortfolioPairOutcome::Inconclusive {
                    order: policy.search.order,
                    reason,
                    projected,
                })
            }
            Err(error) => Err(error),
        },
        cancel,
    )
}
