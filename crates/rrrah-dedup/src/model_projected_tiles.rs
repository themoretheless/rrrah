//! Automatic projected model regions with retained original RGBA confirmation.
use crate::{
    exact::ContentSnapshot,
    model_compare::ProposalProviderError,
    model_input::{GrayModelInput, prepare_gray_model_region_input_with_projection},
    model_input_file::{ModelFileError, prepare_file_retaining_original},
    model_projected_compare::{
        ProjectedModelError, ProjectedModelEvidence, ProjectedModelPolicy, ProjectedPreparedPair,
        compare_prepared_projected_files, validate,
    },
    model_tiles::{MAX_MODEL_TILE_ATTEMPTS, owners_cancelled, regions},
    proposals::ScoredProposal,
};
use rrrah_core::{BufferError, MemoryBudget, SharedBuffer};
use rrrah_decode::DecodeRequest;
use std::{cell::Cell, path::Path};

/// Checked cumulative maxima for ten lazily prepared inputs and 25 comparisons.
/// Provider tensors, workspaces, allocator overhead and total RSS are excluded.
#[derive(Debug, Clone, Copy)]
pub struct ProjectedTileWorkLimits {
    pub model_input_reads: u64,
    pub proposal_ranking_checks: u64,
    pub proposal_separation_checks: u64,
    pub geometry_hypotheses: u64,
    pub original_pixel_work: u64,
    pub identity_input_checks: u64,
    pub identity_point_checks: u64,
}
impl ProjectedTileWorkLimits {
    /// Compute fixed-recipe maxima without relaxing constituent policy limits.
    /// # Errors
    /// Invalid policy or cumulative integer overflow.
    pub fn for_model(policy: ProjectedModelPolicy) -> Result<Self, ProjectedTileError> {
        validate(policy)?;
        let times = |value: u64, count: u64| value.checked_mul(count).ok_or(ProjectedTileError::Work);
        Ok(Self {
            model_input_reads: times(policy.input.max_pixel_reads, 10)?,
            proposal_ranking_checks: times(policy.proposals.max_ranking_checks, 25)?,
            proposal_separation_checks: times(policy.proposals.max_separation_checks, 25)?,
            geometry_hypotheses: times(policy.sampling.trials, 25)?,
            original_pixel_work: times(policy.pixels.max_source_pixels, 25)?,
            identity_input_checks: times(policy.input.max_output_pixels, 25)?,
            identity_point_checks: times(
                u64::try_from(policy.proposals.max_proposals).map_err(|_| ProjectedTileError::Work)?,
                25,
            )?,
        })
    }
    /// Prepay the same bounded recipe for every declared collection pair.
    /// # Errors
    /// Cumulative work multiplication overflow.
    pub fn for_pairs(self, count: usize) -> Result<Self, ProjectedTileError> {
        let count = u64::try_from(count).map_err(|_| ProjectedTileError::Work)?;
        let times = |value: u64| value.checked_mul(count).ok_or(ProjectedTileError::Work);
        Ok(Self {
            model_input_reads: times(self.model_input_reads)?,
            proposal_ranking_checks: times(self.proposal_ranking_checks)?,
            proposal_separation_checks: times(self.proposal_separation_checks)?,
            geometry_hypotheses: times(self.geometry_hypotheses)?,
            original_pixel_work: times(self.original_pixel_work)?,
            identity_input_checks: times(self.identity_input_checks)?,
            identity_point_checks: times(self.identity_point_checks)?,
        })
    }
    pub(crate) fn validate(self, policy: ProjectedModelPolicy) -> Result<(), ProjectedTileError> {
        let r = Self::for_model(policy)?;
        for (required, available) in [
            (r.model_input_reads, self.model_input_reads),
            (r.proposal_ranking_checks, self.proposal_ranking_checks),
            (r.proposal_separation_checks, self.proposal_separation_checks),
            (r.geometry_hypotheses, self.geometry_hypotheses),
            (r.original_pixel_work, self.original_pixel_work),
            (r.identity_input_checks, self.identity_input_checks),
            (r.identity_point_checks, self.identity_point_checks),
        ] {
            if required > available {
                return Err(ProjectedTileError::Work);
            }
        }
        Ok(())
    }
}
#[derive(Debug, thiserror::Error)]
pub enum ProjectedTileError {
    #[error("projected region search cancelled")]
    Cancelled,
    #[error("projected region cumulative work admission exceeded")]
    Work,
    #[error(transparent)]
    Comparison(#[from] ProjectedModelError),
    #[error(transparent)]
    Buffer(#[from] BufferError),
}
#[derive(Debug, Clone, Copy)]
pub struct ProjectedTileAttempt {
    pub rectangles: [[u32; 4]; 2],
    pub evidence: ProjectedModelEvidence,
}
#[derive(Debug, Clone)]
pub struct ProjectedTileEvidence {
    attempts: SharedBuffer<Option<ProjectedTileAttempt>>,
    len: usize,
    prepared_inputs: [u32; 2],
    pixel_work: u64,
}
impl ProjectedTileEvidence {
    /// Every attempted region pair, including negative original-pixel evidence.
    pub fn attempts(&self) -> impl ExactSizeIterator<Item = &ProjectedTileAttempt> {
        self.attempts[..self.len]
            .iter()
            .map(|a| a.as_ref().expect("initialized attempt"))
    }
    pub fn supported(&self) -> bool {
        self.attempts().last().is_some_and(|a| a.evidence.supported)
    }
    /// Number of distinct projected inputs prepared per source, including full.
    pub fn prepared_inputs(&self) -> [u32; 2] {
        self.prepared_inputs
    }
    /// Actual cumulative full-original warp work, including rejected mappings.
    pub fn pixel_work(&self) -> u64 {
        self.pixel_work
    }
}

/// Try full images, then every remaining source-major pair of full image and
/// nonempty quadrants, stopping only at original-RGBA support. Each source is
/// decoded once; each projected region is prepared lazily at most once. Neither
/// projected equality, confidence nor geometry alone establishes support.
///
/// Region proposals are mapped through crop origins to full original domains.
/// All confirmations retain the same complete normalized originals, coverage,
/// agreement and per-attempt cumulative mapping limits as explicit comparison.
/// Shared source/dependency/weights observations and both owner generations span
/// the operation. The final caller checkpoint precedes read-only revalidation.
/// Result ledger credit follows its last shared owner; errors publish no ledger.
/// This API does not claim whole-file identity, provider RSS or single-kernel
/// preemption. No model downloads occur.
/// # Errors
/// Invalid policy, work/payload refusal, decode/source/provider/geometry/pixel
/// failure or sticky cancellation. Typed failures remain terminal.
#[allow(clippy::too_many_arguments)]
pub fn search_projected_model_tiles_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: ProjectedModelPolicy,
    work: ProjectedTileWorkLimits,
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<ProjectedTileEvidence, ProjectedTileError> {
    let latch = Cell::new(false);
    let owners = || owners_cancelled(left, right, &latch);
    let cancelled = || {
        let stopped = owners() || cancel();
        latch.set(stopped);
        stopped
    };
    let result: Result<_, ProjectedTileError> = (|| {
        if cancelled() {
            return Err(ProjectedTileError::Cancelled);
        }
        work.validate(policy)?;
        let mut attempts = budget.try_buffer(MAX_MODEL_TILE_ATTEMPTS, None)?;
        let model = ContentSnapshot::read(weights, policy.max_model_bytes, &cancelled)
            .map_err(ProjectedModelError::ModelSource)?;
        let (a, original_a) = prepare_file_retaining_original(
            left,
            policy.decode,
            policy.input,
            policy.projection,
            budget,
            &cancelled,
        )
        .map_err(ProjectedModelError::Left)?;
        let (b, original_b) = prepare_file_retaining_original(
            right,
            policy.decode,
            policy.input,
            policy.projection,
            budget,
            &cancelled,
        )
        .map_err(ProjectedModelError::Right)?;
        let verify = || -> Result<(), ProjectedModelError> {
            a.verify_source(&cancelled).map_err(ProjectedModelError::Left)?;
            b.verify_source(&cancelled).map_err(ProjectedModelError::Right)?;
            model.verify(&cancelled)?;
            Ok(())
        };
        let (lr, ln) = regions(a.original_dimensions());
        let (rr, rn) = regions(b.original_dimensions());
        let mut lc: [Option<GrayModelInput>; 5] = std::array::from_fn(|_| None);
        let mut rc: [Option<GrayModelInput>; 5] = std::array::from_fn(|_| None);
        let mut prepared_inputs = [1, 1];
        let mut len = 0;
        let mut pixel_work = 0u64;
        'pairs: for i in 0..ln {
            for j in 0..rn {
                verify()?;
                if i > 0 && lc[i].is_none() {
                    let view = original_a.view(&cancelled).map_err(|e| {
                        ProjectedModelError::Left(ModelFileError::Decode(Box::new(
                            crate::decode::CachedError::Normalize(e),
                        )))
                    })?;
                    lc[i] = Some(
                        prepare_gray_model_region_input_with_projection(
                            &view,
                            lr[i],
                            policy.input,
                            policy.projection,
                            budget,
                            &cancelled,
                        )
                        .map_err(ModelFileError::Input)
                        .map_err(ProjectedModelError::Left)?,
                    );
                    prepared_inputs[0] += 1;
                }
                if j > 0 && rc[j].is_none() {
                    let view = original_b.view(&cancelled).map_err(|e| {
                        ProjectedModelError::Right(ModelFileError::Decode(Box::new(
                            crate::decode::CachedError::Normalize(e),
                        )))
                    })?;
                    rc[j] = Some(
                        prepare_gray_model_region_input_with_projection(
                            &view,
                            rr[j],
                            policy.input,
                            policy.projection,
                            budget,
                            &cancelled,
                        )
                        .map_err(ModelFileError::Input)
                        .map_err(ProjectedModelError::Right)?,
                    );
                    prepared_inputs[1] += 1;
                }
                let evidence = compare_prepared_projected_files(
                    ProjectedPreparedPair {
                        files: [&a, &b],
                        originals: [&original_a, &original_b],
                        inputs: [
                            lc[i].as_ref().unwrap_or(a.input()),
                            rc[j].as_ref().unwrap_or(b.input()),
                        ],
                        rectangles: [lr[i], rr[j]],
                    },
                    weights,
                    &model,
                    policy,
                    budget,
                    &mut provider,
                    &cancelled,
                )?;
                pixel_work = pixel_work
                    .checked_add(evidence.pixel_work)
                    .ok_or(ProjectedTileError::Work)?;
                if pixel_work > work.original_pixel_work {
                    return Err(ProjectedTileError::Work);
                }
                let supported = evidence.supported;
                attempts[len] = Some(ProjectedTileAttempt {
                    rectangles: [lr[i], rr[j]],
                    evidence,
                });
                len += 1;
                if supported {
                    break 'pairs;
                }
            }
        }
        verify()?;
        Ok((
            ProjectedTileEvidence {
                attempts: attempts.freeze(),
                len,
                prepared_inputs,
                pixel_work,
            },
            a,
            b,
            model,
        ))
    })();
    if cancelled() {
        return Err(ProjectedTileError::Cancelled);
    }
    let (evidence, a, b, model) = result?;
    let verified = (|| -> Result<_, ProjectedTileError> {
        a.verify_source(&owners).map_err(ProjectedModelError::Left)?;
        b.verify_source(&owners).map_err(ProjectedModelError::Right)?;
        model.verify(&owners).map_err(ProjectedModelError::ModelSource)?;
        Ok(evidence)
    })();
    if owners() {
        Err(ProjectedTileError::Cancelled)
    } else {
        verified
    }
}
