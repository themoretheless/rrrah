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
