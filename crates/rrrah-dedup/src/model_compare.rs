//! One lifecycle for supplied model proposals, native geometry and pixel support.
//! Providers are responsible for inference allocations and cooperative cancellation.
use crate::{
    anchor_rank::{AnchorRankEvidence, AnchorRankPolicy, validate_anchor_rank_policy},
    anchor_rank_file::{AnchorRankFileError, compare_anchor_rank_files},
    animated::AnimationBudget,
    exact::{ContentSnapshot, SnapshotError},
    geometry::{
        GeometryError, GeometryPolicy, ProjectiveSamplingPolicy, ProjectiveTransform,
        verify_projective_sampled_for_domains, JointProjectivePolicy,
        verify_projective_sampled_joint_for_domains,
    },
    model_input::{GrayModelInput, ModelInputPolicy},
    model_input_file::{ModelFileError, PreparedModelFile, prepare_file},
    proposals::{
        ProposalDomain, ProposalPolicy, RegionProposalDomain, ScoredProposal,
        select_scored_region_proposals_managed,
    },
};
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::{DecodeRequest, GenerationToken};
use std::{cell::Cell, error::Error, ops::Deref, path::Path};

pub type ProposalProviderError = Box<dyn Error + Send + Sync>;
#[derive(Debug, Clone, Copy)]
pub struct ModelComparisonPolicy {
    pub decode: AnimationBudget,
    pub input: ModelInputPolicy,
    pub proposals: ProposalPolicy,
    pub geometry: GeometryPolicy,
    pub sampling: ProjectiveSamplingPolicy,
    pub anchors: AnchorRankPolicy,
    pub max_model_bytes: u64,
}
/// Explicit two-direction geometry and prepaid worst-case projection work.
/// Pixel confirmation and provider/source lifecycle use the comparison policy.
#[derive(Debug, Clone, Copy)]
pub struct JointModelComparisonPolicy {
    pub comparison: ModelComparisonPolicy,
    pub maximum_projection_checks: u64,
}
#[derive(Debug, thiserror::Error)]
pub enum ModelComparisonError {
    #[error("model comparison cancelled")]
    Cancelled,
    #[error("invalid model comparison policy")]
    Policy,
    #[error(transparent)]
    ModelSource(#[from] SnapshotError),
    #[error("left input: {0}")]
    Left(#[source] ModelFileError),
    #[error("right input: {0}")]
    Right(#[source] ModelFileError),
    #[error("proposal provider: {0}")]
    Provider(#[source] ProposalProviderError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error(transparent)]
    Pixels(#[from] AnchorRankFileError),
}
#[derive(Debug, Clone, Copy)]
pub struct ModelComparisonEvidence {
    /// Complete oriented domains, even when inference used explicit crops.
    pub original_dimensions: [[u32; 2]; 2],
    pub model_digest: [u8; 32],
    pub raw_proposals: usize,
    pub selected_points: usize,
    pub transform: Option<ProjectiveTransform>,
    pub geometry_inliers: usize,
    pub hypotheses: u64,
    pub pixels: Option<AnchorRankEvidence>,
}
impl ModelComparisonEvidence {
    pub fn supported(&self) -> bool {
        self.pixels.as_ref().is_some_and(|p| p.supported)
    }
}

pub(crate) fn validate_model_comparison_policy(
    policy: ModelComparisonPolicy,
) -> Result<(), ModelComparisonError> {
    if policy.max_model_bytes == 0
        || policy.decode.max_frames == 0
        || policy.decode.max_pixels == 0
        || policy.decode.max_file_bytes == 0
        || policy.input.max_side < 8
        || policy.input.max_source_pixels == 0
        || policy.input.max_output_pixels == 0
        || policy.input.max_pixel_reads == 0
        || !policy.proposals.minimum_separation.is_finite()
        || policy.proposals.minimum_separation < 0.0
        || !policy.proposals.minimum_separation.powi(2).is_finite()
        || policy.proposals.max_proposals > policy.geometry.max_points
        || policy.proposals.max_proposals > policy.anchors.local.maximum_points
        || !policy.geometry.tolerance.is_finite()
        || policy.geometry.tolerance <= 0.0
        || !policy.geometry.tolerance.powi(2).is_finite()
        || policy.geometry.min_inliers < 4
        || policy.geometry.min_inliers > policy.geometry.max_points
        || policy.sampling.trials == 0
        || policy.sampling.trials > policy.geometry.max_hypotheses
        || policy.sampling.seed == 0
        || validate_anchor_rank_policy(policy.anchors).is_err()
    {
        return Err(ModelComparisonError::Policy);
    }
    Ok(())
}

/// Prepare both selected frames, call a model provider, select correspondences,
/// fit full-domain projective geometry and confirm original pixels. Original
/// source/dependency snapshots and the model file span the entire operation,
/// including fresh pixel confirmation. Both request generations and global
/// cancellation are sticky. Source/model changes and provider errors never
/// become an ordinary miss; no partial evidence is published on failure.
///
/// The provider must use the supplied weight path, allocate retained proposals
/// against the supplied budget, and cooperate with the cancellation callback.
/// This entrypoint enforces output count/selection work and managed payload
/// ownership, but cannot enforce arbitrary provider inference allocations or
/// cancellation inside a blocking provider. Geometry scratch/allocator overhead
/// is also outside the managed proposal/input payload account. No download occurs.
///
/// # Errors
/// Invalid pre-I/O policy, changed inputs/model, provider failure, resource/work
/// refusal, invalid geometry or cancellation. Missing geometry is candidate-only
/// negative evidence for this provider/recipe, never universal non-duplication.
#[allow(clippy::too_many_arguments)]
pub fn compare_model_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: ModelComparisonPolicy,
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<ModelComparisonEvidence, ModelComparisonError> {
    compare_files(
        left,
        right,
        weights,
        None,
        None,
        policy,
        budget,
        &mut provider,
        &cancel,
        None,
        None,
        false,
).map(|(evidence, _)| evidence)
}

/// Compare two explicit original-pixel rectangles through a model provider,
/// while fitting geometry and confirming pixels in the full original domains.
/// Crop offsets are applied before distinctness, geometry and pixel admission.
/// Whole source/dependency snapshots and both request generations remain live.
/// This is local visual support, not whole-image identity or a crop-file API.
///
/// # Errors
/// Invalid rectangles/policy, source/model mutation, provider failure, resource
/// refusal, geometry/pixel failure or cancellation. No partial result escapes.
#[allow(clippy::too_many_arguments)]
pub fn compare_model_regions_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    left_rectangle: [u32; 4],
    right_rectangle: [u32; 4],
    policy: ModelComparisonPolicy,
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<ModelComparisonEvidence, ModelComparisonError> {
    compare_files(
        left,
        right,
        weights,
        Some(left_rectangle),
        Some(right_rectangle),
        policy,
        budget,
        &mut provider,
        &cancel,
        None,
        None,
        false,
).map(|(evidence, _)| evidence)
}

pub(crate) type Provider<'a> = dyn FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>
    + 'a;

// Private prepared-input path for one guarded tile operation. Borrowed files
// retain their original request tokens and complete dependency observations;
// the operation's original weights observation is reused, never refreshed to
// accept a different digest midway through the recipe.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_prepared_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    left_input: &PreparedModelFile,
    right_input: &PreparedModelFile,
    model: &ContentSnapshot,
    policy: ModelComparisonPolicy,
    budget: &MemoryBudget,
    provider: &mut Provider<'_>,
    cancel: &dyn Fn() -> bool,
    joint_projection_checks: Option<u64>,
) -> Result<ModelComparisonEvidence, ModelComparisonError> {
    compare_files(
        left,
        right,
        weights,
        Some(left_input.rectangle()),
        Some(right_input.rectangle()),
        policy,
        budget,
        provider,
        cancel,
        Some((left_input, right_input, model)),
        joint_projection_checks,
        false,
).map(|(evidence, _)| evidence)
}

enum Retained<'a, T> {
    Owned(T),
    Borrowed(&'a T),
}
impl<T> Deref for Retained<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self {
            Self::Owned(value) => value,
            Self::Borrowed(value) => value,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_files(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    left_rectangle: Option<[u32; 4]>,
    right_rectangle: Option<[u32; 4]>,
    policy: ModelComparisonPolicy,
    budget: &MemoryBudget,
    provider: &mut Provider<'_>,
    cancel: &dyn Fn() -> bool,
    prepared: Option<(&PreparedModelFile, &PreparedModelFile, &ContentSnapshot)>,
    joint_projection_checks: Option<u64>,
    capture_only: bool,
) -> Result<(ModelComparisonEvidence, Option<SharedBuffer<crate::geometry::Correspondence>>), ModelComparisonError> {
    let latch = Cell::new(false);
    let cancelled = || {
        let stop = latch.get()
            || cancel()
            || [left, right]
                .iter()
                .any(|r| r.cancellation.as_ref().is_some_and(GenerationToken::is_cancelled));
        latch.set(stop);
        stop
    };
    if cancelled() {
        return Err(ModelComparisonError::Cancelled);
    }
    validate_model_comparison_policy(policy)?;
    if let Some(limit) = joint_projection_checks {
        let source_tolerance2 = policy.anchors.local.source_tolerance.powi(2);
        if !source_tolerance2.is_finite() || source_tolerance2 <= 0. {
            return Err(ModelComparisonError::Policy);
        }
        let required = u64::try_from(policy.geometry.max_points).ok()
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| policy.sampling.trials.checked_add(1).and_then(|t| n.checked_mul(t)))
            .ok_or(ModelComparisonError::Geometry(GeometryError::Budget))?;
        if required > limit {
            return Err(ModelComparisonError::Geometry(GeometryError::Budget));
        }
    }
    for (rectangle, side) in [(left_rectangle, true), (right_rectangle, false)] {
        if rectangle.is_some_and(|[x, y, w, h]| {
            w == 0 || h == 0 || x.checked_add(w).is_none() || y.checked_add(h).is_none()
        }) {
            let error = ModelFileError::Input(crate::model_input::ModelInputError::Invalid);
            return Err(if side {
                ModelComparisonError::Left(error)
            } else {
                ModelComparisonError::Right(error)
            });
        }
    }
    let result = (|| {
        let model = if let Some((_, _, model)) = prepared {
            Retained::Borrowed(model)
        } else {
            Retained::Owned(ContentSnapshot::read(weights, policy.max_model_bytes, cancelled)?)
        };
        let left_input = if let Some((input, _, _)) = prepared {
            Retained::Borrowed(input)
        } else {
            Retained::Owned(
                prepare_file(
                    left,
                    left_rectangle,
                    policy.decode,
                    policy.input,
                    budget,
                    &cancelled,
                )
                .map_err(ModelComparisonError::Left)?,
            )
        };
        let right_input = if let Some((_, input, _)) = prepared {
            Retained::Borrowed(input)
        } else {
            Retained::Owned(
                prepare_file(
                    right,
                    right_rectangle,
                    policy.decode,
                    policy.input,
                    budget,
                    &cancelled,
                )
                .map_err(ModelComparisonError::Right)?,
            )
        };
        let verify = || -> Result<(), ModelComparisonError> {
            left_input
                .verify_source(cancelled)
                .map_err(ModelComparisonError::Left)?;
            right_input
                .verify_source(cancelled)
                .map_err(ModelComparisonError::Right)?;
            model.verify(cancelled)?;
            Ok(())
        };
        verify()?;
        let raw = provider(
            weights,
            left_input.input(),
            right_input.input(),
            budget,
            &cancelled,
        )
        .map_err(ModelComparisonError::Provider)?;
        if cancelled() {
            return Err(ModelComparisonError::Cancelled);
        }
        verify()?;
        let domain = |file: &PreparedModelFile| RegionProposalDomain {
            domain: ProposalDomain {
                input: file.input().dimensions,
                original: file.input().original,
            },
            origin: [file.rectangle()[0], file.rectangle()[1]],
            full: file.original_dimensions(),
        };
        let selected = select_scored_region_proposals_managed(
            &raw,
            domain(&left_input),
            domain(&right_input),
            policy.proposals,
            budget,
            cancelled,
        )?;
        let raw_proposals = raw.len();
        drop(raw);
        let ld = left_input.original_dimensions();
        let rd = right_input.original_dimensions();
        let geometry = if capture_only { None } else if let Some(maximum_projection_checks) = joint_projection_checks {
            verify_projective_sampled_joint_for_domains(
                &selected,
                JointProjectivePolicy {
                    target: policy.geometry,
                    source_tolerance: policy.anchors.local.source_tolerance,
                    maximum_projection_checks,
                },
                policy.sampling, (ld[0], ld[1]), (rd[0], rd[1]), cancelled,
            )?.map(|g| (g.transform, g.inliers.len(), g.hypotheses))
        } else {
            verify_projective_sampled_for_domains(
                &selected, policy.geometry, policy.sampling,
                (ld[0], ld[1]), (rd[0], rd[1]), cancelled,
            )?.map(|g| (g.transform, g.inliers.len(), g.hypotheses))
        };
        let mut evidence = ModelComparisonEvidence {
            original_dimensions: [ld, rd],
            model_digest: model.digest(),
            raw_proposals,
            selected_points: selected.len(),
            transform: None,
            geometry_inliers: 0,
            hypotheses: if !capture_only && selected.len() >= 4 {
                policy.sampling.trials
            } else {
                0
            },
            pixels: None,
        };
        if let Some((transform, inliers, hypotheses)) = geometry {
            evidence.transform = Some(transform);
            evidence.geometry_inliers = inliers;
            evidence.hypotheses = hypotheses;
            evidence.pixels = Some(compare_anchor_rank_files(
                left,
                right,
                transform,
                &selected,
                policy.decode,
                policy.anchors,
                budget,
                cancelled,
            )?);
        }
        let captured = if capture_only { Some(selected) } else { drop(selected); None };
        verify()?;
        Ok((evidence, left_input, right_input, model, captured))
    })();
    if cancelled() {
        return Err(ModelComparisonError::Cancelled);
    }
    let (evidence, left_input, right_input, model, captured) = result?;
    let owners = || {
        let stopped = latch.get()
            || [left, right].iter().any(|request| {
                request
                    .cancellation
                    .as_ref()
                    .is_some_and(GenerationToken::is_cancelled)
            });
        latch.set(stopped);
        stopped
    };
    // The last caller checkpoint may change a source. Read-only verification
    // after it never invokes caller code; both request generations stay live.
    let verified = (|| {
        left_input
            .verify_source(&owners)
            .map_err(ModelComparisonError::Left)?;
        right_input
            .verify_source(&owners)
            .map_err(ModelComparisonError::Right)?;
        model.verify(&owners)?;
        Ok((evidence, captured))
    })();
    if owners() {
        Err(ModelComparisonError::Cancelled)
    } else {
        verified
    }
}

/// Fit jointly in both original domains, with projection admission before I/O.
/// Original-pixel thresholds, source guards and cancellation remain mandatory.
/// Missing geometry is inconclusive; provider tensors and geometry scratch are
/// outside managed payload accounting. Legacy entrypoints retain their fitter.
#[allow(clippy::too_many_arguments)]
pub fn compare_model_joint_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: JointModelComparisonPolicy,
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<ModelComparisonEvidence, ModelComparisonError> {
    compare_files(
        left,
        right,
        weights,
        None,
        None,
        policy.comparison,
        budget,
        &mut provider,
        &cancel,
        None,
        Some(policy.maximum_projection_checks),
        false,
).map(|(evidence, _)| evidence)
}

/// Fit jointly in both original domains, with projection admission before I/O.
/// Original-pixel thresholds, source guards and cancellation remain mandatory.
/// Missing geometry is inconclusive; provider tensors and geometry scratch are
/// outside managed payload accounting. Legacy entrypoints retain their fitter.
#[allow(clippy::too_many_arguments)]
pub fn compare_model_joint_regions_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    left_rectangle: [u32; 4],
    right_rectangle: [u32; 4],
    policy: JointModelComparisonPolicy,
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<ModelComparisonEvidence, ModelComparisonError> {
    compare_files(
        left,
        right,
        weights,
        Some(left_rectangle),
        Some(right_rectangle),
        policy.comparison,
        budget,
        &mut provider,
        &cancel,
        None,
        Some(policy.maximum_projection_checks),
        false,
).map(|(evidence, _)| evidence)
}


/// Managed, original-coordinate model correspondences for geometric aggregation.
/// This value carries no geometry/pixel verdict. Files are observed throughout
/// extraction; a later operation must retain or revalidate its own source guards.
#[derive(Debug, Clone)]
pub struct ModelPointBank {
    pub original_dimensions: [[u32; 2]; 2],
    pub model_digest: [u8; 32],
    pub raw_proposals: usize,
    pub points: SharedBuffer<crate::geometry::Correspondence>,
}

/// Extract mapped/selected proposals without fitting or pixel reads. Uses the
/// same complete source/weights/generation guards and sticky cancellation as
/// comparison. Returned points retain managed capacity until the last clone.
/// Provider tensors/allocator scratch remain outside managed payload accounting.
#[allow(clippy::too_many_arguments)]
pub fn extract_model_region_points_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    left_rectangle: [u32; 4],
    right_rectangle: [u32; 4],
    policy: ModelComparisonPolicy,
    budget: &MemoryBudget,
    mut provider: impl FnMut(&Path, &GrayModelInput, &GrayModelInput, &MemoryBudget, &dyn Fn() -> bool)
        -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<ModelPointBank, ModelComparisonError> {
    let (evidence, points) = compare_files(left,right,weights,Some(left_rectangle),Some(right_rectangle),
        policy,budget,&mut provider,&cancel,None,None,true)?;
    Ok(ModelPointBank{original_dimensions:evidence.original_dimensions,model_digest:evidence.model_digest,
        raw_proposals:evidence.raw_proposals,points:points.ok_or(ModelComparisonError::Policy)?})
}
