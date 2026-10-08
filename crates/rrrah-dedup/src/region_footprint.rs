//! Geometry-only filter footprint selection. This does not decide copy identity.
use crate::geometry::ProjectiveTransform;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilterFootprint {
    pub jacobian: [[f64; 2]; 2],
    pub area_scale: f64,
    pub source_radius: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FootprintError {
    #[error("invalid footprint geometry or policy")]
    Invalid,
    #[error("filter footprint exceeds radius budget")]
    Budget,
    #[error("footprint selection cancelled")]
    Cancelled,
}
/// Select an integer source radius with area equivalent to the target radius at
/// an explicit source point. Anisotropic/projective footprints are approximated
/// by a box; callers must validate both directions on actual heldout pixels.
/// Homography magnitude does not affect selection. No pixel reads or allocation.
pub fn select_filter_footprint(
    model: ProjectiveTransform,
    source_point: [f64; 2],
    target_radius: u32,
    maximum_radius: u32,
    cancel: impl Fn() -> bool,
) -> Result<FilterFootprint, FootprintError> {
    if cancel() {
        return Err(FootprintError::Cancelled);
    }
    if target_radius == 0 || maximum_radius == 0 || source_point.iter().any(|v| !v.is_finite()) {
        return Err(FootprintError::Invalid);
    }
    let scale = model.matrix.iter().flatten().fold(0.0_f64, |a, b| a.max(b.abs()));
    if !scale.is_finite() || scale == 0.0 || model.matrix.iter().flatten().any(|v| !v.is_finite()) {
        return Err(FootprintError::Invalid);
    }
    let m = model.matrix.map(|row| row.map(|v| v / scale));
    let [x, y] = source_point;
    let den = m[2][0] * x + m[2][1] * y + m[2][2];
    if !den.is_finite() || den == 0.0 {
        return Err(FootprintError::Invalid);
    }
    let q: [f64; 2] = std::array::from_fn(|i| (m[i][0] * x + m[i][1] * y + m[i][2]) / den);
    let jacobian: [[f64; 2]; 2] =
        std::array::from_fn(|i| std::array::from_fn(|k| (m[i][k] - q[i] * m[2][k]) / den));
    let area_scale = (jacobian[0][0] * jacobian[1][1] - jacobian[0][1] * jacobian[1][0])
        .abs()
        .sqrt();
    if !area_scale.is_finite() || area_scale <= 0.0 {
        return Err(FootprintError::Invalid);
    }
    let radius = (f64::from(target_radius) / area_scale).ceil().max(1.0);
    if radius > f64::from(maximum_radius) {
        return Err(FootprintError::Budget);
    }
    if cancel() {
        return Err(FootprintError::Cancelled);
    }
    Ok(FilterFootprint {
        jacobian,
        area_scale,
        source_radius: radius as u32,
    })
}

/// Bound supplied, already verified target inliers inside their fitting region.
/// This allocation-free operation does not verify a transform or prove identity.
/// Indices must be strictly increasing; all selected coordinates must be finite
/// and inside the fitting rectangle. The rectangle includes interpolation neighbors
/// where possible but never extends beyond the supplied fitting region.
pub fn bound_target_inliers(
    points: &[crate::geometry::Correspondence],
    inliers: &[usize],
    fitting_region: [u32; 4],
    maximum_points: usize,
    cancel: impl Fn() -> bool,
) -> Result<[u32; 4], FootprintError> {
    if cancel() {
        return Err(FootprintError::Cancelled);
    }
    let [x, y, w, h] = fitting_region;
    let end_x = x.checked_add(w).ok_or(FootprintError::Invalid)?;
    let end_y = y.checked_add(h).ok_or(FootprintError::Invalid)?;
    if w == 0 || h == 0 || maximum_points == 0 || inliers.is_empty() {
        return Err(FootprintError::Invalid);
    }
    if points.len() > maximum_points || inliers.len() > maximum_points {
        return Err(FootprintError::Budget);
    }
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    let mut previous = None;
    for &index in inliers {
        if cancel() {
            return Err(FootprintError::Cancelled);
        }
        if previous.is_some_and(|value| index <= value) {
            return Err(FootprintError::Invalid);
        }
        previous = Some(index);
        let point = points.get(index).ok_or(FootprintError::Invalid)?;
        if point.source.iter().chain(&point.target).any(|v| !v.is_finite())
            || point.target[0] < f64::from(x)
            || point.target[0] >= f64::from(end_x)
            || point.target[1] < f64::from(y)
            || point.target[1] >= f64::from(end_y)
        {
            return Err(FootprintError::Invalid);
        }
        for i in 0..2 {
            low[i] = low[i].min(point.target[i]);
            high[i] = high[i].max(point.target[i]);
        }
    }
    let left = low[0].floor() as u32;
    let top = low[1].floor() as u32;
    let right = (high[0].ceil() + 1.).min(f64::from(end_x)) as u32;
    let bottom = (high[1].ceil() + 1.).min(f64::from(end_y)) as u32;
    if cancel() {
        return Err(FootprintError::Cancelled);
    }
    Ok([left, top, right - left, bottom - top])
}
