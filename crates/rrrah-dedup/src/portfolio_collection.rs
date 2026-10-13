//! Native/learned exhaustive collection search through the shared guarded engine.
use crate::{
    collection::{CollectionError as EngineError, CollectionSettings, scan_guarded_collection_ordered},
    exact::SnapshotError,
    model_compare::ProposalProviderError,
    model_input::GrayModelInput,
    proposals::ScoredProposal,
    search_portfolio::{
        PortfolioWorkLimits, SearchPortfolioError, SearchPortfolioEvidence, SearchPortfolioPolicy,
        search_portfolio_files_with, validate_search_portfolio_policy,
        search_joint_model_portfolio_files_with, validate_joint_model_portfolio_policy,
        search_model_bank_portfolio_files_with, validate_model_bank_portfolio_policy,
    },
};
use rrrah_core::{BufferError, MemoryBudget, SharedBuffer};
use rrrah_decode::DecodeRequest;
use std::path::Path;
#[derive(Debug, Clone, Copy)]
pub struct PortfolioCollectionPolicy {
    pub search: SearchPortfolioPolicy,
    pub max_files: usize,
    pub max_pairs: usize,
    /// Prepaid overlapping work for `max_pairs` complete pair attempts.
    pub work: PortfolioWorkLimits,
}

pub type PortfolioCollectionPair = crate::collection::CollectionPair<SearchPortfolioEvidence>;
pub type PortfolioCollectionReport = crate::collection::CollectionReport<SearchPortfolioEvidence>;
#[derive(Debug, thiserror::Error)]
pub enum PortfolioCollectionError {
    #[error("collection search cancelled")]
    Cancelled,
    #[error("collection file limit exceeded")]
    Files,
    #[error("complete collection pair enumeration exceeds pair limit")]
    Pairs,
    #[error("duplicate application id {0}")]
    DuplicateId(u64),
    #[error("collection cumulative work refusal")]
    Work,
    #[error(transparent)]
    Policy(#[from] SearchPortfolioError),
    #[error(transparent)]
    Memory(#[from] BufferError),
    #[error("model source: {0}")]
    ModelSource(#[source] SnapshotError),
    #[error("file {id}: {error}")]
    Source {
        id: u64,
        #[source]
        error: SnapshotError,
    },
    #[error("pair ({left}, {right}): {error}")]
    Pair {
        left: u64,
        right: u64,
        #[source]
        error: SearchPortfolioError,
    },
}

fn validate(
    policy: PortfolioCollectionPolicy,
    factors: &[f64],
    cancel: &dyn Fn() -> bool,
) -> Result<(), EngineError<SearchPortfolioError>> {
    validate_search_portfolio_policy(policy.search, factors, &cancel).map_err(EngineError::Policy)?;
    validate_collection_work(policy)
}

fn validate_collection_work(policy:PortfolioCollectionPolicy)->Result<(),EngineError<SearchPortfolioError>> {
    let count = u64::try_from(policy.max_pairs).map_err(|_| EngineError::Work)?;
    let p = policy.search.work;
    let t = policy.work;
    for (per_pair, total) in [
        (p.point_checks, t.point_checks),
        (p.selection_checks, t.selection_checks),
        (p.rank_sites, t.rank_sites),
        (p.pixel_reads, t.pixel_reads),
        (p.geometry_hypotheses, t.geometry_hypotheses),
        (p.model_input_reads, t.model_input_reads),
    ] {
        if per_pair.checked_mul(count).ok_or(EngineError::Work)? > total {
            return Err(EngineError::Work);
        }
    }
    Ok(())
}

/// Examine every unordered pair in sorted ID order. No retrieval pruning occurs.
/// All constituent and collection work is admitted before input consumption.
/// A source, dependency, weight or generation failure discards the entire batch.
/// Final publication revalidates all sources without further caller callbacks.
/// Local visual support is nontransitive. Managed slots and evidence exclude
/// request paths, snapshot metadata and provider tensors/workspaces; no total
/// RSS or collection scalability claim is made. Local weights are always required.
/// # Errors
/// Invalid policy, work/memory limits, duplicate IDs, source/model/provider errors
/// and sticky cancellation. No partial report is published.
#[allow(clippy::too_many_arguments)]
pub fn scan_portfolio_collection_with(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: PortfolioCollectionPolicy,
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
) -> Result<PortfolioCollectionReport, PortfolioCollectionError> {
    scan_collection_with_outcome(
        files,
        weights,
        policy,
        factors,
        budget,
        provider,
        cancel,
        None,
        None,
        None,
        |result| result,
    )
}

/// A completed local visual decision or explicit insufficient-geometry outcome.
/// Neither variant certifies whole-file identity.
#[derive(Debug)]
pub enum PortfolioPairOutcome {
    Complete(SearchPortfolioEvidence),
    Inconclusive(crate::search_portfolio::InsufficientAnchorStage),
}
impl PortfolioPairOutcome {
    /// `None` preserves insufficient evidence instead of reporting a negative.
    pub fn support(&self) -> Option<bool> {
        match self {
            Self::Complete(evidence) => Some(evidence.supported()),
            Self::Inconclusive(_) => None,
        }
    }
}
pub type PortfolioOutcomeCollectionReport = crate::collection::CollectionReport<PortfolioPairOutcome>;

/// Enumerate every pair and retain typed insufficient geometry as an unknown
/// record. Both search stages and all pair/final source guards still run.
/// The legacy collection API retains its error-on-inconclusive behavior.
/// # Errors
/// All policy, admission, source, dependency, weights, provider and cancellation
/// failures discard the entire report. Only final insufficient geometry is
/// represented as an outcome; it is never a negative comparison.
#[allow(clippy::too_many_arguments)]
pub fn scan_portfolio_outcome_collection_with(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: PortfolioCollectionPolicy,
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
) -> Result<PortfolioOutcomeCollectionReport, PortfolioCollectionError> {
    scan_collection_with_outcome(
        files,
        weights,
        policy,
        factors,
        budget,
        provider,
        cancel,
        None,
        None,
        None,
        |result| {
            result
                .map(PortfolioPairOutcome::Complete)
                .or_else(|error| match error {
                    SearchPortfolioError::InsufficientAnchorGeometry(reason) => {
                        Ok(PortfolioPairOutcome::Inconclusive(reason))
                    }
                    other => Err(other),
                })
        },
    )
}

/// Confirm every file pair, proposals first and exhaustive fallback afterward.
/// The schedule must name exactly the admitted file IDs. A mismatch returns
/// `Pairs` before source/model I/O. Final records remain in canonical ID order.
/// All existing guards and atomic error/cancellation behavior remain active.
/// # Errors
/// Policy, complete-work admission, mismatched schedule, source/provider failures
/// and cancellation refuse the entire report; only final insufficient geometry
/// is retained as an explicit unknown outcome.
#[allow(clippy::too_many_arguments)]
pub fn scan_portfolio_scheduled_outcome_collection_with(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: &crate::pair_schedule::CompletePairSchedule,
    weights: &Path,
    policy: PortfolioCollectionPolicy,
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
) -> Result<PortfolioOutcomeCollectionReport, PortfolioCollectionError> {
    scan_collection_with_outcome(
        files,
        weights,
        policy,
        factors,
        budget,
        provider,
        cancel,
        Some(schedule),
        None,
        None,
        |result| {
            result
                .map(PortfolioPairOutcome::Complete)
                .or_else(|error| match error {
                    SearchPortfolioError::InsufficientAnchorGeometry(reason) => {
                        Ok(PortfolioPairOutcome::Inconclusive(reason))
                    }
                    other => Err(other),
                })
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn scan_collection_with_outcome<O>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: PortfolioCollectionPolicy,
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
    order: Option<&crate::pair_schedule::CompletePairSchedule>,
    joint_model: Option<(u64,u64,u64)>,
    bank_model: Option<(crate::joint_witnesses::JointBankPolicy,crate::model_tiles::ModelTileWorkLimits,&[u32],ModelBankCollectionWorkLimits)>,
    outcome: impl Fn(Result<SearchPortfolioEvidence, SearchPortfolioError>) -> Result<O, SearchPortfolioError>,
) -> Result<crate::collection::CollectionReport<O>, PortfolioCollectionError> {
    scan_guarded_collection_ordered(
        files,
        weights,
        |cancel| {
            if let Some((banks,work,radii,total))=bank_model {
                validate_model_bank_portfolio_policy(policy.search,banks,work,radii,factors,budget,&cancel)
                    .map_err(EngineError::Policy)?;
                validate_collection_work(policy)?;
                let pairs=u64::try_from(policy.max_pairs).map_err(|_|EngineError::Work)?;
                for (per_pair,total) in [(banks.maximum_total_projection_checks,total.projection_checks),
                    (banks.maximum_total_separation_checks,total.separation_checks)] {
                    if per_pair.checked_mul(pairs).is_none_or(|required|required>total) {
                        return Err(EngineError::Work);
                    }
                }
            } else {
                validate(policy, factors, &cancel)?;
            }
            if let Some((per_attempt,per_pair,total))=joint_model {
                validate_joint_model_portfolio_policy(policy.search,factors,&cancel,per_attempt,per_pair)
                    .map_err(EngineError::Policy)?;
                let pairs=u64::try_from(policy.max_pairs).map_err(|_|EngineError::Work)?;
                if per_pair.checked_mul(pairs).is_none_or(|required|required>total) {
                    return Err(EngineError::Work);
                }
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
        |left, right, cancel| {
            let result=if let Some((banks,work,radii,_))=bank_model {
                search_model_bank_portfolio_files_with(left,right,weights,policy.search,banks,work,radii,
                    factors,budget,&mut provider,cancel)
            } else if let Some((per_attempt,per_pair,_))=joint_model {
                search_joint_model_portfolio_files_with(left,right,weights,policy.search,per_attempt,per_pair,
                    factors,budget,&mut provider,cancel)
            } else {
                search_portfolio_files_with(left,right,weights,policy.search,factors,budget,&mut provider,cancel)
            };
            outcome(result)
        },
        order,
        cancel,
    )
    .map_err(PortfolioCollectionError::from)
}

impl From<EngineError<SearchPortfolioError>> for PortfolioCollectionError {
    fn from(error: EngineError<SearchPortfolioError>) -> Self {
        match error {
            EngineError::Cancelled => Self::Cancelled,
            EngineError::Files => Self::Files,
            EngineError::Pairs => Self::Pairs,
            EngineError::DuplicateId(id) => Self::DuplicateId(id),
            EngineError::Work => Self::Work,
            EngineError::Policy(error) => Self::Policy(error),
            EngineError::Memory(error) => Self::Memory(error),
            EngineError::ModelSource(error) => Self::ModelSource(error),
            EngineError::Source { id, error } => Self::Source { id, error },
            EngineError::Pair { left, right, error } => Self::Pair { left, right, error },
        }
    }
}

/// Exhaustive collection policy including six additional opposite-native
/// recovery attempts per pair. Work limits cover `max_pairs` complete attempts.
#[derive(Debug, Clone, Copy)]
pub struct RecoveryCollectionPolicy {
    pub search: crate::search_portfolio::SearchRecoveryPolicy,
    pub max_files: usize,
    pub max_pairs: usize,
    pub work: PortfolioWorkLimits,
}
impl RecoveryCollectionPolicy {
    /// Checked aggregate of admitted per-pair maxima, including recovery.
    pub fn required_work(&self) -> Result<PortfolioWorkLimits, PortfolioCollectionError> {
        let count = u64::try_from(self.max_pairs).map_err(|_| PortfolioCollectionError::Work)?;
        let p = self.search.work;
        let mul = |v: u64| v.checked_mul(count).ok_or(PortfolioCollectionError::Work);
        Ok(PortfolioWorkLimits {
            point_checks: mul(p.point_checks)?,
            selection_checks: mul(p.selection_checks)?,
            rank_sites: mul(p.rank_sites)?,
            pixel_reads: mul(p.pixel_reads)?,
            geometry_hypotheses: mul(p.geometry_hypotheses)?,
            model_input_reads: mul(p.model_input_reads)?,
        })
    }
}

pub type RecoveryCollectionReport =
    crate::collection::CollectionReport<crate::search_portfolio::SearchRecoveryOutcome>;

/// Compare every unordered pair with primary search and opposite-native
/// recovery. An optional complete schedule prioritizes hints without pruning
/// missing pairs; publication remains canonical by application ID.
/// Insufficient geometry is retained as unknown. All other failures discard the
/// entire report, including earlier support. Sources, dependencies, weights and
/// cancellation remain guarded through final publication.
/// Managed storage/work admission excludes provider tensors and total RSS.
/// # Errors
/// Policy, cumulative work, file/pair/managed memory limits, schedule mismatch,
/// changed inputs/weights, provider failures and cancellation.
#[allow(clippy::too_many_arguments)]
pub fn scan_recovery_collection_with(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: Option<&crate::pair_schedule::CompletePairSchedule>,
    weights: &Path,
    policy: RecoveryCollectionPolicy,
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
) -> Result<RecoveryCollectionReport, PortfolioCollectionError> {
    scan_recovery_collection_selected(
        files, schedule, weights, policy, factors, budget, None, None, provider, cancel,
    )
}

/// Exhaustive primary and joint-opposite search with checked pair and collection
/// projection ceilings, admitted before consuming the input iterator.
#[allow(clippy::too_many_arguments)]
pub fn scan_joint_recovery_collection_with(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: Option<&crate::pair_schedule::CompletePairSchedule>,
    weights: &Path,
    policy: RecoveryCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    maximum_pair_projection_checks: u64,
    maximum_total_projection_checks: u64,
    provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<RecoveryCollectionReport, PortfolioCollectionError> {
    scan_recovery_collection_selected(
        files,
        schedule,
        weights,
        policy,
        factors,
        budget,
        Some((
            maximum_pair_projection_checks,
            maximum_total_projection_checks,
        )),
        None,
        provider,
        cancel,
    )
}

#[allow(clippy::too_many_arguments)]
fn scan_recovery_collection_selected(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: Option<&crate::pair_schedule::CompletePairSchedule>,
    weights: &Path,
    policy: RecoveryCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    joint_projection_checks: Option<(u64, u64)>,
    bank_model:Option<(crate::joint_witnesses::JointBankPolicy,crate::model_tiles::ModelTileWorkLimits,&[u32],ModelBankCollectionWorkLimits)>,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<RecoveryCollectionReport, PortfolioCollectionError> {
    scan_recovery_collection_engine(files,schedule,weights,policy,factors,budget,
        joint_projection_checks,bank_model,
        |left,right,cancel| crate::search_portfolio::search_recovery_files_selected(
            left,right,weights,policy.search,factors,budget,
            joint_projection_checks.map(|p|p.0),
            bank_model.map(|(banks,work,radii,_)|(banks,work,radii)),
            &mut provider,cancel),cancel)
}

fn scan_recovery_collection_engine<E>(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    schedule: Option<&crate::pair_schedule::CompletePairSchedule>,
    weights: &Path,
    policy: RecoveryCollectionPolicy,
    factors: &[f64],
    budget: &MemoryBudget,
    joint_projection_checks: Option<(u64, u64)>,
    bank_model:Option<(crate::joint_witnesses::JointBankPolicy,crate::model_tiles::ModelTileWorkLimits,&[u32],ModelBankCollectionWorkLimits)>,
    mut pair_search: impl FnMut(&DecodeRequest,&DecodeRequest,&dyn Fn()->bool)
        ->Result<E,SearchPortfolioError>,
    cancel: impl Fn() -> bool,
) -> Result<crate::collection::CollectionReport<E>, PortfolioCollectionError> {
    use crate::search_portfolio::{
        validate_search_recovery_policy,
    };
    scan_guarded_collection_ordered(
        files,
        weights,
        |cancel| {
            let bytes=if let Some((banks,work,radii,total))=bank_model {
                let bytes=crate::search_portfolio::validate_model_bank_recovery_policy(policy.search,banks,work,radii,factors,budget,&cancel)
                    .map_err(EngineError::Policy)?;
                let count=u64::try_from(policy.max_pairs).map_err(|_|EngineError::Work)?;
                for (pair,total) in [(banks.maximum_total_projection_checks,total.projection_checks),
                    (banks.maximum_total_separation_checks,total.separation_checks)] {
                    if pair.checked_mul(count).is_none_or(|required|required>total) {return Err(EngineError::Work);}
                }
                bytes
            } else {
                validate_search_recovery_policy(policy.search,factors,&cancel).map_err(EngineError::Policy)?
            };
            if let Some((pair, total)) = joint_projection_checks {
                crate::anchor_rank_file::validate_joint_opposite_projection_policy(
                    policy.search.opposite,
                    pair,
                    &cancel,
                )
                .map_err(crate::search_portfolio::SearchPortfolioError::from)
                .map_err(EngineError::Policy)?;
                let count = u64::try_from(policy.max_pairs).map_err(|_| EngineError::Work)?;
                if pair.checked_mul(count).ok_or(EngineError::Work)? > total {
                    return Err(EngineError::Work);
                }
            }
            let required = policy.required_work().map_err(|_| EngineError::Work)?;
            if [
                (required.point_checks, policy.work.point_checks),
                (required.selection_checks, policy.work.selection_checks),
                (required.rank_sites, policy.work.rank_sites),
                (required.pixel_reads, policy.work.pixel_reads),
                (
                    required.geometry_hypotheses,
                    policy.work.geometry_hypotheses,
                ),
                (required.model_input_reads, policy.work.model_input_reads),
            ]
            .iter()
            .any(|(required, limit)| required > limit)
            {
                return Err(EngineError::Work);
            }
            // Probe maximum recovery storage before consuming the input iterator.
            // Pair execution reserves it again alongside retained report evidence.
            drop(budget.try_reserve(bytes).map_err(EngineError::Memory)?);
            let primary = policy
                .search
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
            let opposite = policy
                .search
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
            Ok(CollectionSettings {
                max_files: policy.max_files,
                max_pairs: policy.max_pairs,
                max_model_bytes: policy.search.primary.model.max_model_bytes,
                max_file_bytes: primary
                    .max_file_bytes
                    .min(opposite.max_file_bytes)
                    .min(policy.search.primary.model.decode.max_file_bytes),
            })
        },
        budget,
        |left,right,cancel| pair_search(left,right,cancel),
        schedule,
        cancel,
    )
    .map_err(PortfolioCollectionError::from)
}

/// Exhaust every collection pair with joint model geometry. Projection work for
/// all admitted pairs and model windows is checked before consuming the iterator.
/// Typed insufficient geometry remains unknown; fatal errors discard the batch.
#[allow(clippy::too_many_arguments)]
pub fn scan_joint_model_portfolio_outcome_collection_with(
    files: impl IntoIterator<Item = (u64, DecodeRequest)>,
    weights: &Path,
    policy: PortfolioCollectionPolicy,
    maximum_model_projection_checks:u64,
    maximum_pair_projection_checks:u64,
    maximum_total_projection_checks:u64,
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
) -> Result<PortfolioOutcomeCollectionReport, PortfolioCollectionError> {
    scan_collection_with_outcome(
        files,
        weights,
        policy,
        factors,
        budget,
        provider,
        cancel,
        None,
        Some((maximum_model_projection_checks,maximum_pair_projection_checks,maximum_total_projection_checks)),
        None,
        |result| {
            result
                .map(PortfolioPairOutcome::Complete)
                .or_else(|error| match error {
                    SearchPortfolioError::InsufficientAnchorGeometry(reason) => {
                        Ok(PortfolioPairOutcome::Inconclusive(reason))
                    }
                    other => Err(other),
                })
        },
    )
}

/// Prepaid joint fitting/witness work across all admitted collection pairs.
#[derive(Debug,Clone,Copy)]
pub struct ModelBankCollectionWorkLimits {
    pub projection_checks:u64,
    pub separation_checks:u64,
}

/// Exhaust every pair with managed learned bank aggregation and bounded pixel
/// coverage. Full pair/radius work is admitted before consuming the iterator.
/// Optional complete scheduling changes order only; records use canonical IDs.
/// Insufficient evidence remains unknown; fatal errors discard the whole report.
#[allow(clippy::too_many_arguments)]
pub fn scan_model_bank_portfolio_outcome_collection_with(
    files:impl IntoIterator<Item=(u64,DecodeRequest)>,
    schedule:Option<&crate::pair_schedule::CompletePairSchedule>,
    weights:&Path,policy:PortfolioCollectionPolicy,
    banks:crate::joint_witnesses::JointBankPolicy,
    work:crate::model_tiles::ModelTileWorkLimits,radii:&[u32],
    total:ModelBankCollectionWorkLimits,
    factors:&[f64],budget:&MemoryBudget,
    provider:impl FnMut(&Path,&GrayModelInput,&GrayModelInput,&MemoryBudget,&dyn Fn()->bool)
        ->Result<SharedBuffer<ScoredProposal>,ProposalProviderError>,
    cancel:impl Fn()->bool,
)->Result<PortfolioOutcomeCollectionReport,PortfolioCollectionError> {
    scan_collection_with_outcome(files,weights,policy,factors,budget,provider,cancel,schedule,
        None,Some((banks,work,radii,total)),|result|result.map(PortfolioPairOutcome::Complete).or_else(|e|match e {
            SearchPortfolioError::InsufficientAnchorGeometry(reason)=>Ok(PortfolioPairOutcome::Inconclusive(reason)),
            other=>Err(other),
        }))
}

/// Exhaustive or complete scheduled collection: bank primary then joint opposite
/// recovery. All bank/pair/opposite work and storage precede iterator consumption.
/// Fatal errors discard the entire report; unresolved support remains unknown.
#[allow(clippy::too_many_arguments)]
pub fn scan_model_bank_joint_recovery_collection_with(
    files:impl IntoIterator<Item=(u64,DecodeRequest)>,
    schedule:Option<&crate::pair_schedule::CompletePairSchedule>,weights:&Path,
    policy:RecoveryCollectionPolicy,banks:crate::joint_witnesses::JointBankPolicy,
    work:crate::model_tiles::ModelTileWorkLimits,radii:&[u32],total:ModelBankCollectionWorkLimits,
    maximum_pair_projection_checks:u64,maximum_total_projection_checks:u64,
    factors:&[f64],budget:&MemoryBudget,
    provider:impl FnMut(&Path,&GrayModelInput,&GrayModelInput,&MemoryBudget,&dyn Fn()->bool)
        ->Result<SharedBuffer<ScoredProposal>,ProposalProviderError>,cancel:impl Fn()->bool,
)->Result<RecoveryCollectionReport,PortfolioCollectionError> {
    scan_recovery_collection_selected(files,schedule,weights,policy,factors,budget,
        Some((maximum_pair_projection_checks,maximum_total_projection_checks)),
        Some((banks,work,radii,total)),provider,cancel)
}

/// Exhaust every scheduled or canonical pair using native, joint opposite, then
/// model banks. All work is admitted before input iteration. Pair stage traces
/// and typed unknowns survive publication; fatal failures discard the batch.
#[allow(clippy::too_many_arguments)]
pub fn scan_native_bank_recovery_collection_with(
    files:impl IntoIterator<Item=(u64,DecodeRequest)>,
    schedule:Option<&crate::pair_schedule::CompletePairSchedule>,weights:&Path,
    policy:RecoveryCollectionPolicy,banks:crate::joint_witnesses::JointBankPolicy,
    work:crate::model_tiles::ModelTileWorkLimits,radii:&[u32],total:ModelBankCollectionWorkLimits,
    maximum_pair_projection_checks:u64,maximum_total_projection_checks:u64,
    factors:&[f64],budget:&MemoryBudget,
    mut provider:impl FnMut(&Path,&GrayModelInput,&GrayModelInput,&MemoryBudget,&dyn Fn()->bool)
        ->Result<SharedBuffer<ScoredProposal>,ProposalProviderError>,cancel:impl Fn()->bool,
)->Result<crate::collection::CollectionReport<crate::search_portfolio::NativeBankRecoveryEvidence>,PortfolioCollectionError> {
    scan_recovery_collection_engine(files,schedule,weights,policy,factors,budget,
        Some((maximum_pair_projection_checks,maximum_total_projection_checks)),
        Some((banks,work,radii,total)),
        |left,right,cancel| crate::search_portfolio::search_native_bank_recovery_files_with(
            left,right,weights,policy.search,banks,work,radii,maximum_pair_projection_checks,
            factors,budget,&mut provider,cancel),cancel)
}
