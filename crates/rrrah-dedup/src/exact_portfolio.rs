//! File-byte identity first, followed by independently guarded visual evidence.
use crate::{
    exact::{ContentSnapshot, compare_observed},
    model_compare::ProposalProviderError,
    model_input::GrayModelInput,
    model_tiles::owners_cancelled,
    projected_portfolio::{
        ProjectedPortfolioError, ProjectedPortfolioEvidence, ProjectedPortfolioPolicy,
        search_projected_portfolio_files_with, validate_projected_portfolio,
    },
    proposals::ScoredProposal,
};
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::{DecodeRequest, GenerationToken};
use std::{cell::Cell, path::Path};

/// Exact file identity and regional visual support are distinct evidence classes.
#[derive(Debug)]
pub enum ExactPortfolioEvidence {
    ExactBytes,
    Visual(ProjectedPortfolioEvidence),
}

/// Confirm file bytes before attempting the complete visual portfolio.
///
/// Validates all visual work policies before I/O. Exact copies require neither
/// decodable pixels nor local weights, and do not invoke the provider. A byte
/// mismatch starts the existing visual source/dependency/weight guards. Original
/// file observations span both stages and are validated again before return.
/// No extra caller checkpoint follows visual publication, preserving its final
/// dependency/weight validation boundary. Request cancellation remains sticky.
///
/// ExactBytes refers only to file content; external palettes, decode settings and
/// other resources may still produce different pixels. Visual evidence retains
/// its existing regional scope. Neither is an atomic filesystem snapshot.
/// # Errors
/// Work/policy refusal, source mutation, cancellation, or visual-stage errors.
#[allow(clippy::too_many_arguments)]
pub fn search_exact_projected_portfolio_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: ProjectedPortfolioPolicy,
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
) -> Result<ExactPortfolioEvidence, ProjectedPortfolioError> {
    let latch = Cell::new(false);
    let cancelled = || {
        let stop = latch.get() || cancel() || [left, right].iter().any(|r| {
            r.cancellation.as_ref().is_some_and(GenerationToken::is_cancelled)
        });
        latch.set(stop);
        stop
    };
    let result = (|| {
        if cancelled() {
            return Err(ProjectedPortfolioError::Cancelled);
        }
        validate_projected_portfolio(policy, factors, &cancelled)?;
        let native = policy.native.search.search.search.search.gradient.search.local.decode;
        let max_bytes = native.max_file_bytes.min(policy.model.decode.max_file_bytes);
        let first = ContentSnapshot::read(&left.path, max_bytes, &cancelled)?;
        let second = ContentSnapshot::read(&right.path, max_bytes, &cancelled)?;
        let evidence = if compare_observed(&first, &second, &cancelled)? {
            if cancelled() {
                return Err(ProjectedPortfolioError::Cancelled);
            }
            ExactPortfolioEvidence::ExactBytes
        } else {
            ExactPortfolioEvidence::Visual(search_projected_portfolio_files_with(
                left, right, weights, policy, factors, budget, provider, &cancelled,
            )?)
        };
        let owners = || owners_cancelled(left, right, &latch);
        let verified = (|| {
            first.verify(&owners)?;
            second.verify(&owners)?;
            Ok(evidence)
        })();
        if owners() { Err(ProjectedPortfolioError::Cancelled) } else { verified }
    })();
    // Error outcomes publish no evidence; allow cancellation to take priority.
    if result.is_err() && cancelled() {
        Err(ProjectedPortfolioError::Cancelled)
    } else {
        result
    }
}
