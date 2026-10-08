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
// Local edge points share one contiguous allocation per axis. Duplicates are
// removed after collection, before any interior-range query.
#[derive(Default)]
struct Edges { points: Vec<(u32,u32)> }
impl Edges {
    fn insert(&mut self, fixed:u32, value:u32) {self.points.push((fixed,value));}
    fn extend(&mut self, fixed:u32, values:impl Iterator<Item=u32>) {
        self.points.extend(values.map(|value|(fixed,value)));
    }
    fn finish(&mut self) {self.points.sort_unstable();self.points.dedup();}
}
const PARAM_SCALE: f64 = 65536.;
type EdgeKey = [u64; 8];
struct SharedEdgeSample { key: EdgeKey, parameter: u32, point: Point }
struct Boundary<'a> {
    reversed: bool,
    points: &'a [SharedEdgeSample],
}
impl Boundary<'_> {
    fn local(&self, t: u32) -> u32 {
        if self.reversed { 65536 - t } else { t }
    }
}
fn boundary_controls(patch: &TensorProductPatch) -> [[Point; 4]; 4] {
    [[0, 11, 10, 9], [9, 8, 7, 6], [3, 4, 5, 6], [0, 1, 2, 3]]
        .map(|indices| indices.map(|i| patch.control_points[i]))
}
fn canonical_edge(points: [Point; 4]) -> (EdgeKey, bool, [Point; 4]) {
    let points =
        points.map(|p| Point::new(if p.x == 0. { 0. } else { p.x }, if p.y == 0. { 0. } else { p.y }));
    let key = |p: [Point; 4]| {
        std::array::from_fn(|i| {
            if i % 2 == 0 {
                p[i / 2].x.to_bits()
            } else {
                p[i / 2].y.to_bits()
            }
        })
    };
    let reverse = [points[3], points[2], points[1], points[0]];
    if key(reverse) < key(points) {
        (key(reverse), true, reverse)
    } else {
        (key(points), false, points)
    }
}
fn curve_point(points: [Point; 4], t: u32) -> Point {
    if t == 0 {
        return points[0];
    }
    if t == 65536 {
        return points[3];
    }
    let t = f64::from(t) / PARAM_SCALE;
    let weights = [
        (1. - t).powi(3),
        3. * t * (1. - t).powi(2),
        3. * t.powi(2) * (1. - t),
        t.powi(3),
    ];
    points
        .into_iter()
        .zip(weights)
        .fold(kurbo::Vec2::ZERO, |v, (p, w)| v + p.to_vec2() * w)
        .to_point()
}
/// A borrowed source patch in an adaptive mesh. No source geometry is copied.
#[derive(Clone, Copy)]
pub enum AdaptivePatchRef<'a> {
    /// A tensor-product bicubic patch.
    Tensor(&'a TensorProductPatch),
    /// A Coons patch whose boundary cubics are converted exactly to tensor form.
    Coons(&'a super::CoonsPatch),
}
/// Tessellate Coons and tensor patches together, sharing exactly matching cubic
/// boundaries, preserving source order and each patch's own color components.
/// Limits and rollback apply to the whole mesh. Planning memory is count-bounded
/// but not admitted through a shared byte budget. This API remains opt-in.
pub fn tessellate_patch_mesh_adaptive(
    patches: &[AdaptivePatchRef<'_>],
    transform: Affine,
    limits: AdaptivePatchLimits,
    output: &mut Vec<Triangle>,
    cancelled: impl FnMut() -> bool,
) -> Result<AdaptivePatchStats, AdaptivePatchError> {
    tessellate_mesh(
        patches.iter().copied(),
        transform,
        limits,
        output,
        None,
        cancelled,
    )
}
/// Tessellate a mixed mesh and append one absolute triangle range per source
/// patch, in source order. Ranges describe only triangles appended by this call.
/// On error both output vectors retain their original entries. Range metadata
/// lets compositing preserve source patch ownership across adaptive cell splits.
pub fn tessellate_patch_mesh_adaptive_with_ranges(
    patches: &[AdaptivePatchRef<'_>],
    transform: Affine,
    limits: AdaptivePatchLimits,
    output: &mut Vec<Triangle>,
    ranges: &mut Vec<std::ops::Range<usize>>,
    cancelled: impl FnMut() -> bool,
) -> Result<AdaptivePatchStats, AdaptivePatchError> {
    tessellate_mesh(
        patches.iter().copied(),
        transform,
        limits,
        output,
        Some(ranges),
        cancelled,
    )
}
/// Tessellate a tensor patch mesh with identical points along exactly matching
/// cubic boundaries, including reversed parameter direction. Colors remain owned
/// by each patch. The triangle cap applies to the whole call; errors roll back
/// all newly appended triangles. Near but distinct curves are never welded.
/// Temporary planning allocations are count-bounded, not charged to a memory budget.
/// This opt-in API does not change the renderer's legacy tessellation path.
pub fn tessellate_tensor_patch_mesh_adaptive(
    patches: &[TensorProductPatch],
    transform: Affine,
    limits: AdaptivePatchLimits,
    output: &mut Vec<Triangle>,
    cancelled: impl FnMut() -> bool,
) -> Result<AdaptivePatchStats, AdaptivePatchError> {
    tessellate_mesh(
        patches.iter().map(AdaptivePatchRef::Tensor),
        transform,
        limits,
        output,
        None,
        cancelled,
    )
}
fn tessellate_mesh<'a>(
    patches: impl ExactSizeIterator<Item = AdaptivePatchRef<'a>>,
    transform: Affine,
    limits: AdaptivePatchLimits,
    output: &mut Vec<Triangle>,
    mut ranges: Option<&mut Vec<std::ops::Range<usize>>>,
    mut cancelled: impl FnMut() -> bool,
) -> Result<AdaptivePatchStats, AdaptivePatchError> {
    use std::borrow::Cow;
    if cancelled() {
        return Err(AdaptivePatchError::Cancelled);
    }
    if !limits.pixel_error.is_finite()
        || limits.pixel_error <= 0.
        || !limits.component_error.is_finite()
        || limits.component_error <= 0.
        || limits.max_depth > 16
        || limits.max_triangles > 1_048_576
        || transform.as_coeffs().iter().any(|v| !v.is_finite())
    {
        return Err(AdaptivePatchError::Invalid);
    }
    if patches.len() > limits.max_triangles / 2 {
        return Err(AdaptivePatchError::TriangleLimit);
    }
    let initial = output.len();
    let initial_ranges = ranges.as_ref().map_or(0, |ranges| ranges.len());
    let result = (|| {
        let mut planned = Vec::new();
        let mut minimum = 0;
        let mut shared: Vec<SharedEdgeSample> = Vec::new();
        let mut patch_edges = Vec::new();
        for source in patches {
            if cancelled() {
                return Err(AdaptivePatchError::Cancelled);
            }
            let patch = match source {
                AdaptivePatchRef::Tensor(patch) => Cow::Borrowed(patch),
                AdaptivePatchRef::Coons(patch) => Cow::Owned(patch.adaptive_tensor()?),
            };
            let mut available = limits;
            available.max_triangles = limits.max_triangles - minimum;
            let leaves = patch.plan_adaptive(transform, available, &mut cancelled)?;
            minimum += leaves.len() * 2;
            let controls = boundary_controls(&patch);
            let edges = controls.map(canonical_edge);
            for leaf in &leaves {
                if cancelled() {
                    return Err(AdaptivePatchError::Cancelled);
                }
                let [u0, u1, v0, v1] = leaf.uv.map(|v| (v * PARAM_SCALE).round() as u32);
                for (edge, touches, values) in [
                    (0, v0 == 0, [u0, u1]),
                    (1, u1 == 65536, [v0, v1]),
                    (2, v1 == 65536, [u0, u1]),
                    (3, u0 == 0, [v0, v1]),
                ] {
                    if touches {
                        let (key, reversed, controls) = edges[edge];
                        for parameter in values.map(|t| if reversed {65536-t} else {t}) {
                            if cancelled() {return Err(AdaptivePatchError::Cancelled);}
                            let point=transform*curve_point(controls,parameter);
                            if !point.x.is_finite() || !point.y.is_finite() {return Err(AdaptivePatchError::Invalid);}
                            shared.push(SharedEdgeSample {key,parameter,point});
                        }
                    }
                }
            }
            patch_edges.push(edges);
            planned.push((patch, leaves));
        }
        if cancelled() {return Err(AdaptivePatchError::Cancelled);}
        shared.sort_unstable_by_key(|sample|(sample.key,sample.parameter));
        shared.dedup_by(|a,b|a.key==b.key && a.parameter==b.parameter);
        if cancelled() {return Err(AdaptivePatchError::Cancelled);}
        let mut total = AdaptivePatchStats::default();
        for ((patch, leaves), edges) in planned.into_iter().zip(patch_edges) {
            let boundaries = edges.map(|(key, reversed, _)| Boundary {
                reversed,
                points: &shared[shared.partition_point(|sample|sample.key<key)
                    ..shared.partition_point(|sample|sample.key<=key)],
            });
            let mut remaining = limits;
            remaining.max_triangles -= total.triangles;
            let patch_start = output.len();
            let stats = patch.emit_adaptive(
                leaves,
                transform,
                remaining,
                output,
                Some(&boundaries),
                &mut cancelled,
            )?;
            if let Some(ranges) = &mut ranges {
                ranges.push(patch_start..output.len());
            }
            total.triangles += stats.triangles;
            total.deepest = total.deepest.max(stats.deepest);
            total.max_pixel_bound = total.max_pixel_bound.max(stats.max_pixel_bound);
            total.max_component_bound = total.max_component_bound.max(stats.max_component_bound);
        }
        Ok(total)
    })();
    if result.is_err() {
        output.truncate(initial);
        if let Some(ranges) = &mut ranges {
            ranges.truncate(initial_ranges);
        }
    }
    result
}

fn interior(edges: &Edges, fixed: u32, low: u32, high: u32) -> Vec<u32> {
    let start=edges.points.partition_point(|&point|point<=(fixed,low));
    let end=edges.points.partition_point(|&point|point<(fixed,high));
    edges.points[start..end].iter().map(|&(_,value)|value).collect()
}

fn patch_vertex(
    patch: &TensorProductPatch,
    transform: Affine,
    uv: Point,
    boundaries: Option<&[Boundary; 4]>,
) -> Result<TriangleVertex, AdaptivePatchError> {
    let shared = boundaries.and_then(|edges| {
        let (edge, t) = if uv.y == 0. {
            (0, uv.x)
        } else if uv.x == 1. {
            (1, uv.y)
        } else if uv.y == 1. {
            (2, uv.x)
        } else if uv.x == 0. {
            (3, uv.y)
        } else {
            return None;
        };
        let local = (t * PARAM_SCALE).round() as u32;
        edges[edge].points.binary_search_by_key(&edges[edge].local(local),|sample|sample.parameter)
            .ok().map(|index|edges[edge].points[index].point)
    });
    let point = shared.unwrap_or_else(|| transform * patch.map_coordinate(uv));
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
    boundaries: Option<&[Boundary; 4]>,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), AdaptivePatchError> {
    let center = patch_vertex(patch, transform, center, boundaries)?;
    for index in 0..polygon.len() {
        if cancelled() {
            return Err(AdaptivePatchError::Cancelled);
        }
        output.push(Triangle::new(
            center.clone(),
            patch_vertex(patch, transform, polygon[index], boundaries)?,
            patch_vertex(patch, transform, polygon[(index + 1) % polygon.len()], boundaries)?,
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
        let initial = output.len();
        let result = (|| {
            let leaves = self.plan_adaptive(transform, limits, &mut cancelled)?;
            self.emit_adaptive(leaves, transform, limits, output, None, &mut cancelled)
        })();
        if result.is_err() {
            output.truncate(initial);
        }
        result
    }
    fn plan_adaptive(
        &self,
        transform: Affine,
        limits: AdaptivePatchLimits,
        cancelled: &mut impl FnMut() -> bool,
    ) -> Result<Vec<Leaf>, AdaptivePatchError> {
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
        let mut leaves = Vec::new();
        // DFS keeps at most three siblings per level plus the current cell.
        // Depth <=16 was validated above, so planning needs no heap stack.
        let mut pending: [Option<Cell>;49]=std::array::from_fn(|_|None);
        pending[0]=Some(Cell {grid,uv:[0.,1.,0.,1.],depth:0});
        let mut pending_len=1;
        while pending_len>0 {
            pending_len-=1;
            let cell=pending[pending_len].take().ok_or(AdaptivePatchError::Invalid)?;
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
                    if pending_len>=pending.len() {return Err(AdaptivePatchError::Invalid);}
                    pending[pending_len]=Some(Cell {grid,uv,depth:cell.depth+1});
                    pending_len+=1;
                }
            }
        }
        Ok(leaves)
    }
    fn emit_adaptive(
        &self,
        leaves: Vec<Leaf>,
        transform: Affine,
        limits: AdaptivePatchLimits,
        output: &mut Vec<Triangle>,
        boundaries: Option<&[Boundary; 4]>,
        cancelled: &mut impl FnMut() -> bool,
    ) -> Result<AdaptivePatchStats, AdaptivePatchError> {
        let mut vertical = Edges::default();
        let mut horizontal = Edges::default();
        for leaf in &leaves {
            if cancelled() {
                return Err(AdaptivePatchError::Cancelled);
            }
            let [u0, u1, v0, v1] = leaf.uv.map(|v| (v * PARAM_SCALE).round() as u32);
            for x in [u0, u1] {
                for y in [v0, v1] {
                    vertical.insert(x,y);
                    horizontal.insert(y,x);
                }
            }
        }
        if let Some(boundaries) = boundaries {
            for (edge, boundary) in boundaries.iter().enumerate() {
                if cancelled() {
                    return Err(AdaptivePatchError::Cancelled);
                }
                let values = boundary.points.iter().map(|sample|boundary.local(sample.parameter));
                match edge {
                    0 => horizontal.extend(0,values),
                    1 => vertical.extend(65536,values),
                    2 => horizontal.extend(65536,values),
                    _ => vertical.extend(0,values),
                }
            }
        }
        if cancelled() { return Err(AdaptivePatchError::Cancelled); }
        vertical.finish(); horizontal.finish();
        if cancelled() { return Err(AdaptivePatchError::Cancelled); }
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
                let a = patch_vertex(self, transform, a, boundaries)?;
                let b = patch_vertex(self, transform, b, boundaries)?;
                let c = patch_vertex(self, transform, c, boundaries)?;
                let d = patch_vertex(self, transform, d, boundaries)?;
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
                    boundaries,
                    cancelled,
                )?;
                emit_fan(
                    self,
                    transform,
                    &second,
                    Point::new(u0 + 2. * (u1 - u0) / 3., v0 + 2. * (v1 - v0) / 3.),
                    output,
                    boundaries,
                    cancelled,
                )?;
            }
            stats.triangles += added;
            stats.deepest = stats.deepest.max(leaf.depth);
            stats.max_pixel_bound = stats.max_pixel_bound.max(leaf.geometry);
            stats.max_component_bound = stats.max_component_bound.max(leaf.color);
        }
        Ok(stats)
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
        self.adaptive_tensor()?
            .to_triangles_adaptive(transform, limits, output, cancelled)
    }
    fn adaptive_tensor(&self) -> Result<TensorProductPatch, AdaptivePatchError> {
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
        // The boundary cubics are already in tensor form. Preserve them directly:
        // adding and subtracting the Coons corner surface can change their bits,
        // preventing exact boundary identity with adjacent source patches.
        control_points[..12].copy_from_slice(cp);
        Ok(TensorProductPatch {
            control_points,
            colors: self.colors.clone(),
        })
    }
}

#[cfg(test)]
mod qualification_tests {
    use super::*;
    fn patch(x: f64, colors: [f32;4]) -> TensorProductPatch {
        let indices = [[0,1,2,3],[11,12,13,4],[10,15,14,5],[9,8,7,6]];
        let mut control_points=[Point::ZERO;16];
        for u in 0..4 {for v in 0..4 {
            control_points[indices[u][v]]=Point::new(x+u as f64*8./3.,v as f64*8./3.);
        }}
        TensorProductPatch {control_points,colors:colors.map(|v|smallvec::smallvec![v])}
    }
    fn limits() -> AdaptivePatchLimits {
        AdaptivePatchLimits {pixel_error:0.01,component_error:0.01,max_depth:8,max_triangles:4096}
    }
    #[test]
    fn bilinear_source_component_is_within_bound_at_independent_dense_samples() {
        let source=patch(0.,[0.,0.,1.,0.]);
        let mut output=Vec::new();
        let stats=source.to_triangles_adaptive(Affine::IDENTITY,limits(),&mut output,||false).unwrap();
        assert_eq!(stats.triangles,output.len());
        assert!(stats.max_pixel_bound<=0.01 && stats.max_component_bound<=0.01);
        for y in 0..99 {for x in 0..99 {
            let u=(x as f64+0.317)/99.; let v=(y as f64+0.193)/99.;
            let point=Point::new(u*8.,v*8.);
            let triangle=output.iter().find(|t|t.contains_point(point)).expect("no holes in planar patch");
            let actual=f64::from(triangle.interpolate(point)[0]);
            assert!((actual-u*v).abs()<=stats.max_component_bound+1e-7);
        }}
    }
    #[test]
    fn unequal_patch_subdivision_welds_shared_boundary_and_preserves_source_color() {
        let mut left=patch(0.,[0.125;4]); left.control_points[12].y+=2.;
        let right=patch(8.,[0.875;4]);
        let patches=[AdaptivePatchRef::Tensor(&left),AdaptivePatchRef::Tensor(&right)];
        let mut output=Vec::new();let mut ranges=Vec::new();
        tessellate_patch_mesh_adaptive_with_ranges(&patches,Affine::IDENTITY,limits(),&mut output,&mut ranges,||false).unwrap();
        assert_eq!(ranges.len(),2); assert!(ranges[0].len()>ranges[1].len());
        let boundary=|range:std::ops::Range<usize>| {
            let mut points=Vec::new();
            for triangle in &output[range] {for vertex in [&triangle.p0,&triangle.p1,&triangle.p2] {
                if vertex.point.x==8. {points.push((vertex.point.x.to_bits(),vertex.point.y.to_bits()));}
            }}
            points.sort_unstable();points.dedup();points
        };
        let shared=boundary(ranges[0].clone());assert!(shared.len()>2);
        assert_eq!(shared,boundary(ranges[1].clone()));
        for (range,expected) in ranges.iter().zip([0.125,0.875]) {
            for triangle in &output[range.clone()] {for vertex in [&triangle.p0,&triangle.p1,&triangle.p2] {assert_eq!(vertex.colors[0],expected);}}
        }
    }
    #[test]
    fn mesh_cancellation_and_triangle_limit_preserve_existing_output_and_ranges() {
        let source=patch(0.,[0.;4]);let patches=[AdaptivePatchRef::Tensor(&source)];
        let mut seed=Vec::new();source.to_triangles_adaptive(Affine::IDENTITY,limits(),&mut seed,||false).unwrap();
        let original=seed[0].clone();
        let polls=std::cell::Cell::new(0);
        let mut output=vec![original.clone()];let mut ranges=vec![99..101];
        tessellate_patch_mesh_adaptive_with_ranges(&patches,Affine::IDENTITY,limits(),&mut output,&mut ranges,||{polls.set(polls.get()+1);false}).unwrap();
        let total=polls.get();
        for target in 1..=total {
            polls.set(0);let mut output=vec![original.clone()];let mut ranges=vec![99..101];
            assert_eq!(tessellate_patch_mesh_adaptive_with_ranges(&patches,Affine::IDENTITY,limits(),&mut output,&mut ranges,||{polls.set(polls.get()+1);polls.get()>=target}).err(),Some(AdaptivePatchError::Cancelled));
            assert_eq!(output.len(),1);assert_eq!(ranges,vec![99..101]);
            assert_eq!(output[0].p0.point,original.p0.point);
        }
        let mut output=vec![original];let mut ranges=vec![99..101];let mut tight=limits();tight.max_triangles=1;
        assert_eq!(tessellate_patch_mesh_adaptive_with_ranges(&patches,Affine::IDENTITY,tight,&mut output,&mut ranges,||false).err(),Some(AdaptivePatchError::TriangleLimit));
        assert_eq!(output.len(),1);assert_eq!(ranges,vec![99..101]);
    }
}
