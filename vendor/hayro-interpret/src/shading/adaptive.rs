//! Control-hull bounds for adaptive patch tessellation, independent of pixel integration.
//! Floating-point safety margins are used; this is not an interval-arithmetic certificate.
use super::{TensorProductPatch, Triangle, TriangleVertex};
use kurbo::{Affine, Point};

/// Coordinates are transformed into the caller's pixel space before bounding.
/// Component error bounds shading inputs, not a nonlinear function/color transform.
#[derive(Debug, Clone, Copy)]
pub struct AdaptivePatchLimits {
    /// Maximum analytical geometric bound in transformed pixel units.
    pub pixel_error: f64,
    /// Maximum bound for each source shading component.
    pub component_error: f64,
    /// Maximum dyadic subdivision depth, at most sixteen.
    pub max_depth: u8,
    /// Maximum newly appended triangles, at most 1,048,576.
    pub max_triangles: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Explicit failure without publishing partially tessellated output.
pub enum AdaptivePatchError {
    /// Nonfinite inputs, invalid tolerances or unsupported resource limits.
    Invalid,
    /// The caller requested cancellation.
    Cancelled,
    /// The requested bounds cannot be met within the depth limit.
    Depth,
    /// Appending the next cell would exceed the triangle limit.
    TriangleLimit,
}
#[derive(Debug, Default, Clone, Copy)]
/// Bounds and occupancy of successfully tessellated cells.
pub struct AdaptivePatchStats {
    /// Number of triangles appended by this call.
    pub triangles: usize,
    /// Deepest accepted subdivision level.
    pub deepest: u8,
    /// Largest accepted geometric bound, including a floating-point safety margin.
    pub max_pixel_bound: f64,
    /// Largest accepted source-component bound.
    pub max_component_bound: f64,
}
type Grid = [[Point; 4]; 4];
struct Leaf {
    uv: [f64; 4],
    depth: u8,
    geometry: f64,
    color: f64,
}
type Edges = std::collections::BTreeMap<u32, std::collections::BTreeSet<u32>>;
const PARAM_SCALE: f64 = 65536.;
fn interior(edges: &Edges, fixed: u32, low: u32, high: u32) -> Vec<u32> {
    use std::ops::Bound::Excluded;
    edges
        .get(&fixed)
        .map(|values| values.range((Excluded(low), Excluded(high))).copied().collect())
        .unwrap_or_default()
}
fn patch_vertex(
    patch: &TensorProductPatch,
    transform: Affine,
    uv: Point,
) -> Result<TriangleVertex, AdaptivePatchError> {
    let point = transform * patch.map_coordinate(uv);
    let colors = patch.interpolate(uv);
    if !point.x.is_finite() || !point.y.is_finite() || colors.iter().any(|v| !v.is_finite()) {
        return Err(AdaptivePatchError::Invalid);
    }
    Ok(TriangleVertex {
        flag: 0,
        point,
        colors,
    })
}
fn emit_fan(
    patch: &TensorProductPatch,
    transform: Affine,
    polygon: &[Point],
    center: Point,
    output: &mut Vec<Triangle>,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), AdaptivePatchError> {
    let center = patch_vertex(patch, transform, center)?;
    for index in 0..polygon.len() {
        if cancelled() {
            return Err(AdaptivePatchError::Cancelled);
        }
        output.push(Triangle::new(
            center.clone(),
            patch_vertex(patch, transform, polygon[index])?,
            patch_vertex(patch, transform, polygon[(index + 1) % polygon.len()])?,
        ));
    }
    Ok(())
}
#[derive(Clone)]
struct Cell {
    grid: Grid,
    uv: [f64; 4],
    depth: u8,
}
fn split_curve(p: [Point; 4]) -> ([Point; 4], [Point; 4]) {
    let a = p[0].midpoint(p[1]);
    let b = p[1].midpoint(p[2]);
    let c = p[2].midpoint(p[3]);
    let d = a.midpoint(b);
    let e = b.midpoint(c);
    let m = d.midpoint(e);
    ([p[0], a, d, m], [m, e, c, p[3]])
}
fn split_grid(grid: Grid) -> [Grid; 4] {
    let mut u = [[[Point::ZERO; 4]; 4]; 2];
    for j in 0..4 {
        let (a, b) = split_curve(std::array::from_fn(|i| grid[i][j]));
        for i in 0..4 {
            u[0][i][j] = a[i];
            u[1][i][j] = b[i];
        }
    }
    let mut children = [[[Point::ZERO; 4]; 4]; 4];
    for side in 0..2 {
        for i in 0..4 {
            let (a, b) = split_curve(u[side][i]);
            children[side][i] = a;
            children[side + 2][i] = b;
        }
    }
    children
}
// A bicubic minus the degree-elevated bilinear corner surface is a Bezier
// surface whose deviation is bounded by its control hull. The bilinear surface
// differs from any triangle whose parameter vertices lie inside the cell by
// at most |corner cross difference| / 4 (a covariance bound). Stitching inserts
// vertices on the true surface, so deviation from the corner bilinear surface
// is paid both at the evaluation point and at the interpolated vertices.
fn pixel_bound(grid: &Grid) -> f64 {
    let [a, b, c, d] = [grid[0][0], grid[3][0], grid[0][3], grid[3][3]];
    let mut deviation: f64 = 0.;
    let mut scale: f64 = 1.;
    for i in 0..4 {
        for j in 0..4 {
            let u = i as f64 / 3.;
            let v = j as f64 / 3.;
            let linear = a.to_vec2() * (1. - u) * (1. - v)
                + b.to_vec2() * u * (1. - v)
                + c.to_vec2() * (1. - u) * v
                + d.to_vec2() * u * v;
            deviation = deviation.max((grid[i][j].to_vec2() - linear).hypot());
            scale = scale.max(grid[i][j].x.abs()).max(grid[i][j].y.abs());
        }
    }
    let cross = a.to_vec2() - b.to_vec2() - c.to_vec2() + d.to_vec2();
    (2. * deviation + cross.hypot() * 0.25) * (1. + 512. * f64::EPSILON) + 512. * f64::EPSILON * scale
}
impl TensorProductPatch {
    /// Tessellate with control-hull bounds on geometry and bilinear components.
    /// Internal neighboring cells use identical boundary vertices, avoiding T junctions.
    /// Separate source patches still require mesh-level boundary agreement.
    /// Existing triangles remain unchanged on any error, including cancellation.
    /// This opt-in API does not change the renderer's legacy tessellation path.
    pub fn to_triangles_adaptive(
        &self,
        transform: Affine,
        limits: AdaptivePatchLimits,
        output: &mut Vec<Triangle>,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<AdaptivePatchStats, AdaptivePatchError> {
        if cancelled() {
            return Err(AdaptivePatchError::Cancelled);
        }
        let channels = self.colors[0].len();
        if !limits.pixel_error.is_finite()
            || limits.pixel_error <= 0.
            || !limits.component_error.is_finite()
            || limits.component_error <= 0.
            || limits.max_depth > 16
            || limits.max_triangles > 1_048_576
            || channels == 0
            || channels > 32
            || self
                .colors
                .iter()
                .any(|c| c.len() != channels || c.iter().any(|v| !v.is_finite()))
        {
            return Err(AdaptivePatchError::Invalid);
        }
        let indices = [[0, 1, 2, 3], [11, 12, 13, 4], [10, 15, 14, 5], [9, 8, 7, 6]];
        let grid: Grid =
            std::array::from_fn(|i| std::array::from_fn(|j| transform * self.control_points[indices[i][j]]));
        if grid
            .iter()
            .flatten()
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
        {
            return Err(AdaptivePatchError::Invalid);
        }
        let cross = self.colors[0]
            .iter()
            .enumerate()
            .map(|(i, &a)| {
                (f64::from(a) - f64::from(self.colors[3][i]) - f64::from(self.colors[1][i])
                    + f64::from(self.colors[2][i]))
                .abs()
            })
            .fold(0., f64::max);
        let scale = self
            .colors
            .iter()
            .flatten()
            .map(|v| f64::from(v.abs()))
            .fold(1., f64::max);
        let initial = output.len();
        let result = (|| {
            let mut leaves = Vec::new();
            let mut pending = vec![Cell {
                grid,
                uv: [0., 1., 0., 1.],
                depth: 0,
            }];
            while let Some(cell) = pending.pop() {
                if cancelled() {
                    return Err(AdaptivePatchError::Cancelled);
                }
                let [u0, u1, v0, v1] = cell.uv;
                let geometry = pixel_bound(&cell.grid);
                let color = cross * (u1 - u0) * (v1 - v0) * 0.25 + 256. * f64::from(f32::EPSILON) * scale;
                if geometry <= limits.pixel_error && color <= limits.component_error {
                    if (leaves.len() + 1) * 2 > limits.max_triangles {
                        return Err(AdaptivePatchError::TriangleLimit);
                    }
                    leaves.push(Leaf {
                        uv: cell.uv,
                        depth: cell.depth,
                        geometry,
                        color,
                    });
                } else {
                    if cell.depth >= limits.max_depth {
                        return Err(AdaptivePatchError::Depth);
                    }
                    let um = (u0 + u1) * 0.5;
                    let vm = (v0 + v1) * 0.5;
                    let uv = [
                        [u0, um, v0, vm],
                        [um, u1, v0, vm],
                        [u0, um, vm, v1],
                        [um, u1, vm, v1],
                    ];
                    for (grid, uv) in split_grid(cell.grid).into_iter().zip(uv).rev() {
                        pending.push(Cell {
                            grid,
                            uv,
                            depth: cell.depth + 1,
                        });
                    }
                }
            }
            let mut vertical = Edges::new();
            let mut horizontal = Edges::new();
            for leaf in &leaves {
                if cancelled() {
                    return Err(AdaptivePatchError::Cancelled);
                }
                let [u0, u1, v0, v1] = leaf.uv.map(|v| (v * PARAM_SCALE).round() as u32);
                for x in [u0, u1] {
                    for y in [v0, v1] {
                        vertical.entry(x).or_default().insert(y);
                        horizontal.entry(y).or_default().insert(x);
                    }
                }
            }
            let mut stats = AdaptivePatchStats::default();
            for leaf in leaves {
                if cancelled() {
                    return Err(AdaptivePatchError::Cancelled);
                }
                let [u0, u1, v0, v1] = leaf.uv;
                let [x0, x1, y0, y1] = leaf.uv.map(|v| (v * PARAM_SCALE).round() as u32);
                let bottom = interior(&horizontal, y0, x0, x1);
                let top = interior(&horizontal, y1, x0, x1);
                let left = interior(&vertical, x0, y0, y1);
                let right = interior(&vertical, x1, y0, y1);
                let a = Point::new(u0, v0);
                let b = Point::new(u1, v0);
                let c = Point::new(u0, v1);
                let d = Point::new(u1, v1);
                let added = if bottom.is_empty() && top.is_empty() && left.is_empty() && right.is_empty() {
                    2
                } else {
                    6 + bottom.len() + top.len() + left.len() + right.len()
                };
                if added > limits.max_triangles.saturating_sub(stats.triangles) {
                    return Err(AdaptivePatchError::TriangleLimit);
                }
                if added == 2 {
                    let a = patch_vertex(self, transform, a)?;
                    let b = patch_vertex(self, transform, b)?;
                    let c = patch_vertex(self, transform, c)?;
                    let d = patch_vertex(self, transform, d)?;
                    output.push(Triangle::new(a, b.clone(), c.clone()));
                    output.push(Triangle::new(b, d, c));
                } else {
                    let mut first = vec![a];
                    first.extend(
                        bottom
                            .into_iter()
                            .map(|x| Point::new(f64::from(x) / PARAM_SCALE, v0)),
                    );
                    first.extend([b, c]);
                    first.extend(
                        left.into_iter()
                            .rev()
                            .map(|y| Point::new(u0, f64::from(y) / PARAM_SCALE)),
                    );
                    let mut second = vec![b];
                    second.extend(
                        right
                            .into_iter()
                            .map(|y| Point::new(u1, f64::from(y) / PARAM_SCALE)),
                    );
                    second.push(d);
                    second.extend(
                        top.into_iter()
                            .rev()
                            .map(|x| Point::new(f64::from(x) / PARAM_SCALE, v1)),
                    );
                    second.push(c);
                    emit_fan(
                        self,
                        transform,
                        &first,
                        Point::new(u0 + (u1 - u0) / 3., v0 + (v1 - v0) / 3.),
                        output,
                        &mut cancelled,
                    )?;
                    emit_fan(
                        self,
                        transform,
                        &second,
                        Point::new(u0 + 2. * (u1 - u0) / 3., v0 + 2. * (v1 - v0) / 3.),
                        output,
                        &mut cancelled,
                    )?;
                }
                stats.triangles += added;
                stats.deepest = stats.deepest.max(leaf.depth);
                stats.max_pixel_bound = stats.max_pixel_bound.max(leaf.geometry);
                stats.max_component_bound = stats.max_component_bound.max(leaf.color);
            }
            Ok(stats)
        })();
        if result.is_err() {
            output.truncate(initial);
        }
        result
    }
}

impl super::CoonsPatch {
    /// Convert the exact bicubic Coons control net to tensor form, then apply
    /// the same transformed-space geometry and source-component bounds.
    pub fn to_triangles_adaptive(
        &self,
        transform: Affine,
        limits: AdaptivePatchLimits,
        output: &mut Vec<Triangle>,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<AdaptivePatchStats, AdaptivePatchError> {
        if cancelled() {
            return Err(AdaptivePatchError::Cancelled);
        }
        let channels = self.colors[0].len();
        if channels == 0
            || channels > 32
            || self
                .colors
                .iter()
                .any(|c| c.len() != channels || c.iter().any(|v| !v.is_finite()))
        {
            return Err(AdaptivePatchError::Invalid);
        }
        let cp = &self.control_points;
        let c1 = [cp[0], cp[11], cp[10], cp[9]];
        let c2 = [cp[3], cp[4], cp[5], cp[6]];
        let d1 = [cp[0], cp[1], cp[2], cp[3]];
        let d2 = [cp[9], cp[8], cp[7], cp[6]];
        let indices = [[0, 1, 2, 3], [11, 12, 13, 4], [10, 15, 14, 5], [9, 8, 7, 6]];
        let mut control_points = [Point::ZERO; 16];
        for i in 0..4 {
            for j in 0..4 {
                let u = i as f64 / 3.;
                let v = j as f64 / 3.;
                let corner = cp[0].to_vec2() * (1. - u) * (1. - v)
                    + cp[9].to_vec2() * u * (1. - v)
                    + cp[3].to_vec2() * (1. - u) * v
                    + cp[6].to_vec2() * u * v;
                control_points[indices[i][j]] = (c1[i].to_vec2() * (1. - v)
                    + c2[i].to_vec2() * v
                    + d1[j].to_vec2() * (1. - u)
                    + d2[j].to_vec2() * u
                    - corner)
                    .to_point();
            }
        }
        TensorProductPatch {
            control_points,
            colors: self.colors.clone(),
        }
        .to_triangles_adaptive(transform, limits, output, cancelled)
    }
}
