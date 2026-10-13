//! Geometry-only residual bounds relative to a fixed projective baseline.
use crate::{
    geometry::{ProjectiveTransform, projective_domain_eligible},
    piecewise_warp::{PiecewiseError, PiecewiseWarp},
};

#[derive(Debug, Clone, Copy)]
pub struct MeshDeformationPolicy {
    /// Source displacement in image-axis normalized coordinates.
    pub maximum_displacement: f64,
    /// Residual difference divided by normalized target edge length.
    pub maximum_edge_slope: f64,
    /// Three vertex projections and three edge comparisons per face.
    pub maximum_checks: u64,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshDeformationEvidence {
    pub faces: usize,
    pub checks: u64,
    pub maximum_displacement: f64,
    pub maximum_edge_slope: f64,
    pub conforms: bool,
}
/// Score every face of an already topology-qualified mesh against an unchanged
/// source-to-target baseline. Residuals compare source vertices with inverse
/// baseline projections of target vertices, normalizing each image axis by its
/// dimension. Shared vertices/edges are deliberately checked per occurrence.
///
/// Work is prepaid, storage constant and cancellation publishes no partial
/// evidence. A failed bound returns complete evidence with `conforms=false`.
/// Edge bounds describe mesh vertices; they do not bound interior deviation
/// from a nonlinear projective inverse, certify global smoothness, or confirm
/// copying. The baseline and bounds must be fixed before inspecting scores.
pub fn validate_mesh_deformation(
    mesh: &PiecewiseWarp,
    baseline: ProjectiveTransform,
    policy: MeshDeformationPolicy,
    cancel: impl Fn() -> bool,
) -> Result<MeshDeformationEvidence, PiecewiseError> {
    if cancel() {
        return Err(PiecewiseError::Cancelled);
    }
    let (source, target) = mesh.dimensions();
    if !policy.maximum_displacement.is_finite()
        || policy.maximum_displacement < 0.
        || !policy.maximum_edge_slope.is_finite()
        || policy.maximum_edge_slope < 0.
        || !projective_domain_eligible(baseline, source, target)
    {
        return Err(PiecewiseError::Invalid);
    }
    let checks = u64::try_from(mesh.triangles().len())
        .ok()
        .and_then(|n| n.checked_mul(6))
        .ok_or(PiecewiseError::Budget)?;
    if checks > policy.maximum_checks {
        return Err(PiecewiseError::Budget);
    }
    let inverse = baseline.inverse().map_err(|_| PiecewiseError::Invalid)?;
    let mut evidence = MeshDeformationEvidence {
        faces: mesh.triangles().len(),
        checks,
        maximum_displacement: 0.,
        maximum_edge_slope: 0.,
        conforms: true,
    };
    for face in mesh.triangles() {
        let mut residuals = [[0.; 2]; 3];
        for (i, residual) in residuals.iter_mut().enumerate() {
            if cancel() {
                return Err(PiecewiseError::Cancelled);
            }
            let p = inverse.apply(face.target[i]).ok_or(PiecewiseError::Invalid)?;
            *residual = [
                (face.source[i][0] - p[0]) / f64::from(source.0),
                (face.source[i][1] - p[1]) / f64::from(source.1),
            ];
            let displacement = residual[0].hypot(residual[1]);
            if !displacement.is_finite() {
                return Err(PiecewiseError::Invalid);
            }
            evidence.maximum_displacement = evidence.maximum_displacement.max(displacement);
        }
        for i in 0..3 {
            if cancel() {
                return Err(PiecewiseError::Cancelled);
            }
            let j = (i + 1) % 3;
            let length = ((face.target[i][0] - face.target[j][0]) / f64::from(target.0))
                .hypot((face.target[i][1] - face.target[j][1]) / f64::from(target.1));
            let slope = (residuals[i][0] - residuals[j][0]).hypot(residuals[i][1] - residuals[j][1]) / length;
            if length <= 0. || !slope.is_finite() {
                return Err(PiecewiseError::Invalid);
            }
            evidence.maximum_edge_slope = evidence.maximum_edge_slope.max(slope);
        }
    }
    if cancel() {
        return Err(PiecewiseError::Cancelled);
    }
    evidence.conforms = evidence.maximum_displacement <= policy.maximum_displacement
        && evidence.maximum_edge_slope <= policy.maximum_edge_slope;
    Ok(evidence)
}
