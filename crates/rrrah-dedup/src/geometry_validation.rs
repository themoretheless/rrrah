//! Score a fixed projective model on a declared, coordinate-disjoint point set.
//! This validates geometry, not pixels, descriptor provenance or image identity.
use crate::geometry::{
    Correspondence, GeometryError, JointProjectiveEvidence, JointProjectivePolicy, ProjectiveSamplingPolicy,
    ProjectiveTransform, verify_projective_sampled_joint_for_domains,
};

#[derive(Debug, Clone, Copy)]
pub struct ProjectiveValidationPolicy {
    pub maximum_fitting_points: usize,
    pub maximum_validation_points: usize,
    /// All unordered pairs across both sets, including pairs within each set.
    pub maximum_separation_checks: u64,
    /// Two projections per validation point; fitting points are never scored.
    pub maximum_projection_checks: u64,
    pub minimum_endpoint_separation: f64,
    pub source_tolerance: f64,
    pub target_tolerance: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectiveValidationEvidence {
    pub validation_points: usize,
    pub forward_inliers: usize,
    pub reverse_inliers: usize,
    pub joint_inliers: usize,
    pub maximum_forward_error: f64,
    pub maximum_reverse_error: f64,
    pub separation_checks: u64,
    pub projection_checks: u64,
}

/// Validate endpoints and score only `validation` under the unchanged model.
/// Both sets must be supplied before validation is inspected or used for model
/// selection. `fitting` must include the complete pool used to select/refine the
/// model, not just its eventual inliers. Every source endpoint and every target
/// endpoint must be separated from every other endpoint in the same domain by
/// more than the caller's minimum distance. Work is prepaid; no allocation,
/// fitting, refinement, inlier selection or threshold-based verdict occurs.
///
/// The caller owns provenance: this function cannot establish that the supplied
/// model was fitted without validation points. Coordinate separation also does
/// not establish disjoint descriptor/pixel contexts, statistical independence,
/// or independence from a neural provider that saw the complete images.
/// Neither inliers nor outliers prove copying or non-copying.
///
/// # Errors
/// Invalid policy/domains/model/endpoints, aliases in either domain, numerical
/// projection failure, prepaid work refusal or cancellation. No partial evidence.
#[allow(clippy::too_many_arguments)]
pub fn validate_projective_points(
    model: ProjectiveTransform,
    fitting: &[Correspondence],
    validation: &[Correspondence],
    source: (u32, u32),
    target: (u32, u32),
    policy: ProjectiveValidationPolicy,
    cancel: impl Fn() -> bool,
) -> Result<ProjectiveValidationEvidence, GeometryError> {
    let work = validate_sets(fitting, validation, source, target, policy, &cancel)?;
    score_validation(model, validation, source, target, policy, work, &cancel)
}

fn validate_sets(
    fitting: &[Correspondence],
    validation: &[Correspondence],
    source: (u32, u32),
    target: (u32, u32),
    policy: ProjectiveValidationPolicy,
    cancel: &impl Fn() -> bool,
) -> Result<(u64, u64), GeometryError> {
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    if fitting.len() < 4
        || validation.is_empty()
        || source.0 == 0
        || source.1 == 0
        || target.0 == 0
        || target.1 == 0
        || [
            policy.minimum_endpoint_separation,
            policy.source_tolerance,
            policy.target_tolerance,
        ]
        .iter()
        .any(|x| !x.is_finite() || *x <= 0.)
    {
        return Err(GeometryError::Invalid);
    }
    let count = fitting
        .len()
        .checked_add(validation.len())
        .ok_or(GeometryError::Budget)?;
    let count = u64::try_from(count).map_err(|_| GeometryError::Budget)?;
    let separation_checks = count
        .checked_mul(count - 1)
        .and_then(|x| x.checked_div(2))
        .ok_or(GeometryError::Budget)?;
    let projection_checks = u64::try_from(validation.len())
        .ok()
        .and_then(|x| x.checked_mul(2))
        .ok_or(GeometryError::Budget)?;
    if fitting.len() > policy.maximum_fitting_points
        || validation.len() > policy.maximum_validation_points
        || separation_checks > policy.maximum_separation_checks
        || projection_checks > policy.maximum_projection_checks
    {
        return Err(GeometryError::Budget);
    }
    for (i, point) in fitting.iter().chain(validation).enumerate() {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        for (xy, dimensions) in [(point.source, source), (point.target, target)] {
            if xy.iter().any(|v| !v.is_finite() || *v < 0.)
                || xy[0] >= f64::from(dimensions.0)
                || xy[1] >= f64::from(dimensions.1)
            {
                return Err(GeometryError::Invalid);
            }
        }
        for prior in fitting.iter().chain(validation).take(i) {
            if cancel() {
                return Err(GeometryError::Cancelled);
            }
            for (a, b) in [(prior.source, point.source), (prior.target, point.target)] {
                if (a[0] - b[0]).hypot(a[1] - b[1]) <= policy.minimum_endpoint_separation {
                    return Err(GeometryError::Invalid);
                }
            }
        }
    }
    Ok((separation_checks, projection_checks))
}

#[allow(clippy::too_many_arguments)]
fn score_validation(
    model: ProjectiveTransform,
    validation: &[Correspondence],
    source: (u32, u32),
    target: (u32, u32),
    policy: ProjectiveValidationPolicy,
    (separation_checks, projection_checks): (u64, u64),
    cancel: &impl Fn() -> bool,
) -> Result<ProjectiveValidationEvidence, GeometryError> {
    if !crate::geometry::projective_domain_eligible(model, source, target) {
        return Err(GeometryError::Invalid);
    }
    let inverse = model.inverse()?;
    let mut evidence = ProjectiveValidationEvidence {
        validation_points: validation.len(),
        forward_inliers: 0,
        reverse_inliers: 0,
        joint_inliers: 0,
        maximum_forward_error: 0.,
        maximum_reverse_error: 0.,
        separation_checks,
        projection_checks,
    };
    for point in validation {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        let projected = model.apply(point.source).ok_or(GeometryError::Invalid)?;
        let forward = (projected[0] - point.target[0]).hypot(projected[1] - point.target[1]);
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        let projected = inverse.apply(point.target).ok_or(GeometryError::Invalid)?;
        let reverse = (projected[0] - point.source[0]).hypot(projected[1] - point.source[1]);
        if !forward.is_finite() || !reverse.is_finite() {
            return Err(GeometryError::Invalid);
        }
        let a = forward <= policy.target_tolerance;
        let b = reverse <= policy.source_tolerance;
        evidence.forward_inliers += usize::from(a);
        evidence.reverse_inliers += usize::from(b);
        evidence.joint_inliers += usize::from(a && b);
        evidence.maximum_forward_error = evidence.maximum_forward_error.max(forward);
        evidence.maximum_reverse_error = evidence.maximum_reverse_error.max(reverse);
    }
    if cancel() {
        return Err(GeometryError::Cancelled);
    }
    Ok(evidence)
}

/// A training-only fit and its unchanged-model validation evidence.
/// Training indices refer only to the caller's `fitting` slice.
#[derive(Debug, Clone)]
pub struct ValidatedProjectiveFit {
    pub fitting: JointProjectiveEvidence,
    pub validation: ProjectiveValidationEvidence,
}

/// Fit/select/refine exclusively on `fitting`, then score `validation` once.
/// Complete sets and the split must be fixed before this call. Validation
/// coordinates are inspected for input validity/separation before fitting, but
/// never enter sampling, hypothesis ranking, refinement or inlier selection.
/// Even zero validation inliers retain the training candidate and all validation
/// error counts; the function never chooses another model to improve validation.
/// `None` means no qualifying training hypothesis, not unrelated images.
///
/// Validation pair/projection work uses `validation_policy`. Training search,
/// its point validation and bounded fallible scratch use `fitting_policy` and
/// `sampling`, exactly as in the underlying joint projective fitter. Scratch is
/// outside a managed payload budget. This enforces algorithmic separation of
/// declared point sets; it does not prove descriptor/pixel independence, prevent
/// callers from tuning repeated calls against validation, or establish copying.
///
/// # Errors
/// Invalid complete sets/domains/policies, aliases, fitting/validation work
/// refusal, numerical failure, allocation failure or cancellation. Invalid
/// validation input is refused even if the fitting set would yield no model.
#[allow(clippy::too_many_arguments)]
pub fn fit_and_validate_projective_points(
    fitting: &[Correspondence],
    validation: &[Correspondence],
    source: (u32, u32),
    target: (u32, u32),
    fitting_policy: JointProjectivePolicy,
    sampling: ProjectiveSamplingPolicy,
    validation_policy: ProjectiveValidationPolicy,
    cancel: impl Fn() -> bool,
) -> Result<Option<ValidatedProjectiveFit>, GeometryError> {
    let work = validate_sets(fitting, validation, source, target, validation_policy, &cancel)?;
    let candidate = verify_projective_sampled_joint_for_domains(
        fitting,
        fitting_policy,
        sampling,
        source,
        target,
        &cancel,
    )?;
    let Some(candidate) = candidate else {
        if cancel() {
            return Err(GeometryError::Cancelled);
        }
        return Ok(None);
    };
    let validation = score_validation(
        candidate.transform,
        validation,
        source,
        target,
        validation_policy,
        work,
        &cancel,
    )?;
    Ok(Some(ValidatedProjectiveFit {
        fitting: candidate,
        validation,
    }))
}
