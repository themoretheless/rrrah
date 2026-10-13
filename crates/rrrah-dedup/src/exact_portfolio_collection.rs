//! Exact-byte identity and full native/projected pair enumeration with explicit unknowns.
use crate::{
    collection::{CollectionSettings, scan_guarded_exact_collection},
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
pub struct ExactPortfolioCollectionPolicy {
    pub search: ProjectedPortfolioPolicy,
    pub max_files: usize,
    pub max_pairs: usize,
    pub work: ProjectedPortfolioWorkLimits,
}
#[derive(Debug)]
pub enum ExactPortfolioPairOutcome {
    ExactBytes,
    Complete(ProjectedPortfolioEvidence),
    Inconclusive {
        order: SearchOrder,
        reason: NativeInconclusive,
        projected: ProjectedStageMiss,
    },
}
impl ExactPortfolioPairOutcome {
    /// `None` preserves unknown native capability/geometric support. It must not
    /// be treated as a negative match, uniqueness or whole-file identity. Only the
    /// explicit ExactBytes variant proves source file byte equality.
    pub fn support(&self) -> Option<bool> {
        match self {
            Self::ExactBytes => Some(true),
            Self::Complete(e) => Some(e.supported()),
            Self::Inconclusive { .. } => None,
        }
    }
}
pub type ExactPortfolioCollectionPair = crate::collection::CollectionPair<ExactPortfolioPairOutcome>;
pub type ExactPortfolioCollectionReport = crate::collection::CollectionReport<ExactPortfolioPairOutcome>;
pub type ExactPortfolioCollectionError = crate::collection::CollectionError<ProjectedPortfolioError>;
/// Byte-confirm every matching digest, then examine byte-different pairs with
/// the complete visual portfolio. Retain supported, unsupported and explicitly
/// inconclusive outcomes. Only the exact final native-inconclusive outcome from
/// the guarded pair API becomes an unknown record; other errors discard the
/// whole collection. Source/dependency/weight mutations and cancellation never
/// become unknowns. No descriptor pruning, transitive grouping or truncation.
///
/// All constituent/aggregate work and slot capacities are admitted before input
/// consumption. IDs are unique and sorted; every pair is present on successful
/// publication. The shared collection engine guards all selected sources and
/// generations through the final read-only publication boundary. Local weights
/// are read only after a byte-different pair requires visual search. Empty,
/// singleton and all-exact collections do not read weights or decode dependencies.
/// Once visual work begins, dependencies of all files and local weights remain
/// guarded through publication, including idle files. ExactBytes proves only
/// source file content, independently of decode settings or sidecars. Managed
/// credit excludes request paths, snapshots and provider tensors/workspaces.
/// This is exhaustive enumeration, not indexed scalability or hard total RSS.
/// # Errors
/// Policy, work/memory/file/pair limits, duplicate IDs, source/model/provider
/// failures or sticky cancellation. No partial report escapes.
#[allow(clippy::too_many_arguments)]
pub fn scan_exact_projected_portfolio_collection_with(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: ExactPortfolioCollectionPolicy,
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
) -> Result<ExactPortfolioCollectionReport, ExactPortfolioCollectionError> {
    scan_guarded_exact_collection(
        files,
        weights,
        |cancel| {
            validate_projected_portfolio(policy.search, factors, cancel)
                .map_err(ExactPortfolioCollectionError::Policy)?;
            let required = policy
                .search
                .work
                .for_pairs(policy.max_pairs)
                .map_err(ExactPortfolioCollectionError::Policy)?;
            if !policy.work.covers(required) {
                return Err(ExactPortfolioCollectionError::Work);
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
            Ok(e) => Ok(ExactPortfolioPairOutcome::Complete(e)),
            Err(ProjectedPortfolioError::InconclusiveNative { reason, projected }) => {
                Ok(ExactPortfolioPairOutcome::Inconclusive {
                    order: policy.search.order,
                    reason,
                    projected,
                })
            }
            Err(error) => Err(error),
        },
        || ExactPortfolioPairOutcome::ExactBytes,
        cancel,
    )
}
