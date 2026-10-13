//! Exhaustive HDR/alpha collection search with original RGBA confirmation.
use crate::{
    collection::{CollectionSettings, scan_guarded_collection},
    model_compare::ProposalProviderError,
    model_input::GrayModelInput,
    model_projected_compare::ProjectedModelPolicy,
    model_projected_tiles::{
        ProjectedTileError, ProjectedTileEvidence, ProjectedTileWorkLimits,
        search_projected_model_tiles_files_with,
    },
    proposals::ScoredProposal,
};
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::DecodeRequest;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct ProjectedCollectionPolicy {
    pub model: ProjectedModelPolicy,
    /// Complete work admitted for each automatic 25-region recipe.
    pub pair_work: ProjectedTileWorkLimits,
    pub max_files: usize,
    pub max_pairs: usize,
    /// Must cover `pair_work.for_pairs(max_pairs)` before iterator consumption.
    pub work: ProjectedTileWorkLimits,
}
pub type ProjectedCollectionPair = crate::collection::CollectionPair<ProjectedTileEvidence>;
pub type ProjectedCollectionReport = crate::collection::CollectionReport<ProjectedTileEvidence>;
pub type ProjectedCollectionError = crate::collection::CollectionError<ProjectedTileError>;

/// Enumerate every unordered pair in sorted opaque-ID order. Each pair searches
/// all admitted whole/quadrant combinations until original-RGBA support. Projection
/// equality and provider confidence never establish support; no retrieval filter
/// suppresses region-only matches. Both supported and unsupported pairs are retained.
/// Visual support remains nontransitive and does not imply file identity.
///
/// Source/dependency/weights observations span the entire collection; any error
/// discards all earlier decisions. Final publication revalidates all sources
/// after the last caller checkpoint. All selected generations remain live.
/// Per-pair recipes and declared total work/slot capacities are admitted before
/// consuming files. Oversized actual collections refuse instead of truncating.
/// Local weights are required even for an empty collection. Managed accounting
/// excludes request paths, snapshot metadata, provider tensors and total RSS.
/// This exhaustive API does not claim index scalability or cross-pair decode reuse.
/// # Errors
/// Invalid policy, work/memory refusal, duplicate IDs, source/model/provider
/// failures or sticky cancellation. No partial collection report is returned.
pub fn scan_projected_model_collection_with(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: ProjectedCollectionPolicy,
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<ProjectedCollectionReport, ProjectedCollectionError> {
    scan_guarded_collection(
        files,
        weights,
        |_| {
            policy
                .pair_work
                .validate(policy.model)
                .map_err(ProjectedCollectionError::Policy)?;
            let required = policy
                .pair_work
                .for_pairs(policy.max_pairs)
                .map_err(ProjectedCollectionError::Policy)?;
            let t = policy.work;
            if [
                (required.model_input_reads, t.model_input_reads),
                (required.proposal_ranking_checks, t.proposal_ranking_checks),
                (required.proposal_separation_checks, t.proposal_separation_checks),
                (required.geometry_hypotheses, t.geometry_hypotheses),
                (required.original_pixel_work, t.original_pixel_work),
                (required.identity_input_checks, t.identity_input_checks),
                (required.identity_point_checks, t.identity_point_checks),
            ]
            .iter()
            .any(|(required, total)| required > total)
            {
                return Err(ProjectedCollectionError::Work);
            }
            Ok(CollectionSettings {
                max_files: policy.max_files,
                max_pairs: policy.max_pairs,
                max_model_bytes: policy.model.max_model_bytes,
                max_file_bytes: policy.model.decode.max_file_bytes,
            })
        },
        budget,
        |left, right, cancel| {
            search_projected_model_tiles_files_with(
                left,
                right,
                weights,
                policy.model,
                policy.pair_work,
                budget,
                &mut provider,
                cancel,
            )
        },
        cancel,
    )
}
