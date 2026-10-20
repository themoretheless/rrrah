//! Source-component residuals after an affine projected-triangle interpolation.
//! Control-hull bounds use floating-point safety margins, not interval arithmetic.
use super::{PixelPartitionError, PixelSourceOwner};
use crate::shading::{PatchTriangleSource, TensorProductPatch, Triangle};
use kurbo::{Affine, Point};
fn tick(work: &mut usize, cancelled: &dyn Fn() -> bool) -> Result<(), PixelPartitionError> {
    if cancelled() {
        return Err(PixelPartitionError::Cancelled);
    }
    *work = work.checked_sub(1).ok_or(PixelPartitionError::Limit)?;
    Ok(())
}
fn split(p: [Point; 4], t: f64) -> ([Point; 4], [Point; 4]) {
    let a = p[0].lerp(p[1], t);
    let b = p[1].lerp(p[2], t);
    let c = p[2].lerp(p[3], t);
    let d = a.lerp(b, t);
    let e = b.lerp(c, t);
    let f = d.lerp(e, t);
    ([p[0], a, d, f], [f, e, c, p[3]])
}
fn restrict(p: [Point; 4], low: f64, high: f64) -> [Point; 4] {
    let upper = if high == 1. { p } else { split(p, high).0 };
    if low == 0. {
        upper
    } else {
        split(upper, low / high).1
    }
}
/// Bound each affine painted source-component residual over the triangle's UV
/// bounding rectangle: C_affine(F(u,v)) - C_source(u,v). This includes curved
/// projection, and quantifies source components before nonlinear functions or
/// color conversion. It does not certify inverse branch selection or coverage.
/// All sixteen source controls are used; no allocation is performed.
pub fn projected_component_bounds(
    patch: &TensorProductPatch,
    transform: Affine,
    triangle: &Triangle,
    source: PatchTriangleSource,
    work: &mut usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<[f64; 4], PixelPartitionError> {
    tick(work, cancelled)?;
    let origin = triangle.p0.point;
    let owner = PixelSourceOwner::from_triangle(triangle, source, origin)?;
    let count = owner.component_count;
    if patch
        .colors
        .iter()
        .any(|c| c.len() != count || c.iter().any(|v| !v.is_finite()))
        || patch
            .control_points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
        || transform.as_coeffs().iter().any(|v| !v.is_finite())
    {
        return Err(PixelPartitionError::Invalid);
    }
    // Include original arithmetic magnitudes, not only a possibly cancelling
    // restricted control. This margin also covers restriction and transform.
    let matrix = transform.as_coeffs().map(f64::abs);
    let mut position_scale = 0.0f64;
    for p in &patch.control_points {
        tick(work, cancelled)?;
        let x = matrix[0] * p.x.abs() + matrix[2] * p.y.abs() + matrix[4];
        let y = matrix[1] * p.x.abs() + matrix[3] * p.y.abs() + matrix[5];
        if !x.is_finite() || !y.is_finite() {
            return Err(PixelPartitionError::Conditioning);
        }
        position_scale = position_scale.max(x).max(y);
    }
    let color_scale: [f64; 4] = std::array::from_fn(|k| {
        if k < count {
            patch
                .colors
                .iter()
                .map(|c| f64::from(c[k]).abs())
                .fold(0., f64::max)
        } else {
            0.
        }
    });
    let u0 = source.parameters.iter().map(|p| p.x).fold(1., f64::min);
    let u1 = source.parameters.iter().map(|p| p.x).fold(0., f64::max);
    let v0 = source.parameters.iter().map(|p| p.y).fold(1., f64::min);
    let v1 = source.parameters.iter().map(|p| p.y).fold(0., f64::max);
    if u1 <= u0 || v1 <= v0 {
        return Err(PixelPartitionError::Conditioning);
    }
    let indices = [[0, 1, 2, 3], [11, 12, 13, 4], [10, 15, 14, 5], [9, 8, 7, 6]];
    let mut grid = indices.map(|column| column.map(|i| patch.control_points[i]));
    for j in 0..4 {
        tick(work, cancelled)?;
        let column = std::array::from_fn(|i| grid[i][j]);
        let clipped = restrict(column, u0, u1);
        for i in 0..4 {
            grid[i][j] = clipped[i];
        }
    }
    for row in &mut grid {
        tick(work, cancelled)?;
        *row = restrict(*row, v0, v1);
    }
    let mut result = [0.0f64; 4];
    let mut magnitude = [0.0f64; 4];
    for (i, row) in grid.iter().enumerate() {
        for (j, point) in row.iter().enumerate() {
            tick(work, cancelled)?;
            let point = transform * *point;
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(PixelPartitionError::Conditioning);
            }
            let local = Point::new(point.x - origin.x, point.y - origin.y);
            let u = u0 + (u1 - u0) * i as f64 / 3.;
            let v = v0 + (v1 - v0) * j as f64 / 3.;
            for k in 0..count {
                let expected = f64::from(patch.colors[0][k]) * (1. - u) * (1. - v)
                    + f64::from(patch.colors[3][k]) * u * (1. - v)
                    + f64::from(patch.colors[1][k]) * (1. - u) * v
                    + f64::from(patch.colors[2][k]) * u * v;
                let f = owner.components[k];
                let actual = f[1].mul_add(local.x, f[2].mul_add(local.y, f[0]));
                let scale = f[0].abs()
                    + (f[1] * local.x).abs()
                    + (f[2] * local.y).abs()
                    + expected.abs()
                    + color_scale[k]
                    + (f[1].abs() + f[2].abs()) * position_scale
                    + (f[1].abs() + f[2].abs())
                        * (point.x.abs() + point.y.abs() + origin.x.abs() + origin.y.abs());
                if !actual.is_finite() || !scale.is_finite() {
                    return Err(PixelPartitionError::Conditioning);
                }
                result[k] = result[k].max((actual - expected).abs());
                magnitude[k] = magnitude[k].max(scale);
            }
        }
    }
    for k in 0..count {
        result[k] = result[k] * (1. + 1024. * f64::EPSILON) + 1024. * f64::EPSILON * magnitude[k];
        if !result[k].is_finite() {
            return Err(PixelPartitionError::Conditioning);
        }
    }
    tick(work, cancelled)?;
    Ok(result)
}

/// Bound source-component residual only where the mapped source rectangle and
/// the painted projected triangle overlap. Within that triangle interpolation
/// is a convex combination of the three retained vertex components. Bilinear
/// source extrema on a UV rectangle occur at its four corners. This range bound
/// avoids an ill-conditioned inverse; it does not bound extrapolation outside
/// the painted triangle or certify coverage/inverse ownership.
pub fn visible_projected_component_bounds(
    patch: &TensorProductPatch,
    transform: Affine,
    triangle: &Triangle,
    source: PatchTriangleSource,
    work: &mut usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<[f64; 4], PixelPartitionError> {
    tick(work, cancelled)?;
    let vertices = [&triangle.p0, &triangle.p1, &triangle.p2];
    let count = vertices[0].colors.len();
    if !(1..=4).contains(&count)
        || patch
            .colors
            .iter()
            .any(|c| c.len() != count || c.iter().any(|v| !v.is_finite()))
        || patch
            .control_points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
        || transform.as_coeffs().iter().any(|v| !v.is_finite())
        || vertices.iter().any(|v| {
            !v.point.x.is_finite()
                || !v.point.y.is_finite()
                || v.colors.len() != count
                || v.colors.iter().any(|c| !c.is_finite())
        })
        || source
            .parameters
            .iter()
            .any(|p| !(0.0..=1.0).contains(&p.x) || !(0.0..=1.0).contains(&p.y))
    {
        return Err(PixelPartitionError::Invalid);
    }
    let u0 = source.parameters.iter().map(|p| p.x).fold(1., f64::min);
    let u1 = source.parameters.iter().map(|p| p.x).fold(0., f64::max);
    let v0 = source.parameters.iter().map(|p| p.y).fold(1., f64::min);
    let v1 = source.parameters.iter().map(|p| p.y).fold(0., f64::max);
    if u0 >= u1 || v0 >= v1 {
        return Err(PixelPartitionError::Conditioning);
    }
    if patch.control_points.iter().any(|p| {
        let q = transform * *p;
        !q.x.is_finite() || !q.y.is_finite()
    }) {
        return Err(PixelPartitionError::Conditioning);
    }
    let field = PixelSourceOwner::from_triangle(triangle, source, triangle.p0.point)?;
    let dx = vertices
        .iter()
        .map(|v| (v.point.x - triangle.p0.point.x).abs())
        .fold(0., f64::max);
    let dy = vertices
        .iter()
        .map(|v| (v.point.y - triangle.p0.point.y).abs())
        .fold(0., f64::max);
    let mut range = [0.; 4];
    for k in 0..count {
        tick(work, cancelled)?;
        let c = patch.colors.each_ref().map(|c| f64::from(c[k]));
        let samples = [(u0, v0), (u0, v1), (u1, v0), (u1, v1)].map(|(u, v)| {
            c[0] * (1. - u) * (1. - v) + c[3] * u * (1. - v) + c[1] * (1. - u) * v + c[2] * u * v
        });
        let low = samples.into_iter().fold(f64::INFINITY, f64::min);
        let high = samples.into_iter().fold(f64::NEG_INFINITY, f64::max);
        let painted_low = vertices
            .iter()
            .map(|v| f64::from(v.colors[k]))
            .fold(f64::INFINITY, f64::min);
        let painted_high = vertices
            .iter()
            .map(|v| f64::from(v.colors[k]))
            .fold(f64::NEG_INFINITY, f64::max);
        let magnitude = c.into_iter().map(f64::abs).fold(0., f64::max)
            + painted_low.abs()
            + painted_high.abs()
            + field.components[k][0].abs()
            + field.components[k][1].abs() * dx
            + field.components[k][2].abs() * dy;
        range[k] =
            (painted_high - low).abs().max((high - painted_low).abs()) + 1024. * f64::EPSILON * magnitude;
        if !range[k].is_finite() {
            return Err(PixelPartitionError::Conditioning);
        }
    }
    match projected_component_bounds(patch, transform, triangle, source, work, cancelled) {
        Ok(full) => Ok(std::array::from_fn(|k| full[k].min(range[k]))),
        Err(PixelPartitionError::Conditioning) => Ok(range),
        Err(error) => Err(error),
    }
}

/// Exactly zero area in the retained finite device coordinates. A small but
/// nonzero triangle is never discarded by a floating-point tolerance here.
pub(crate) fn projected_triangle_is_degenerate(triangle: &Triangle) -> bool {
    let points = [triangle.p0.point, triangle.p1.point, triangle.p2.point];
    points.iter().all(|p| p.x.is_finite() && p.y.is_finite())
        && super::ordered_pixel::orientation(points[0], points[1], points[2]) == 0
}

#[cfg(test)]
mod degeneracy_tests {
    use super::*;
    #[test]
    fn zero_area_filter_preserves_subnormal_nonzero_triangles() {
        let patch = TensorProductPatch {
            control_points: std::array::from_fn(|i| Point::new((i % 4) as f64, (i / 4) as f64)),
            colors: std::array::from_fn(|_| smallvec::smallvec![0.]),
        };
        let mut generated = Vec::new();
        patch.to_triangles(&mut generated);
        let mut triangle = generated.pop().unwrap();
        triangle.p0.point = Point::ZERO;
        triangle.p1.point = Point::new(f64::from_bits(1), 0.);
        triangle.p2.point = triangle.p1.point;
        assert!(projected_triangle_is_degenerate(&triangle));
        triangle.p2.point.y = f64::from_bits(1);
        assert!(!projected_triangle_is_degenerate(&triangle));
        triangle.p2.point.y = f64::NAN;
        assert!(!projected_triangle_is_degenerate(&triangle));
    }
}
