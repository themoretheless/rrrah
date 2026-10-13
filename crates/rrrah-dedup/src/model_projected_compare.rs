//! Explicit projected model candidates confirmed against retained original RGBA.
use crate::{
    animated::AnimationBudget,
    exact::{ContentSnapshot, SnapshotError},
    geometry::{
        verify_projective_sampled_for_domains, GeometryError, GeometryPolicy, ProjectiveSamplingPolicy,
        ProjectiveTransform,
    },
    model_compare::ProposalProviderError,
    model_input::{GrayModelInput, ModelInputPolicy},
    model_input_file::{prepare_file_retaining_original, ModelFileError, PreparedModelFile},
    model_projection::ModelInputProjection,
    proposals::{
        select_scored_region_proposals_managed, ProposalDomain, ProposalPolicy, RegionProposalDomain,
        ScoredProposal,
    },
    raster::NormalizedRaster,
    warp::{verify_projective_bidirectional, BidirectionalEvidence, WarpError, WarpPolicy},
};
use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::{DecodeRequest, GenerationToken};
use std::{cell::Cell, path::Path};

#[derive(Debug, Clone, Copy)]
pub struct ProjectedModelPolicy {
    pub decode: AnimationBudget,
    pub input: ModelInputPolicy,
    pub projection: ModelInputProjection,
    pub proposals: ProposalPolicy,
    pub geometry: GeometryPolicy,
    pub sampling: ProjectiveSamplingPolicy,
    /// Cumulative original-grid work across at most two verified mappings.
    pub pixels: WarpPolicy,
    /// Required compared/source fraction in each direction, in (0, 1].
    pub minimum_coverage: f64,
    /// Required matched/compared fraction in each direction, in (0, 1].
    pub minimum_agreement: f64,
    pub max_model_bytes: u64,
}
#[derive(Debug, thiserror::Error)]
pub enum ProjectedModelError {
    #[error("projected comparison cancelled")]
    Cancelled,
    #[error("invalid projected comparison policy")]
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
    Pixels(#[from] WarpError),
}
#[derive(Debug, Clone, Copy)]
pub struct ProjectedModelEvidence {
    pub original_dimensions: [[u32; 2]; 2],
    pub projection: ModelInputProjection,
    pub model_digest: [u8; 32],
    pub raw_proposals: usize,
    pub selected_points: usize,
    pub transform: Option<ProjectiveTransform>,
    pub geometry_inliers: usize,
    pub pixel_attempts: u32,
    pub pixel_work: u64,
    pub pixels: Option<BidirectionalEvidence>,
    /// Regional visual support under this recipe; never whole-file identity.
    pub supported: bool,
}
pub(crate) fn validate(p: ProjectedModelPolicy) -> Result<(), ProjectedModelError> {
    if p.max_model_bytes == 0
        || p.decode.max_frames == 0
        || p.decode.max_pixels == 0
        || p.decode.max_file_bytes == 0
        || p.input.max_side < 8
        || p.input.max_source_pixels == 0
        || p.input.max_output_pixels == 0
        || p.input.max_pixel_reads == 0
        || p.projection.validate().is_err()
        || p.proposals.max_proposals > p.geometry.max_points
        || !p.proposals.minimum_separation.is_finite()
        || p.proposals.minimum_separation < 0.
        || !p.proposals.minimum_separation.powi(2).is_finite()
        || !p.geometry.tolerance.is_finite()
        || p.geometry.tolerance <= 0.
        || !p.geometry.tolerance.powi(2).is_finite()
        || p.geometry.min_inliers < 4
        || p.geometry.min_inliers > p.geometry.max_points
        || p.sampling.trials == 0
        || p.sampling.trials > p.geometry.max_hypotheses
        || p.sampling.seed == 0
        || !p.pixels.tolerance.is_finite()
        || p.pixels.tolerance < 0.
        || p.pixels.max_source_pixels == 0
        || [p.minimum_coverage, p.minimum_agreement]
            .iter()
            .any(|x| !x.is_finite() || *x <= 0. || *x > 1.)
    {
        return Err(ProjectedModelError::Policy);
    }
    Ok(())
}

/// Decode each selected frame once, generate projected model inputs, fit native
/// projective geometry, and verify both original linear premultiplied-RGBA grids.
/// Both file/dependency observations, request generations and weights span the
/// operation. Equal projected bytes and identity-inlier support nominate an
/// additional exact identity mapping, always confirmed against original pixels.
/// At most two mappings are verified against one cumulative pixel-work cap.
/// A last caller checkpoint is followed by verification without caller
/// callbacks, so a mutation there cannot publish stale evidence.
///
/// Managed originals, inputs and proposals share `budget`; arbitrary provider
/// inference tensors/workspaces and geometry allocator overhead remain outside
/// that account. The provider must cooperate with cancellation. No download occurs.
///
/// # Errors
/// Invalid pre-I/O policy, resource/work refusal, source/model changes, provider
/// failure, invalid geometry/pixels or cancellation; no partial evidence escapes.
#[allow(clippy::too_many_arguments)]
pub fn compare_projected_model_files_with(
    left: &DecodeRequest,
    right: &DecodeRequest,
    weights: &Path,
    policy: ProjectedModelPolicy,
    budget: &MemoryBudget,
    mut provider: impl FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>,
    cancel: impl Fn() -> bool,
) -> Result<ProjectedModelEvidence, ProjectedModelError> {
    let latch = Cell::new(false);
    let owners = || {
        let stop = latch.get()
            || [left, right]
                .iter()
                .any(|r| r.cancellation.as_ref().is_some_and(GenerationToken::is_cancelled));
        latch.set(stop);
        stop
    };
    let cancelled = || {
        let stop = owners() || cancel();
        latch.set(stop);
        stop
    };
    if cancelled() {
        return Err(ProjectedModelError::Cancelled);
    }
    validate(policy)?;
    let result: Result<_, ProjectedModelError> = (|| {
        let model = ContentSnapshot::read(weights, policy.max_model_bytes, &cancelled)?;
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
        let evidence = compare_prepared_projected_files(
            ProjectedPreparedPair {
                files: [&a, &b],
                originals: [&original_a, &original_b],
                inputs: [a.input(), b.input()],
                rectangles: [a.rectangle(), b.rectangle()],
            },
            weights,
            &model,
            policy,
            budget,
            &mut provider,
            &cancelled,
        )?;
        Ok((evidence, a, b, model))
    })();
    if cancelled() {
        return Err(ProjectedModelError::Cancelled);
    }
    let (evidence, a, b, model) = result?;
    let verified = (|| {
        a.verify_source(&owners).map_err(ProjectedModelError::Left)?;
        b.verify_source(&owners).map_err(ProjectedModelError::Right)?;
        model.verify(&owners)?;
        Ok(evidence)
    })();
    if owners() {
        Err(ProjectedModelError::Cancelled)
    } else {
        verified
    }
}

pub(crate) struct ProjectedPreparedPair<'a> {
    pub files: [&'a PreparedModelFile; 2],
    pub originals: [&'a NormalizedRaster; 2],
    pub inputs: [&'a GrayModelInput; 2],
    pub rectangles: [[u32; 4]; 2],
}
pub(crate) type ProjectedProvider<'a> = dyn FnMut(
        &Path,
        &GrayModelInput,
        &GrayModelInput,
        &MemoryBudget,
        &dyn Fn() -> bool,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError>
    + 'a;

pub(crate) fn compare_prepared_projected_files(
    pair: ProjectedPreparedPair<'_>,
    weights: &Path,
    model: &ContentSnapshot,
    policy: ProjectedModelPolicy,
    budget: &MemoryBudget,
    provider: &mut ProjectedProvider<'_>,
    cancel: &dyn Fn() -> bool,
) -> Result<ProjectedModelEvidence, ProjectedModelError> {
    let [a, b] = pair.files;
    let [original_a, original_b] = pair.originals;
    let da = a.original_dimensions();
    let db = b.original_dimensions();
    let work = (u64::from(da[0]) * u64::from(da[1]))
        .checked_add(u64::from(db[0]) * u64::from(db[1]))
        .ok_or(WarpError::Budget)?;
    if work > policy.pixels.max_source_pixels {
        return Err(WarpError::Budget.into());
    }
    let verify = || -> Result<(), ProjectedModelError> {
        a.verify_source(&cancel).map_err(ProjectedModelError::Left)?;
        b.verify_source(&cancel).map_err(ProjectedModelError::Right)?;
        model.verify(&cancel)?;
        Ok(())
    };
    verify()?;
    let raw = provider(weights, pair.inputs[0], pair.inputs[1], budget, &cancel)
        .map_err(ProjectedModelError::Provider)?;
    if cancel() {
        return Err(ProjectedModelError::Cancelled);
    }
    verify()?;
    let domain = |index: usize| RegionProposalDomain {
        domain: ProposalDomain {
            input: pair.inputs[index].dimensions,
            original: pair.inputs[index].original,
        },
        origin: [pair.rectangles[index][0], pair.rectangles[index][1]],
        full: pair.files[index].original_dimensions(),
    };
    let selected = select_scored_region_proposals_managed(
        &raw,
        domain(0),
        domain(1),
        policy.proposals,
        budget,
        &cancel,
    )?;
    let raw_proposals = raw.len();
    drop(raw);
    let geometry = verify_projective_sampled_for_domains(
        &selected,
        policy.geometry,
        policy.sampling,
        (da[0], da[1]),
        (db[0], db[1]),
        &cancel,
    )?;
    let mut evidence = ProjectedModelEvidence {
        original_dimensions: [da, db],
        projection: policy.projection,
        model_digest: model.digest(),
        raw_proposals,
        selected_points: selected.len(),
        transform: None,
        geometry_inliers: 0,
        pixel_attempts: 0,
        pixel_work: 0,
        pixels: None,
        supported: false,
    };
    if let Some(g) = geometry {
        let va = original_a.view(&cancel).map_err(|e| {
            ProjectedModelError::Left(ModelFileError::Decode(Box::new(
                crate::decode::CachedError::Normalize(e),
            )))
        })?;
        let vb = original_b.view(&cancel).map_err(|e| {
            ProjectedModelError::Right(ModelFileError::Decode(Box::new(
                crate::decode::CachedError::Normalize(e),
            )))
        })?;
        let identity = ProjectiveTransform {
            matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        };
        let mut identity_inliers = 0;
        // Equal projected inputs nominate identity, never prove it. This
        // avoids interpolating a sharp HDR edge through a noisy self-fit.
        let mut same_input = da == db
            && pair.rectangles[0] == pair.rectangles[1]
            && pair.inputs[0].dimensions == pair.inputs[1].dimensions;
        if same_input {
            for (left, right) in pair.inputs[0].pixels.iter().zip(pair.inputs[1].pixels.iter()) {
                if cancel() {
                    return Err(ProjectedModelError::Cancelled);
                }
                if left != right {
                    same_input = false;
                    break;
                }
            }
        }
        if same_input {
            let tolerance = policy.geometry.tolerance.powi(2);
            for point in selected.iter() {
                if cancel() {
                    return Err(ProjectedModelError::Cancelled);
                }
                let dx = point.source[0] - point.target[0];
                let dy = point.source[1] - point.target[1];
                if dx * dx + dy * dy <= tolerance {
                    identity_inliers += 1;
                }
            }
        }
        let identity_candidate = identity_inliers >= policy.geometry.min_inliers;
        let candidates = [
            identity_candidate.then_some((identity, identity_inliers)),
            (!identity_candidate || g.transform != identity).then_some((g.transform, g.inliers.len())),
        ];
        for (transform, inliers) in candidates.into_iter().flatten() {
            let next_work = evidence.pixel_work.checked_add(work).ok_or(WarpError::Budget)?;
            if next_work > policy.pixels.max_source_pixels {
                return Err(WarpError::Budget.into());
            }
            let pixels = verify_projective_bidirectional(
                &va,
                &vb,
                transform,
                WarpPolicy {
                    max_source_pixels: work,
                    ..policy.pixels
                },
                &cancel,
            )?;
            evidence.pixel_attempts += 1;
            evidence.pixel_work = next_work;
            evidence.supported = [pixels.forward, pixels.reverse].iter().all(|p| {
                p.compared_pixels > 0
                    && p.source_pixels > 0
                    && p.compared_pixels as f64 / p.source_pixels as f64 >= policy.minimum_coverage
                    && p.matched_pixels as f64 / p.compared_pixels as f64 >= policy.minimum_agreement
            });
            evidence.transform = Some(transform);
            evidence.geometry_inliers = inliers;
            evidence.pixels = Some(pixels);
            if evidence.supported {
                break;
            }
        }
    }
    drop(selected);
    verify()?;
    Ok(evidence)
}
