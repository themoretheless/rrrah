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
    /// Capacity admission or a fallible allocation was refused.
    Admission,
    /// An emitted projected source component exceeds the requested bound.
    ComponentAccuracy,
}
#[derive(Debug, Default, Clone, Copy)]
/// Bounds and occupancy of successfully tessellated cells.
pub struct AdaptivePatchStats {
    /// Number of triangles appended by this call.
    pub triangles: usize,
    /// Additional local leaf-refinement passes after conforming fan validation.
    pub refinement_passes: u8,
    /// Leaf cells explicitly split after a failing emitted component bound.
    pub refined_leaves: usize,
    /// Deepest accepted subdivision level.
    pub deepest: u8,
    /// Largest accepted geometric bound, including a floating-point safety margin.
    pub max_pixel_bound: f64,
    /// Largest accepted source-component bound.
    pub max_component_bound: f64,
}
type Credit = Box<dyn std::any::Any + Send + Sync>;
type Admitter<'a> = dyn Fn(usize) -> Option<Credit> + 'a;
// The previous credit stays alive while the replacement allocation is admitted
// and grown. Only after successful allocation does the old reservation drop.
fn grow_array<T>(
    array: &mut Vec<T>,
    credit: &mut Option<Credit>,
    admit: Option<&Admitter<'_>>,
) -> Result<(), AdaptivePatchError> {
    grow_array_with_step(array, credit, admit, 1)
}
fn grow_array_with_step<T>(
    array: &mut Vec<T>,
    credit: &mut Option<Credit>,
    admit: Option<&Admitter<'_>>,
    step_divisor: usize,
) -> Result<(), AdaptivePatchError> {
    use AdaptivePatchError::Admission;
    let capacity = array
        .capacity()
        .checked_add((array.capacity() / step_divisor).max(1))
        .ok_or(Admission)?
        .max(4);
    let bytes = capacity.checked_mul(size_of::<T>()).ok_or(Admission)?;
    let next = match admit {
        Some(admit) => match admit(bytes) {
            Some(credit) => Some(credit),
            None => {
                if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
                    eprintln!(
                        "NATIVE_ADAPTIVE_ADMISSION type={} len={} old_capacity={} requested_capacity={capacity} requested_bytes={bytes}",
                        std::any::type_name::<T>(),
                        array.len(),
                        array.capacity()
                    );
                }
                return Err(Admission);
            }
        },
        None => None,
    };
    array
        .try_reserve_exact(capacity - array.len())
        .map_err(|_| Admission)?;
    if array.capacity() > capacity {
        return Err(Admission);
    }
    *credit = next;
    Ok(())
}
fn push_array<T>(array: &mut Vec<T>, value: T) -> Result<(), AdaptivePatchError> {
    if array.len() == array.capacity() {
        grow_array(array, &mut None, None)?;
    }
    array.push(value);
    Ok(())
}
struct MeshArray<'a, T> {
    data: Vec<T>,
    credit: Option<Credit>,
    admit: Option<&'a Admitter<'a>>,
}
impl<'a, T> MeshArray<'a, T> {
    fn len(&self) -> usize {
        self.data.len()
    }
    fn new(admit: Option<&'a Admitter<'a>>) -> Self {
        Self {
            data: Vec::new(),
            credit: None,
            admit,
        }
    }
    fn push(&mut self, value: T) -> Result<(), AdaptivePatchError> {
        if self.data.len() == self.data.capacity() {
            grow_array(&mut self.data, &mut self.credit, self.admit)?;
        }
        self.data.push(value);
        Ok(())
    }
    fn extend(&mut self, values: impl IntoIterator<Item = T>) -> Result<(), AdaptivePatchError> {
        for value in values {
            self.push(value)?;
        }
        Ok(())
    }
    fn compact(&mut self) -> Result<(), AdaptivePatchError> {
        use AdaptivePatchError::Admission;
        if self.data.capacity() == self.data.len() {
            return Ok(());
        }
        let bytes = self.data.len().checked_mul(size_of::<T>()).ok_or(Admission)?;
        let next = match self.admit {
            Some(admit) => Some(admit(bytes).ok_or(Admission)?),
            None => None,
        };
        let mut exact = Vec::new();
        exact.try_reserve_exact(self.data.len()).map_err(|_| Admission)?;
        if exact.capacity() > self.data.len() {
            return Err(Admission);
        }
        exact.append(&mut self.data);
        self.data = exact;
        self.credit = next;
        Ok(())
    }
    fn into_parts(self) -> (Vec<T>, Option<Credit>) {
        (self.data, self.credit)
    }
}
impl<T> std::ops::Deref for MeshArray<'_, T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.data
    }
}
impl<T> std::ops::DerefMut for MeshArray<'_, T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.data
    }
}
struct MeshArrayIter<T> {
    inner: std::vec::IntoIter<T>,
    _credit: Option<Credit>,
}
impl<T> Iterator for MeshArrayIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.inner.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
impl<T> DoubleEndedIterator for MeshArrayIter<T> {
    fn next_back(&mut self) -> Option<T> {
        self.inner.next_back()
    }
}
impl<T> ExactSizeIterator for MeshArrayIter<T> {}
impl<T> IntoIterator for MeshArray<'_, T> {
    type Item = T;
    type IntoIter = MeshArrayIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        MeshArrayIter {
            inner: self.data.into_iter(),
            _credit: self.credit,
        }
    }
}
impl<'b, T> IntoIterator for &'b MeshArray<'_, T> {
    type Item = &'b T;
    type IntoIter = std::slice::Iter<'b, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}
trait TriangleOutput {
    fn len(&self) -> usize;
    fn truncate(&mut self, len: usize);
    fn push_triangle(&mut self, triangle: Triangle) -> Result<(), AdaptivePatchError>;
    fn push_patch_triangle(
        &mut self,
        triangle: Triangle,
        _parameters: [Point; 3],
    ) -> Result<(), AdaptivePatchError> {
        self.push_triangle(triangle)
    }
    fn begin_patch(&mut self, _index: usize) {}
    fn begin_leaf(&mut self, _uv: [f64;4], _depth: u8) {}
    fn projected_components(&self) -> bool {
        false
    }
    fn reserve_patch(&mut self, _triangles: usize) -> Result<(), AdaptivePatchError> {
        Ok(())
    }
    fn plans_global_capacity(&self) -> bool {
        false
    }
}
impl TriangleOutput for Vec<Triangle> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn truncate(&mut self, len: usize) {
        Vec::truncate(self, len)
    }
    fn push_triangle(&mut self, triangle: Triangle) -> Result<(), AdaptivePatchError> {
        if self.len() == self.capacity() {
            grow_array_with_step(self, &mut None, None, 4)?;
        }
        self.push(triangle);
        Ok(())
    }
}
trait EmittedTriangle {
    fn from_triangle(triangle: Triangle) -> Self;
}
impl EmittedTriangle for Triangle {
    fn from_triangle(triangle: Triangle) -> Self {
        triangle
    }
}
/// Compact device-coordinate geometry retaining all source components (one to four).
/// Cached generic interpolation fields are omitted; coordinates/components are unchanged.
#[derive(Debug, Clone)]
pub struct CompactAdaptiveTriangle {
    points: [Point; 3],
    components: [[f32; 4]; 3],
    component_count: usize,
}
impl CompactAdaptiveTriangle {
    /// Original vertices in transformed device coordinates.
    pub fn points(&self) -> &[Point; 3] {
        &self.points
    }
    /// Original source components; inactive slots are zero.
    pub fn components(&self) -> &[[f32; 4]; 3] {
        &self.components
    }
    /// Number of active components at each vertex.
    pub fn component_count(&self) -> usize {
        self.component_count
    }
    /// Expand one triangle on the stack; active components remain inline.
    pub fn to_triangle(&self) -> Triangle {
        let vertices = std::array::from_fn::<_, 3, _>(|i| TriangleVertex {
            flag: 0,
            point: self.points[i],
            colors: super::ColorComponents::from_slice(&self.components[i][..self.component_count]),
        });
        let [p0, p1, p2] = vertices;
        Triangle::new(p0, p1, p2)
    }
}
impl EmittedTriangle for CompactAdaptiveTriangle {
    fn from_triangle(triangle: Triangle) -> Self {
        let vertices = [&triangle.p0, &triangle.p1, &triangle.p2];
        let component_count = vertices[0].colors.len();
        let mut components = [[0.; 4]; 3];
        for (out, vertex) in components.iter_mut().zip(vertices) {
            out[..component_count].copy_from_slice(&vertex.colors);
        }
        Self {
            points: vertices.map(|v| v.point),
            components,
            component_count,
        }
    }
}
impl<T: EmittedTriangle> TriangleOutput for MeshArray<'_, T> {
    fn plans_global_capacity(&self) -> bool {
        true
    }
    fn reserve_patch(&mut self, triangles: usize) -> Result<(), AdaptivePatchError> {
        use AdaptivePatchError::Admission;
        let capacity = self.data.len().checked_add(triangles).ok_or(Admission)?;
        if capacity <= self.data.capacity() {
            return Ok(());
        }
        let bytes = capacity.checked_mul(size_of::<T>()).ok_or(Admission)?;
        let next = match self.admit {
            Some(admit) => Some(admit(bytes).ok_or(Admission)?),
            None => None,
        };
        // Admit new capacity while the old allocation/credit is still live.
        self.data
            .try_reserve_exact(capacity - self.data.len())
            .map_err(|_| Admission)?;
        if self.data.capacity() > capacity {
            return Err(Admission);
        }
        self.credit = next;
        Ok(())
    }

    fn len(&self) -> usize {
        self.data.len()
    }
    fn truncate(&mut self, len: usize) {
        self.data.truncate(len)
    }
    fn push_triangle(&mut self, triangle: Triangle) -> Result<(), AdaptivePatchError> {
        // Only retained triangles grow by 5/4; planning scratch keeps doubling.
        if self.data.len() == self.data.capacity() {
            grow_array_with_step(&mut self.data, &mut self.credit, self.admit, 4)?;
        }
        self.data.push(T::from_triangle(triangle));
        Ok(())
    }
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
struct Edges<'a> {
    points: MeshArray<'a, (u32, u32)>,
}
impl<'a> Edges<'a> {
    fn new(admit: Option<&'a Admitter<'a>>) -> Self {
        Self {
            points: MeshArray::new(admit),
        }
    }
    fn insert(&mut self, fixed: u32, value: u32) -> Result<(), AdaptivePatchError> {
        self.points.push((fixed, value))
    }
    fn extend(&mut self, fixed: u32, values: impl Iterator<Item = u32>) -> Result<(), AdaptivePatchError> {
        self.points.extend(values.map(|value| (fixed, value)))
    }
    fn finish(&mut self) -> Result<(), AdaptivePatchError> {
        self.points.sort_unstable();
        self.points.dedup();
        // Release duplicate/slack storage before output triangle growth.
        self.points.compact()
    }
}
const PARAM_SCALE: f64 = 65536.;
type EdgeKey = [u64; 8];
struct EdgeRecord {
    key: EdgeKey,
    controls: [Point; 4],
}
struct SharedEdgeSample {
    edge: usize,
    parameter: u32,
    point: Point,
}
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
        None,
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
        None,
    )
}
/// Tessellate a tensor patch mesh with identical points along exactly matching
/// cubic boundaries, including reversed parameter direction. Colors remain owned
/// by each patch. The triangle cap applies to the whole call; errors roll back
/// all newly appended triangles. Near but distinct curves are never welded.
/// Temporary planning allocations are count-bounded, not charged to a memory budget.
/// Shared cubic edges receive a chord tolerance 128 times tighter than
/// `pixel_error`; depth and triangle caps also apply to this refinement.
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
        None,
    )
}
/// An admitted mesh with immutable triangles and their retained capacity owner.
/// Parsed source patches belong to the caller and precede this reservation.
pub struct AdmittedAdaptivePatchMesh<T = Triangle> {
    triangles: Vec<T>,
    stats: AdaptivePatchStats,
    // Geometry drops before the reservation that owns its capacity.
    _credit: Option<Credit>,
}
impl<T> AdmittedAdaptivePatchMesh<T> {
    /// Original source-component triangles in source patch order.
    pub fn triangles(&self) -> &[T] {
        &self.triangles
    }
    /// Accepted subdivision bounds and occupancy.
    pub fn stats(&self) -> AdaptivePatchStats {
        self.stats
    }
}
/// Build a mixed patch mesh with capacity admission before every array growth.
/// Replacement admission includes the still-live old allocation. One to four
/// components per corner keep color temporaries inline. Scratch reservations
/// follow their arrays/iterators; only output capacity remains on success.
/// Source parsing, thread stack, credit owners and allocator bookkeeping are excluded.
/// Sorting checks cancellation before/after each sort, not inside comparisons.
pub fn tessellate_patch_mesh_adaptive_admitted(
    patches: &[AdaptivePatchRef<'_>],
    transform: Affine,
    limits: AdaptivePatchLimits,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Result<AdmittedAdaptivePatchMesh, AdaptivePatchError> {
    admitted_mesh(patches.iter().copied(), transform, limits, cancelled, admit)
}
/// Build an admitted tensor mesh without allocating a separate patch-reference
/// array. Capacity ownership and component restrictions match the mixed API.
pub fn tessellate_tensor_patch_mesh_adaptive_admitted(
    patches: &[TensorProductPatch],
    transform: Affine,
    limits: AdaptivePatchLimits,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Result<AdmittedAdaptivePatchMesh, AdaptivePatchError> {
    admitted_mesh(
        patches.iter().map(AdaptivePatchRef::Tensor),
        transform,
        limits,
        cancelled,
        admit,
    )
}
/// Emit compact geometry directly, without a retained generic-triangle intermediate.
/// Original f64 coordinates, f32 components, capacity admission and cancellation are preserved.
pub fn tessellate_tensor_patch_mesh_compact_admitted(
    patches: &[TensorProductPatch],
    transform: Affine,
    limits: AdaptivePatchLimits,
    cancelled: &dyn Fn() -> bool,
    admit: &Admitter<'_>,
) -> Result<AdmittedAdaptivePatchMesh<CompactAdaptiveTriangle>, AdaptivePatchError> {
    admitted_mesh(
        patches.iter().map(AdaptivePatchRef::Tensor),
        transform,
        limits,
        cancelled,
        admit,
    )
}
/// Compact mixed-patch counterpart with the same admission and error bounds.
pub fn tessellate_patch_mesh_compact_admitted(
    patches: &[AdaptivePatchRef<'_>],
    transform: Affine,
    limits: AdaptivePatchLimits,
    cancelled: &dyn Fn() -> bool,
    admit: &Admitter<'_>,
) -> Result<AdmittedAdaptivePatchMesh<CompactAdaptiveTriangle>, AdaptivePatchError> {
    admitted_mesh(patches.iter().copied(), transform, limits, cancelled, admit)
}
/// Stream admitted adaptive triangles in source order without retaining output geometry.
/// Source components are limited to one through four, keeping emitted colors inline.
/// The caller must discard its partial sink on error; planner buffers are released.
pub fn tessellate_patch_mesh_stream_admitted(
    patches: &[AdaptivePatchRef<'_>],
    transform: Affine,
    limits: AdaptivePatchLimits,
    cancelled: &dyn Fn() -> bool,
    admit: &Admitter<'_>,
    consume: &mut dyn FnMut(Triangle) -> Result<(), AdaptivePatchError>,
) -> Result<AdaptivePatchStats, AdaptivePatchError> {
    tessellate_patch_mesh_stream_ordered_admitted(
        patches,
        transform,
        limits,
        cancelled,
        admit,
        &mut |triangle, _source| consume(triangle),
    )
}
/// Compact capture using projected source-component planning and checking.
/// The caller receives no partially checked geometry on refusal or cancellation.
pub fn tessellate_patch_mesh_compact_projected_admitted(
    patches: &[AdaptivePatchRef<'_>],
    transform: Affine,
    limits: AdaptivePatchLimits,
    cancelled: &dyn Fn() -> bool,
    admit: &Admitter<'_>,
) -> Result<AdmittedAdaptivePatchMesh<CompactAdaptiveTriangle>, AdaptivePatchError> {
    let mut output = MeshArray::new(Some(admit));
    let stats = tessellate_patch_mesh_stream_projected_admitted(
        patches,
        transform,
        limits,
        cancelled,
        admit,
        &mut |triangle, _| output.push(CompactAdaptiveTriangle::from_triangle(triangle)),
    )?;
    if cancelled() {
        return Err(AdaptivePatchError::Cancelled);
    }
    let (triangles, credit) = output.into_parts();
    Ok(AdmittedAdaptivePatchMesh {
        triangles,
        stats,
        _credit: credit,
    })
}
/// Source provenance needed for projected overlap ownership. Parameters refer to
/// the three emitted vertices before the device transform; patch order follows
/// the supplied source slice. Folded mappings must compare interpolated v then u,
/// not triangle emission order or device winding.
#[derive(Debug, Clone, Copy)]
pub struct PatchTriangleSource {
    /// Later indices cover earlier patches.
    pub patch_index: usize,
    /// Original (u,v) for p0, p1 and p2, respectively.
    pub parameters: [Point; 3],
}
/// Streaming adaptive tessellation retaining source-order/parameter provenance.
/// No extra retained mesh or per-triangle allocation is introduced. Callback
/// failure/cancellation invalidates the whole consumer result as in the plain API.
pub fn tessellate_patch_mesh_stream_ordered_admitted(
    patches: &[AdaptivePatchRef<'_>],
    transform: Affine,
    limits: AdaptivePatchLimits,
    cancelled: &dyn Fn() -> bool,
    admit: &Admitter<'_>,
    consume: &mut dyn FnMut(Triangle, PatchTriangleSource) -> Result<(), AdaptivePatchError>,
) -> Result<AdaptivePatchStats, AdaptivePatchError> {
    stream_ordered(patches, transform, limits, cancelled, admit, consume, false)
}
/// Opt-in streaming with projected source-component planning and verification
/// of every emitted triangle, including conforming fans. Error bounds concern
/// raw source components on the painted triangle/source overlap, not nonlinear
/// functions, extrapolation, coverage or inverse ownership.
/// On error the caller must discard all partially consumed output.
pub fn tessellate_patch_mesh_stream_projected_admitted(
    patches: &[AdaptivePatchRef<'_>],
    transform: Affine,
    limits: AdaptivePatchLimits,
    cancelled: &dyn Fn() -> bool,
    admit: &Admitter<'_>,
    consume: &mut dyn FnMut(Triangle, PatchTriangleSource) -> Result<(), AdaptivePatchError>,
) -> Result<AdaptivePatchStats, AdaptivePatchError> {
    let mut maximum = 0.0f64;
    let mut skipped = 0usize;
    let mut stats = stream_ordered(
        patches,
        transform,
        limits,
        cancelled,
        admit,
        &mut |triangle, source| {
            if crate::encode::projected_triangle_is_degenerate(&triangle) {
                skipped+=1;
                return Ok(());
            }
            let converted;
            let patch = match patches[source.patch_index] {
                AdaptivePatchRef::Tensor(p) => p,
                AdaptivePatchRef::Coons(p) => {
                    converted = p.adaptive_tensor()?;
                    &converted
                }
            };
            let bound = crate::encode::visible_projected_component_bounds(
                patch, transform, &triangle, source, &mut 4096, cancelled,
            )
            .map_err(projected_error)?;
            let bound = bound.into_iter().fold(0., f64::max);
            if bound > limits.component_error {
                if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
                    eprintln!(
                        "PROJECTED_COMPONENT_REFUSAL patch={} bound={bound} limit={} uv={:?}",
                        source.patch_index, limits.component_error, source.parameters
                    );
                }
                return Err(AdaptivePatchError::ComponentAccuracy);
            }
            maximum = maximum.max(bound);
            consume(triangle, source)
        },
        true,
    )?;
    stats.max_component_bound = stats.max_component_bound.max(maximum);
    stats.triangles = stats.triangles.checked_sub(skipped).ok_or(AdaptivePatchError::Invalid)?;
    Ok(stats)
}
fn projected_error(error: crate::encode::PixelPartitionError) -> AdaptivePatchError {
    use crate::encode::PixelPartitionError as E;
    match error {
        E::Cancelled => AdaptivePatchError::Cancelled,
        E::Admission => AdaptivePatchError::Admission,
        E::Limit => AdaptivePatchError::TriangleLimit,
        E::Invalid => AdaptivePatchError::Invalid,
        E::Conditioning => AdaptivePatchError::ComponentAccuracy,
    }
}
fn stream_ordered(
    patches: &[AdaptivePatchRef<'_>],
    transform: Affine,
    limits: AdaptivePatchLimits,
    cancelled: &dyn Fn() -> bool,
    admit: &Admitter<'_>,
    consume: &mut dyn FnMut(Triangle, PatchTriangleSource) -> Result<(), AdaptivePatchError>,
    projected: bool,
) -> Result<AdaptivePatchStats, AdaptivePatchError> {
    if cancelled() {
        return Err(AdaptivePatchError::Cancelled);
    }
    for patch in patches {
        if cancelled() {
            return Err(AdaptivePatchError::Cancelled);
        }
        let colors = match patch {
            AdaptivePatchRef::Tensor(p) => &p.colors,
            AdaptivePatchRef::Coons(p) => &p.colors,
        };
        let count = colors[0].len();
        if !(1..=4).contains(&count)
            || colors
                .iter()
                .any(|c| c.len() != count || c.iter().any(|v| !v.is_finite()))
        {
            return Err(AdaptivePatchError::Invalid);
        }
    }
    struct Sink<'a> {
        len: usize,
        patch_index: usize,
        projected: bool,
        consume: &'a mut dyn FnMut(Triangle, PatchTriangleSource) -> Result<(), AdaptivePatchError>,
    }
    impl TriangleOutput for Sink<'_> {
        fn len(&self) -> usize {
            self.len
        }
        fn truncate(&mut self, len: usize) {
            self.len = len;
        }
        fn push_triangle(&mut self, _triangle: Triangle) -> Result<(), AdaptivePatchError> {
            // Every adaptive emission must supply its source parameters.
            Err(AdaptivePatchError::Invalid)
        }
        fn begin_patch(&mut self, index: usize) {
            self.patch_index = index;
        }
        fn projected_components(&self) -> bool {
            self.projected
        }
        fn push_patch_triangle(
            &mut self,
            triangle: Triangle,
            parameters: [Point; 3],
        ) -> Result<(), AdaptivePatchError> {
            (self.consume)(
                triangle,
                PatchTriangleSource {
                    patch_index: self.patch_index,
                    parameters,
                },
            )?;
            self.len += 1;
            Ok(())
        }
    }
    tessellate_mesh(
        patches.iter().copied(),
        transform,
        limits,
        &mut Sink {
            len: 0,
            patch_index: 0,
            projected,
            consume,
        },
        None,
        || cancelled(),
        Some(admit),
    )
}
fn admitted_mesh<'a, T: EmittedTriangle>(
    patches: impl ExactSizeIterator<Item = AdaptivePatchRef<'a>> + Clone,
    transform: Affine,
    limits: AdaptivePatchLimits,
    cancelled: &dyn Fn() -> bool,
    admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
) -> Result<AdmittedAdaptivePatchMesh<T>, AdaptivePatchError> {
    use AdaptivePatchError::*;
    if cancelled() {
        return Err(Cancelled);
    }
    if !limits.pixel_error.is_finite()
        || limits.pixel_error <= 0.
        || !limits.component_error.is_finite()
        || limits.component_error <= 0.
        || limits.max_depth > 16
        || limits.max_triangles > 1_048_576
        || transform.as_coeffs().iter().any(|v| !v.is_finite())
    {
        return Err(Invalid);
    }
    if patches.len() > limits.max_triangles / 2 {
        return Err(TriangleLimit);
    }
    for patch in patches.clone() {
        if cancelled() {
            return Err(Cancelled);
        }
        let (points, colors): (&[Point], &[_; 4]) = match patch {
            AdaptivePatchRef::Tensor(p) => (p.control_points.as_slice(), &p.colors),
            AdaptivePatchRef::Coons(p) => (p.control_points.as_slice(), &p.colors),
        };
        let count = colors[0].len();
        if !(1..=4).contains(&count)
            || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite())
            || colors
                .iter()
                .any(|c| c.len() != count || c.iter().any(|v| !v.is_finite()))
        {
            return Err(Invalid);
        }
    }
    let mut triangles: MeshArray<'_, T> = MeshArray::new(Some(admit));
    let stats = tessellate_mesh(
        patches,
        transform,
        limits,
        &mut triangles,
        None,
        || cancelled(),
        Some(admit),
    )?;
    if cancelled() {
        return Err(Cancelled);
    }
    let (triangles, credit) = triangles.into_parts();
    Ok(AdmittedAdaptivePatchMesh {
        triangles,
        stats,
        _credit: credit,
    })
}
// Dry-run the exact conforming fan geometry before publishing output. Only
// failing leaves are forced to split; all shared boundaries are then rebuilt.
struct LocalComponentCheck<'a, 'mem> {
    patch: &'a TensorProductPatch,
    transform: Affine,
    limits: AdaptivePatchLimits,
    patch_index: usize,
    uv: [f64; 4],
    depth: u8,
    failed: bool,
    count: usize,
    failures: &'a mut MeshArray<'mem, (usize, [f64; 4])>,
}
impl TriangleOutput for LocalComponentCheck<'_, '_> {
    fn len(&self) -> usize {
        self.count
    }
    fn truncate(&mut self, len: usize) {
        self.count = len;
    }
    fn push_triangle(&mut self, _: Triangle) -> Result<(), AdaptivePatchError> {
        Err(AdaptivePatchError::Invalid)
    }
    fn begin_leaf(&mut self, uv: [f64; 4], depth: u8) {
        self.uv = uv;
        self.depth = depth;
        self.failed = false;
    }
    fn push_patch_triangle(
        &mut self,
        triangle: Triangle,
        parameters: [Point; 3],
    ) -> Result<(), AdaptivePatchError> {
        self.count += 1;
        if crate::encode::projected_triangle_is_degenerate(&triangle) {
            return Ok(());
        }
        let bound = crate::encode::visible_projected_component_bounds(
            self.patch,
            self.transform,
            &triangle,
            PatchTriangleSource {
                patch_index: self.patch_index,
                parameters,
            },
            &mut 4096,
            &|| false,
        );
        let failed = match bound {
            Ok(values) => values.into_iter().any(|v| v > self.limits.component_error),
            Err(crate::encode::PixelPartitionError::Conditioning) => true,
            Err(error) => return Err(projected_error(error)),
        };
        if failed && !self.failed {
            if self.depth >= self.limits.max_depth {
                return Err(AdaptivePatchError::Depth);
            }
            self.failures.push((self.patch_index, self.uv))?;
            self.failed = true;
        }
        Ok(())
    }
}
fn tessellate_mesh<'a, 'mem>(
    patches: impl ExactSizeIterator<Item = AdaptivePatchRef<'a>> + Clone,
    transform: Affine,
    limits: AdaptivePatchLimits,
    output: &mut impl TriangleOutput,
    mut ranges: Option<&mut Vec<std::ops::Range<usize>>>,
    mut cancelled: impl FnMut() -> bool,
    admit: Option<&'mem Admitter<'mem>>,
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
        let mut edge_records = MeshArray::new(admit);
        for source in patches.clone() {
            if cancelled() {
                return Err(AdaptivePatchError::Cancelled);
            }
            let patch = match source {
                AdaptivePatchRef::Tensor(patch) => Cow::Borrowed(patch),
                AdaptivePatchRef::Coons(patch) => Cow::Owned(patch.adaptive_tensor()?),
            };
            for (key, _, controls) in boundary_controls(&patch).map(canonical_edge) {
                edge_records.push(EdgeRecord { key, controls })?;
            }
        }
        if cancelled() {
            return Err(AdaptivePatchError::Cancelled);
        }
        edge_records.sort_unstable_by_key(|edge| edge.key);
        edge_records.dedup_by(|a, b| a.key == b.key);
        if cancelled() {
            return Err(AdaptivePatchError::Cancelled);
        }
        let mut forced = MeshArray::new(admit);
        for _refinement_pass in 0..=16 {
            let mut planned = MeshArray::new(admit);
            let mut minimum = 0;
            let mut shared: MeshArray<'_, SharedEdgeSample> = MeshArray::new(admit);
            let mut patch_edges = MeshArray::new(admit);
            for (patch_index, source) in patches.clone().enumerate() {
                if cancelled() {
                    return Err(AdaptivePatchError::Cancelled);
                }
                let patch = match source {
                    AdaptivePatchRef::Tensor(patch) => Cow::Borrowed(patch),
                    AdaptivePatchRef::Coons(patch) => Cow::Owned(patch.adaptive_tensor()?),
                };
                let mut available = limits;
                available.max_triangles = limits.max_triangles - minimum;
                let leaves = patch.plan_adaptive(
                    transform,
                    available,
                    &mut cancelled,
                    admit,
                    output.projected_components(),
                    &forced,
                    patch_index,
                )?;
                minimum += leaves.len() * 2;
                let controls = boundary_controls(&patch);
                let edges = controls.map(canonical_edge).map(|(key, reversed, _)| {
                    (
                        edge_records.binary_search_by_key(&key, |edge| edge.key).unwrap(),
                        reversed,
                    )
                });
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
                            let (edge, reversed) = edges[edge];
                            let controls = edge_records[edge].controls;
                            for parameter in values.map(|t| if reversed { 65536 - t } else { t }) {
                                if cancelled() {
                                    return Err(AdaptivePatchError::Cancelled);
                                }
                                let point = transform * curve_point(controls, parameter);
                                if !point.x.is_finite() || !point.y.is_finite() {
                                    return Err(AdaptivePatchError::Invalid);
                                }
                                shared.push(SharedEdgeSample {
                                    edge,
                                    parameter,
                                    point,
                                })?;
                            }
                        }
                    }
                }
                patch_edges.push(edges)?;
                planned.push((patch, leaves))?;
            }
            if cancelled() {
                return Err(AdaptivePatchError::Cancelled);
            }
            // Refine shared cubic boundaries independently of interior leaves.
            // The tighter chord bound reduces silhouette coverage error without
            // multiplying all boundary-adjacent quadtree cells.
            for (edge, record) in edge_records.iter().enumerate() {
                let mut stack = [(record.controls.map(|p| transform * p), 0u32, 65536u32, 0u8); 17];
                let mut count = 1;
                while count > 0 {
                    count -= 1;
                    let (p, low, high, depth) = stack[count];
                    if cancelled() {
                        return Err(AdaptivePatchError::Cancelled);
                    }
                    let deviation = (p[1] - p[0].lerp(p[3], 1. / 3.))
                        .hypot()
                        .max((p[2] - p[0].lerp(p[3], 2. / 3.)).hypot());
                    if deviation <= limits.pixel_error / 128. {
                        if shared.len() >= limits.max_triangles * 4 {
                            return Err(AdaptivePatchError::TriangleLimit);
                        }
                        // Source endpoints already occur in the planned leaves.
                        // Each interior endpoint is emitted once, by its left span.
                        if high != 65536 {
                            shared.push(SharedEdgeSample {
                                edge,
                                parameter: high,
                                point: transform * curve_point(record.controls, high),
                            })?;
                        }
                    } else {
                        if depth >= limits.max_depth {
                            return Err(AdaptivePatchError::Depth);
                        }
                        let (left, right) = split_curve(p);
                        let mid = (low + high) / 2;
                        stack[count] = (right, mid, high, depth + 1);
                        stack[count + 1] = (left, low, mid, depth + 1);
                        count += 2;
                    }
                }
            }
            shared.sort_unstable_by_key(|sample| (sample.edge, sample.parameter));
            shared.dedup_by(|a, b| a.edge == b.edge && a.parameter == b.parameter);
            // Drop duplicate/capacity slack before output growth. Full old+new capacity
            // is admitted while copying; output triangles do not yet overlap this stage.
            shared.compact()?;
            if cancelled() {
                return Err(AdaptivePatchError::Cancelled);
            }
            if output.projected_components() {
                let mut failures = MeshArray::new(admit);
                let mut checked = 0;
                for (patch_index, ((patch, leaves), edges)) in
                    planned.iter().zip(patch_edges.iter()).enumerate()
                {
                    let boundaries = (*edges).map(|(edge, reversed)| Boundary {
                        reversed,
                        points: &shared[shared.partition_point(|sample| sample.edge < edge)
                            ..shared.partition_point(|sample| sample.edge <= edge)],
                    });
                    let mut remaining = limits;
                    remaining.max_triangles = limits
                        .max_triangles
                        .checked_sub(checked)
                        .ok_or(AdaptivePatchError::TriangleLimit)?;
                    let mut checker = LocalComponentCheck {
                        patch,
                        transform,
                        limits,
                        patch_index,
                        uv: [0.; 4],
                        depth: 0,
                        failed: false,
                        count: 0,
                        failures: &mut failures,
                    };
                    let stats = patch.emit_adaptive(
                        leaves,
                        transform,
                        remaining,
                        &mut checker,
                        Some(&boundaries),
                        &mut cancelled,
                        admit,
                    )?;
                    checked += stats.triangles;
                }
                if !failures.is_empty() {
                    for failure in failures {
                        if forced.iter().any(|old| *old == failure) {
                            return Err(AdaptivePatchError::ComponentAccuracy);
                        }
                        forced.push(failure)?;
                    }
                    if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
                        eprintln!(
                            "LOCAL_COMPONENT_REFINEMENT pass={} forced_leaves={}",
                            _refinement_pass,
                            forced.len()
                        );
                    }
                    continue;
                }
            }
            // Canonical controls are no longer read after shared samples are finalized.
            drop(edge_records);
            // Managed output is one retained allocation. Count all patch fans before
            // materializing it, so later patches cannot duplicate a growing prefix.
            if output.plans_global_capacity() {
                let mut count = 0usize;
                for ((_, leaves), edges) in planned.iter().zip(patch_edges.iter()) {
                    if cancelled() {
                        return Err(AdaptivePatchError::Cancelled);
                    }
                    let boundaries = (*edges).map(|(edge, reversed)| Boundary {
                        reversed,
                        points: &shared[shared.partition_point(|sample| sample.edge < edge)
                            ..shared.partition_point(|sample| sample.edge <= edge)],
                    });
                    let (_, _, triangles) =
                        plan_leaf_edges(leaves, Some(&boundaries), limits, &mut cancelled, admit, true)?;
                    count = count
                        .checked_add(triangles)
                        .ok_or(AdaptivePatchError::TriangleLimit)?;
                    if count > limits.max_triangles {
                        return Err(AdaptivePatchError::TriangleLimit);
                    }
                }
                output.reserve_patch(count)?;
            }
            let mut total = AdaptivePatchStats::default();
            for (patch_index, ((patch, leaves), edges)) in planned.into_iter().zip(patch_edges).enumerate() {
                output.begin_patch(patch_index);
                let boundaries = edges.map(|(edge, reversed)| Boundary {
                    reversed,
                    points: &shared[shared.partition_point(|sample| sample.edge < edge)
                        ..shared.partition_point(|sample| sample.edge <= edge)],
                });
                let mut remaining = limits;
                remaining.max_triangles -= total.triangles;
                let patch_start = output.len();
                let stats = patch.emit_adaptive(
                    &leaves,
                    transform,
                    remaining,
                    output,
                    Some(&boundaries),
                    &mut cancelled,
                    admit,
                )?;
                if let Some(ranges) = &mut ranges {
                    push_array(ranges, patch_start..output.len())?;
                }
                total.triangles += stats.triangles;
                total.deepest = total.deepest.max(stats.deepest);
                total.max_pixel_bound = total.max_pixel_bound.max(stats.max_pixel_bound);
                total.max_component_bound = total.max_component_bound.max(stats.max_component_bound);
            }
            total.refinement_passes = _refinement_pass as u8;
            total.refined_leaves = forced.len();
            return Ok(total);
        }
        Err(AdaptivePatchError::Depth)
    })();
    if result.is_err() {
        output.truncate(initial);
        if let Some(ranges) = &mut ranges {
            ranges.truncate(initial_ranges);
        }
    }
    result
}
fn plan_leaf_edges<'mem>(
    leaves: &[Leaf],
    boundaries: Option<&[Boundary<'_>; 4]>,
    limits: AdaptivePatchLimits,
    cancelled: &mut impl FnMut() -> bool,
    admit: Option<&'mem Admitter<'mem>>,
    count_output: bool,
) -> Result<(Edges<'mem>, Edges<'mem>, usize), AdaptivePatchError> {
    let mut vertical = Edges::new(admit);
    let mut horizontal = Edges::new(admit);
    for leaf in leaves {
        if cancelled() {
            return Err(AdaptivePatchError::Cancelled);
        }
        let [u0, u1, v0, v1] = leaf.uv.map(|v| (v * PARAM_SCALE).round() as u32);
        for x in [u0, u1] {
            for y in [v0, v1] {
                vertical.insert(x, y)?;
                horizontal.insert(y, x)?;
            }
        }
    }
    if let Some(boundaries) = boundaries {
        for (edge, boundary) in boundaries.iter().enumerate() {
            if cancelled() {
                return Err(AdaptivePatchError::Cancelled);
            }
            let values = boundary
                .points
                .iter()
                .map(|sample| boundary.local(sample.parameter));
            match edge {
                0 => horizontal.extend(0, values),
                1 => vertical.extend(65536, values),
                2 => horizontal.extend(65536, values),
                _ => vertical.extend(0, values),
            }?;
        }
    }
    if cancelled() {
        return Err(AdaptivePatchError::Cancelled);
    }
    vertical.finish()?;
    horizontal.finish()?;
    // The shared-edge plan fixes the exact fan count. Reserve once before
    // emitting instead of retaining successive old/new triangle capacities.
    let mut triangle_count = 0usize;
    if count_output {
        for leaf in leaves {
            if cancelled() {
                return Err(AdaptivePatchError::Cancelled);
            }
            let [x0, x1, y0, y1] = leaf.uv.map(|v| (v * PARAM_SCALE).round() as u32);
            let count = interior_count(&horizontal, y0, x0, x1)
                + interior_count(&horizontal, y1, x0, x1)
                + interior_count(&vertical, x0, y0, y1)
                + interior_count(&vertical, x1, y0, y1);
            triangle_count = triangle_count
                .checked_add(if count == 0 { 2 } else { 6 + count })
                .ok_or(AdaptivePatchError::TriangleLimit)?;
            if triangle_count > limits.max_triangles {
                return Err(AdaptivePatchError::TriangleLimit);
            }
        }
    }
    Ok((vertical, horizontal, triangle_count))
}
fn interior_count(edges: &Edges<'_>, fixed: u32, low: u32, high: u32) -> usize {
    let start = edges.points.partition_point(|&point| point <= (fixed, low));
    let end = edges.points.partition_point(|&point| point < (fixed, high));
    end - start
}
fn interior<'mem>(
    edges: &Edges<'mem>,
    fixed: u32,
    low: u32,
    high: u32,
) -> Result<MeshArray<'mem, u32>, AdaptivePatchError> {
    let start = edges.points.partition_point(|&point| point <= (fixed, low));
    let end = edges.points.partition_point(|&point| point < (fixed, high));
    let mut result = MeshArray::new(edges.points.admit);
    result.extend(edges.points[start..end].iter().map(|&(_, value)| value))?;
    Ok(result)
}

fn patch_vertex(
    patch: &TensorProductPatch,
    transform: Affine,
    uv: Point,
    boundaries: Option<&[Boundary<'_>; 4]>,
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
        edges[edge]
            .points
            .binary_search_by_key(&edges[edge].local(local), |sample| sample.parameter)
            .ok()
            .map(|index| edges[edge].points[index].point)
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
    output: &mut impl TriangleOutput,
    boundaries: Option<&[Boundary<'_>; 4]>,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), AdaptivePatchError> {
    let center_parameter = center;
    let center = patch_vertex(patch, transform, center, boundaries)?;
    for index in 0..polygon.len() {
        if cancelled() {
            return Err(AdaptivePatchError::Cancelled);
        }
        output.push_patch_triangle(
            Triangle::new(
                center.clone(),
                patch_vertex(patch, transform, polygon[index], boundaries)?,
                patch_vertex(patch, transform, polygon[(index + 1) % polygon.len()], boundaries)?,
            ),
            [
                center_parameter,
                polygon[index],
                polygon[(index + 1) % polygon.len()],
            ],
        )?;
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
fn split_grid_axis(grid: Grid, axis: usize) -> [Grid; 2] {
    let mut result = [[[Point::ZERO; 4]; 4]; 2];
    for j in 0..4 {
        let curve = if axis == 0 {
            std::array::from_fn(|i| grid[i][j])
        } else {
            grid[j]
        };
        let (a, b) = split_curve(curve);
        for (side, curve) in [a, b].into_iter().enumerate() {
            for i in 0..4 {
                if axis == 0 {
                    result[side][i][j] = curve[i];
                } else {
                    result[side][j][i] = curve[i];
                }
            }
        }
    }
    result
}
// Refinement heuristic only; actual emitted component bounds remain mandatory.
fn dominant_curvature(grid: &Grid) -> Option<usize> {
    let mut u = 0.0f64;
    let mut v = 0.0f64;
    for i in 1..3 {
        for j in 0..4 {
            u = u.max(((grid[i + 1][j] - grid[i][j]) - (grid[i][j] - grid[i - 1][j])).hypot());
            v = v.max(((grid[j][i + 1] - grid[j][i]) - (grid[j][i] - grid[j][i - 1])).hypot());
        }
    }
    if u > 2. * v {
        Some(0)
    } else if v > 2. * u {
        Some(1)
    } else {
        None
    }
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
            let leaves = self.plan_adaptive(transform, limits, &mut cancelled, None, false, &[], 0)?;
            self.emit_adaptive(&leaves, transform, limits, output, None, &mut cancelled, None)
        })();
        if result.is_err() {
            output.truncate(initial);
        }
        result
    }
    fn plan_adaptive<'mem>(
        &self,
        transform: Affine,
        limits: AdaptivePatchLimits,
        cancelled: &mut impl FnMut() -> bool,
        admit: Option<&'mem Admitter<'mem>>,
        projected: bool,
        forced: &[(usize, [f64;4])],
        patch_index: usize,
    ) -> Result<MeshArray<'mem, Leaf>, AdaptivePatchError> {
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
        let mut leaves = MeshArray::new(admit);
        // DFS keeps at most three siblings per level plus the current cell.
        // Depth <=16 was validated above, so planning needs no heap stack.
        let mut pending: [Option<Cell>; 49] = std::array::from_fn(|_| None);
        pending[0] = Some(Cell {
            grid,
            uv: [0., 1., 0., 1.],
            depth: 0,
        });
        let mut pending_len = 1;
        while pending_len > 0 {
            pending_len -= 1;
            let cell = pending[pending_len].take().ok_or(AdaptivePatchError::Invalid)?;
            if cancelled() {
                return Err(AdaptivePatchError::Cancelled);
            }
            let [u0, u1, v0, v1] = cell.uv;
            let geometry = pixel_bound(&cell.grid);
            let param_color = cross * (u1 - u0) * (v1 - v0) * 0.25 + 256. * f64::from(f32::EPSILON) * scale;
            let mut color = param_color;
            if projected && geometry <= limits.pixel_error && color <= limits.component_error {
                let points = [
                    Point::new(u0, v0),
                    Point::new(u1, v0),
                    Point::new(u0, v1),
                    Point::new(u1, v1),
                ];
                let a = points[0];
                let b = points[1];
                let c = points[2];
                let d = points[3];
                let first = Point::new(u0 + (u1 - u0) / 3., v0 + (v1 - v0) / 3.);
                let second = Point::new(u0 + 2. * (u1 - u0) / 3., v0 + 2. * (v1 - v0) / 3.);
                for uv in [
                    [a, b, c],
                    [b, d, c],
                    [first, a, b],
                    [first, b, c],
                    [first, c, a],
                    [second, b, d],
                    [second, d, c],
                    [second, c, b],
                ] {
                    let [a, b, c] = uv.map(|p| patch_vertex(self, transform, p, None));
                    let triangle = Triangle::new(a?, b?, c?);
                    if crate::encode::projected_triangle_is_degenerate(&triangle) { continue; }
                    // Planning uses the full cell rectangle for this conservative
                    // preliminary bound. The actual emitted UV box is checked again.
                    let source = PatchTriangleSource {
                        patch_index: 0,
                        parameters: uv,
                    };
                    let cancelled_read = std::cell::RefCell::new(&mut *cancelled);
                    match crate::encode::visible_projected_component_bounds(
                        self,
                        transform,
                        &triangle,
                        source,
                        &mut 4096,
                        &|| (**cancelled_read.borrow_mut())(),
                    ) {
                        Ok(bounds) => color = color.max(bounds.into_iter().fold(0., f64::max)),
                        Err(crate::encode::PixelPartitionError::Conditioning) => color = f64::INFINITY,
                        Err(error) => return Err(projected_error(error)),
                    }
                }
            }
            if geometry <= limits.pixel_error && color <= limits.component_error
                && !forced.iter().any(|(index, uv)| *index == patch_index && *uv == cell.uv)
            {
                if (leaves.len() + 1) * 2 > limits.max_triangles {
                    return Err(AdaptivePatchError::TriangleLimit);
                }
                leaves.push(Leaf {
                    uv: cell.uv,
                    depth: cell.depth,
                    geometry,
                    color,
                })?;
            } else {
                if cell.depth >= limits.max_depth {
                    if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
                        eprintln!("ADAPTIVE_DEPTH_REFUSAL uv={:?} geometry={geometry} color={color} param_color={param_color} projected={projected}", cell.uv);
                    }
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
                let axis =
                    if projected && geometry <= limits.pixel_error && param_color <= limits.component_error {
                        dominant_curvature(&cell.grid)
                    } else {
                        None
                    };
                let children = match axis {
                    Some(0) => {
                        let [a, b] = split_grid_axis(cell.grid, 0);
                        [
                            Some((a, [u0, um, v0, v1])),
                            Some((b, [um, u1, v0, v1])),
                            None,
                            None,
                        ]
                    }
                    Some(1) => {
                        let [a, b] = split_grid_axis(cell.grid, 1);
                        [
                            Some((a, [u0, u1, v0, vm])),
                            Some((b, [u0, u1, vm, v1])),
                            None,
                            None,
                        ]
                    }
                    _ => {
                        let grids = split_grid(cell.grid);
                        std::array::from_fn(|i| Some((grids[i], uv[i])))
                    }
                };
                for (grid, uv) in children.into_iter().rev().flatten() {
                    if pending_len >= pending.len() {
                        return Err(AdaptivePatchError::Invalid);
                    }
                    pending[pending_len] = Some(Cell {
                        grid,
                        uv,
                        depth: cell.depth + 1,
                    });
                    pending_len += 1;
                }
            }
        }
        Ok(leaves)
    }
    fn emit_adaptive<'mem>(
        &self,
        leaves: &MeshArray<'mem, Leaf>,
        transform: Affine,
        limits: AdaptivePatchLimits,
        output: &mut impl TriangleOutput,
        boundaries: Option<&[Boundary<'_>; 4]>,
        cancelled: &mut impl FnMut() -> bool,
        admit: Option<&'mem Admitter<'mem>>,
    ) -> Result<AdaptivePatchStats, AdaptivePatchError> {
        // Managed meshes reserve their complete output before reaching this
        // stage. Streaming/unmanaged sinks do not need a capacity-count pass.
        let (vertical, horizontal, _) =
            plan_leaf_edges(&leaves, boundaries, limits, cancelled, admit, false)?;
        if cancelled() {
            return Err(AdaptivePatchError::Cancelled);
        }
        let mut stats = AdaptivePatchStats::default();
        for leaf in leaves {
            if cancelled() {
                return Err(AdaptivePatchError::Cancelled);
            }
            output.begin_leaf(leaf.uv, leaf.depth);
            let [u0, u1, v0, v1] = leaf.uv;
            let [x0, x1, y0, y1] = leaf.uv.map(|v| (v * PARAM_SCALE).round() as u32);
            let bottom = interior(&horizontal, y0, x0, x1)?;
            let top = interior(&horizontal, y1, x0, x1)?;
            let left = interior(&vertical, x0, y0, y1)?;
            let right = interior(&vertical, x1, y0, y1)?;
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
                let parameters = [a, b, c, d];
                let a = patch_vertex(self, transform, a, boundaries)?;
                let b = patch_vertex(self, transform, b, boundaries)?;
                let c = patch_vertex(self, transform, c, boundaries)?;
                let d = patch_vertex(self, transform, d, boundaries)?;
                output.push_patch_triangle(
                    Triangle::new(a, b.clone(), c.clone()),
                    [parameters[0], parameters[1], parameters[2]],
                )?;
                output.push_patch_triangle(
                    Triangle::new(b, d, c),
                    [parameters[1], parameters[3], parameters[2]],
                )?;
            } else {
                let mut first = MeshArray::new(admit);
                first.push(a)?;
                first.extend(
                    bottom
                        .into_iter()
                        .map(|x| Point::new(f64::from(x) / PARAM_SCALE, v0)),
                )?;
                first.extend([b, c])?;
                first.extend(
                    left.into_iter()
                        .rev()
                        .map(|y| Point::new(u0, f64::from(y) / PARAM_SCALE)),
                )?;
                let mut second = MeshArray::new(admit);
                second.push(b)?;
                second.extend(
                    right
                        .into_iter()
                        .map(|y| Point::new(u1, f64::from(y) / PARAM_SCALE)),
                )?;
                second.push(d)?;
                second.extend(
                    top.into_iter()
                        .rev()
                        .map(|x| Point::new(f64::from(x) / PARAM_SCALE, v1)),
                )?;
                second.push(c)?;
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
    pub(crate) fn adaptive_tensor(&self) -> Result<TensorProductPatch, AdaptivePatchError> {
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
    fn patch(x: f64, colors: [f32; 4]) -> TensorProductPatch {
        let indices = [[0, 1, 2, 3], [11, 12, 13, 4], [10, 15, 14, 5], [9, 8, 7, 6]];
        let mut control_points = [Point::ZERO; 16];
        for u in 0..4 {
            for v in 0..4 {
                control_points[indices[u][v]] = Point::new(x + u as f64 * 8. / 3., v as f64 * 8. / 3.);
            }
        }
        TensorProductPatch {
            control_points,
            colors: colors.map(|v| smallvec::smallvec![v]),
        }
    }
    fn limits() -> AdaptivePatchLimits {
        AdaptivePatchLimits {
            pixel_error: 0.01,
            component_error: 0.01,
            max_depth: 8,
            max_triangles: 4096,
        }
    }
    #[test]
    fn streamed_mesh_matches_owned_order_and_releases_on_consumer_failure_and_cancel() {
        use std::{
            cell::Cell,
            sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            },
        };
        struct Owner(Arc<AtomicUsize>, usize);
        impl Drop for Owner {
            fn drop(&mut self) {
                self.0.fetch_sub(self.1, Ordering::Relaxed);
            }
        }
        let used = Arc::new(AtomicUsize::new(0));
        let calls = Cell::new(0usize);
        let refuse = Cell::new(usize::MAX);
        let admit = |bytes| -> Option<Box<dyn std::any::Any + Send + Sync>> {
            calls.set(calls.get() + 1);
            if calls.get() == refuse.get() {
                return None;
            }
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Owner(used.clone(), bytes)))
        };
        let mut source = patch(0., [0.125, 0.25, 0.75, 1.]);
        source.control_points[1].x -= 0.25;
        let refs = [AdaptivePatchRef::Tensor(&source)];
        let mut expected = Vec::new();
        tessellate_patch_mesh_adaptive(&refs, Affine::IDENTITY, limits(), &mut expected, || false).unwrap();
        assert!(!expected.is_empty());
        let mut actual = Vec::new();
        let stats = tessellate_patch_mesh_stream_admitted(
            &refs,
            Affine::IDENTITY,
            limits(),
            &|| false,
            &admit,
            &mut |triangle| {
                actual.push(triangle);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(stats.triangles, expected.len());
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(&expected) {
            for (a, b) in [&a.p0, &a.p1, &a.p2].iter().zip([&b.p0, &b.p1, &b.p2]) {
                assert_eq!(
                    (a.point.x.to_bits(), a.point.y.to_bits(), a.flag),
                    (b.point.x.to_bits(), b.point.y.to_bits(), b.flag)
                );
                assert_eq!(
                    a.colors.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    b.colors.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                );
            }
        }
        assert_eq!(used.load(Ordering::Relaxed), 0);
        let mut seen = 0;
        assert_eq!(
            tessellate_patch_mesh_stream_admitted(
                &refs,
                Affine::IDENTITY,
                limits(),
                &|| false,
                &admit,
                &mut |_| {
                    seen += 1;
                    if seen == expected.len() / 2 {
                        Err(AdaptivePatchError::Admission)
                    } else {
                        Ok(())
                    }
                }
            )
            .err(),
            Some(AdaptivePatchError::Admission)
        );
        assert!(seen > 0 && seen < expected.len());
        assert_eq!(used.load(Ordering::Relaxed), 0);
        let cancelled = Cell::new(false);
        assert_eq!(
            tessellate_patch_mesh_stream_admitted(
                &refs,
                Affine::IDENTITY,
                limits(),
                &|| cancelled.get(),
                &admit,
                &mut |_| {
                    cancelled.set(true);
                    Ok(())
                }
            )
            .err(),
            Some(AdaptivePatchError::Cancelled)
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
        calls.set(0);
        refuse.set(1);
        assert_eq!(
            tessellate_patch_mesh_stream_admitted(
                &refs,
                Affine::IDENTITY,
                limits(),
                &|| false,
                &admit,
                &mut |_| Ok(())
            )
            .err(),
            Some(AdaptivePatchError::Admission)
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
        refuse.set(usize::MAX);
        cancelled.set(false);
        let mut retry = 0;
        tessellate_patch_mesh_stream_admitted(
            &refs,
            Affine::IDENTITY,
            limits(),
            &|| false,
            &admit,
            &mut |_| {
                retry += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(retry, expected.len());
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn compact_mesh_is_bit_exact_and_releases_every_refused_allocation() {
        use std::{
            cell::Cell,
            sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            },
        };
        struct Owner(Arc<AtomicUsize>, usize);
        impl Drop for Owner {
            fn drop(&mut self) {
                self.0.fetch_sub(self.1, Ordering::Relaxed);
            }
        }
        let used = Arc::new(AtomicUsize::new(0));
        let calls = Cell::new(0usize);
        let refuse = Cell::new(usize::MAX);
        let admit = |bytes| -> Option<Box<dyn std::any::Any + Send + Sync>> {
            calls.set(calls.get() + 1);
            if calls.get() == refuse.get() {
                return None;
            }
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Owner(used.clone(), bytes)))
        };
        let mut source = patch(0., [0.25; 4]);
        source.control_points[1].x = -0.1;
        source.colors = std::array::from_fn(|_| smallvec::smallvec![0.25, 2., -0., -1.]);
        let legacy = tessellate_tensor_patch_mesh_adaptive_admitted(
            &[source.clone()],
            Affine::IDENTITY,
            limits(),
            &|| false,
            &admit,
        )
        .unwrap();
        calls.set(0);
        let compact = tessellate_tensor_patch_mesh_compact_admitted(
            &[source.clone()],
            Affine::IDENTITY,
            limits(),
            &|| false,
            &admit,
        )
        .unwrap();
        let reservations = calls.get();
        assert_eq!(size_of::<CompactAdaptiveTriangle>(), 104);
        assert!(size_of::<CompactAdaptiveTriangle>() < size_of::<Triangle>());
        assert_eq!(legacy.triangles().len(), compact.triangles().len());
        for (a, b) in legacy.triangles().iter().zip(compact.triangles()) {
            assert_eq!(b.component_count(), 4);
            for (i, v) in [&a.p0, &a.p1, &a.p2].iter().enumerate() {
                assert_eq!(
                    [v.point.x.to_bits(), v.point.y.to_bits()],
                    [b.points()[i].x.to_bits(), b.points()[i].y.to_bits()]
                );
                assert_eq!(
                    v.colors.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    b.components()[i].map(f32::to_bits)
                );
            }
        }
        drop(legacy);
        drop(compact);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        for failed in 1..=reservations {
            calls.set(0);
            refuse.set(failed);
            assert!(matches!(
                tessellate_tensor_patch_mesh_compact_admitted(
                    &[source.clone()],
                    Affine::IDENTITY,
                    limits(),
                    &|| false,
                    &admit
                ),
                Err(AdaptivePatchError::Admission)
            ));
            assert_eq!(used.load(Ordering::Relaxed), 0, "reservation{failed}");
        }
        refuse.set(usize::MAX);
        let seen = Cell::new(false);
        assert!(matches!(
            tessellate_tensor_patch_mesh_compact_admitted(
                &[source],
                Affine::IDENTITY,
                limits(),
                &|| {
                    let admitted = used.load(Ordering::Relaxed) > 0;
                    seen.set(seen.get() || admitted);
                    admitted
                },
                &admit
            ),
            Err(AdaptivePatchError::Cancelled)
        ));
        assert!(seen.get());
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn curved_boundary_area_matches_analytic_cubic_integral() {
        let mut source = patch(0., [0.25; 4]);
        source.control_points[1].x = -0.1;
        source.control_points[2].x = -0.1;
        let mut triangles = Vec::new();
        tessellate_tensor_patch_mesh_adaptive(&[source], Affine::IDENTITY, limits(), &mut triangles, || {
            false
        })
        .unwrap();
        let area: f64 = triangles
            .iter()
            .map(|t| ((t.p1.point - t.p0.point).cross(t.p2.point - t.p0.point)).abs() * 0.5)
            .sum();
        // Left cubic: x(t)=-0.3*t*(1-t), y(t)=8*t.
        // Its signed strip integral is -0.4, so the patch area is 64.4.
        assert!((area - 64.4).abs() < 0.0002, "area={area}");
    }
    #[test]
    fn bilinear_source_component_is_within_bound_at_independent_dense_samples() {
        let source = patch(0., [0., 0., 1., 0.]);
        let mut output = Vec::new();
        let stats = source
            .to_triangles_adaptive(Affine::IDENTITY, limits(), &mut output, || false)
            .unwrap();
        assert_eq!(stats.triangles, output.len());
        assert!(stats.max_pixel_bound <= 0.01 && stats.max_component_bound <= 0.01);
        for y in 0..99 {
            for x in 0..99 {
                let u = (x as f64 + 0.317) / 99.;
                let v = (y as f64 + 0.193) / 99.;
                let point = Point::new(u * 8., v * 8.);
                let triangle = output
                    .iter()
                    .find(|t| t.contains_point(point))
                    .expect("no holes in planar patch");
                let actual = f64::from(triangle.interpolate(point)[0]);
                assert!((actual - u * v).abs() <= stats.max_component_bound + 1e-7);
            }
        }
    }
    #[test]
    fn unequal_patch_subdivision_welds_shared_boundary_and_preserves_source_color() {
        let mut left = patch(0., [0.125; 4]);
        left.control_points[12].y += 2.;
        let right = patch(8., [0.875; 4]);
        let patches = [AdaptivePatchRef::Tensor(&left), AdaptivePatchRef::Tensor(&right)];
        let mut output = Vec::new();
        let mut ranges = Vec::new();
        tessellate_patch_mesh_adaptive_with_ranges(
            &patches,
            Affine::IDENTITY,
            limits(),
            &mut output,
            &mut ranges,
            || false,
        )
        .unwrap();
        assert_eq!(ranges.len(), 2);
        assert!(ranges[0].len() > ranges[1].len());
        let boundary = |range: std::ops::Range<usize>| {
            let mut points = Vec::new();
            for triangle in &output[range] {
                for vertex in [&triangle.p0, &triangle.p1, &triangle.p2] {
                    if vertex.point.x == 8. {
                        points.push((vertex.point.x.to_bits(), vertex.point.y.to_bits()));
                    }
                }
            }
            points.sort_unstable();
            points.dedup();
            points
        };
        let shared = boundary(ranges[0].clone());
        assert!(shared.len() > 2);
        assert_eq!(shared, boundary(ranges[1].clone()));
        for (range, expected) in ranges.iter().zip([0.125, 0.875]) {
            for triangle in &output[range.clone()] {
                for vertex in [&triangle.p0, &triangle.p1, &triangle.p2] {
                    assert_eq!(vertex.colors[0], expected);
                }
            }
        }
    }
    #[test]
    fn refined_curved_shared_edge_is_bit_exact_when_reversed() {
        let mut left = patch(0., [0.125; 4]);
        let mut right = patch(8., [0.875; 4]);
        for i in [8, 7] {
            left.control_points[i].x += 0.1;
        }
        for i in [1, 2] {
            right.control_points[i].x += 0.1;
        }
        let original = right.control_points;
        let indices = [[0, 1, 2, 3], [11, 12, 13, 4], [10, 15, 14, 5], [9, 8, 7, 6]];
        for u in 0..4 {
            for v in 0..4 {
                right.control_points[indices[u][v]] = original[indices[u][3 - v]];
            }
        }
        let mut triangles = Vec::new();
        let mut ranges = Vec::new();
        tessellate_patch_mesh_adaptive_with_ranges(
            &[AdaptivePatchRef::Tensor(&left), AdaptivePatchRef::Tensor(&right)],
            Affine::IDENTITY,
            limits(),
            &mut triangles,
            &mut ranges,
            || false,
        )
        .unwrap();
        let boundary = |range: std::ops::Range<usize>| {
            let mut points = Vec::new();
            for triangle in &triangles[range] {
                for v in [&triangle.p0, &triangle.p1, &triangle.p2] {
                    let t = v.point.y / 8.;
                    if (v.point.x - (8. + 0.3 * t * (1. - t))).abs() < 1e-12 {
                        points.push((v.point.x.to_bits(), v.point.y.to_bits()));
                    }
                }
            }
            points.sort_unstable();
            points.dedup();
            points
        };
        let a = boundary(ranges[0].clone());
        assert!(a.len() > 8);
        assert_eq!(a, boundary(ranges[1].clone()));
    }
    #[test]
    fn reversed_identical_edges_weld_but_near_distinct_edges_do_not() {
        let mut left = patch(0., [0.125; 4]);
        left.control_points[12].y += 2.;
        for offset in [0., 1e-10] {
            let mut right = patch(8. + offset, [0.875; 4]);
            let original = right.control_points;
            let indices = [[0, 1, 2, 3], [11, 12, 13, 4], [10, 15, 14, 5], [9, 8, 7, 6]];
            for u in 0..4 {
                for v in 0..4 {
                    right.control_points[indices[u][v]] = original[indices[u][3 - v]];
                }
            }
            let patches = [AdaptivePatchRef::Tensor(&left), AdaptivePatchRef::Tensor(&right)];
            let mut output = Vec::new();
            let mut ranges = Vec::new();
            tessellate_patch_mesh_adaptive_with_ranges(
                &patches,
                Affine::IDENTITY,
                limits(),
                &mut output,
                &mut ranges,
                || false,
            )
            .unwrap();
            let points = |range: std::ops::Range<usize>, x: f64| {
                let mut result = Vec::new();
                for triangle in &output[range] {
                    for vertex in [&triangle.p0, &triangle.p1, &triangle.p2] {
                        if vertex.point.x == x {
                            result.push(vertex.point.y.to_bits());
                        }
                    }
                }
                result.sort_unstable();
                result.dedup();
                result
            };
            if offset == 0. {
                assert!(
                    ranges[1].len() > 2,
                    "matching reversed edge must inherit finer subdivision"
                );
                assert_eq!(points(ranges[0].clone(), 8.), points(ranges[1].clone(), 8.));
            } else {
                assert_eq!(
                    ranges[1].len(),
                    2,
                    "geometric tolerance must not weld distinct curves"
                );
            }
        }
    }
    #[test]
    fn fixed_planning_stack_reaches_maximum_depth_without_capacity_failure() {
        let source = patch(0., [0.; 4]);
        for depth in [0, 1, 15, 16] {
            let mut tight = limits();
            tight.max_depth = depth;
            tight.component_error = 1e-12;
            let mut output = Vec::new();
            assert_eq!(
                source
                    .to_triangles_adaptive(Affine::IDENTITY, tight, &mut output, || false)
                    .err(),
                Some(AdaptivePatchError::Depth)
            );
            assert!(output.is_empty());
        }
    }
    struct Credit(std::sync::Arc<std::sync::atomic::AtomicUsize>, usize);
    impl Drop for Credit {
        fn drop(&mut self) {
            self.0.fetch_sub(self.1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    #[test]
    fn admitted_mesh_refusal_retry_capacity_and_original_components_match_legacy() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut left = patch(0., [0.125; 4]);
        left.control_points[12].y += 2.;
        left.control_points[1].x -= 0.1;
        let right = patch(8., [0.875; 4]);
        let coons = super::super::CoonsPatch {
            control_points: right.control_points[..12].try_into().unwrap(),
            colors: right.colors.clone(),
        };
        let patches = [AdaptivePatchRef::Tensor(&left), AdaptivePatchRef::Coons(&coons)];
        let used = Arc::new(AtomicUsize::new(0));
        let peak = std::cell::Cell::new(0);
        let limit = std::cell::Cell::new(0);
        let calls = std::cell::Cell::new(0);
        let refuse = std::cell::Cell::new(usize::MAX);
        let admit = |bytes| {
            calls.set(calls.get() + 1);
            let next = used.load(Ordering::Relaxed) + bytes;
            if next > limit.get() || calls.get() == refuse.get() {
                return None;
            }
            used.fetch_add(bytes, Ordering::Relaxed);
            peak.set(peak.get().max(next));
            Some(Box::new(Credit(used.clone(), bytes)) as Box<dyn std::any::Any + Send + Sync>)
        };
        assert_eq!(
            tessellate_patch_mesh_adaptive_admitted(&patches, Affine::IDENTITY, limits(), &|| false, &admit)
                .err(),
            Some(AdaptivePatchError::Admission)
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        limit.set(usize::MAX);
        calls.set(0);
        let mesh =
            tessellate_patch_mesh_adaptive_admitted(&patches, Affine::IDENTITY, limits(), &|| false, &admit)
                .unwrap();
        let reservations = calls.get();
        let required_peak = peak.get();
        let retained = used.load(Ordering::Relaxed);
        assert_eq!(retained, mesh.triangles.capacity() * size_of::<Triangle>());
        assert!(retained > 0 && retained < required_peak);
        let mut expected = Vec::new();
        let stats =
            tessellate_patch_mesh_adaptive(&patches, Affine::IDENTITY, limits(), &mut expected, || false)
                .unwrap();
        assert_eq!(mesh.stats().triangles, stats.triangles);
        assert_eq!(mesh.triangles().len(), expected.len());
        for (actual, expected) in mesh.triangles().iter().zip(&expected) {
            for (a, b) in [&actual.p0, &actual.p1, &actual.p2].into_iter().zip([
                &expected.p0,
                &expected.p1,
                &expected.p2,
            ]) {
                assert_eq!(
                    (a.point.x.to_bits(), a.point.y.to_bits()),
                    (b.point.x.to_bits(), b.point.y.to_bits())
                );
                assert_eq!(a.colors, b.colors);
            }
        }
        drop(mesh);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        // Every allocation boundary must release all earlier live buffers on refusal.
        for target in 1..=reservations {
            calls.set(0);
            refuse.set(target);
            assert_eq!(
                tessellate_patch_mesh_adaptive_admitted(
                    &patches,
                    Affine::IDENTITY,
                    limits(),
                    &|| false,
                    &admit
                )
                .err(),
                Some(AdaptivePatchError::Admission)
            );
            assert_eq!(used.load(Ordering::Relaxed), 0, "reservation {target}");
        }
        refuse.set(usize::MAX);
        limit.set(required_peak - 1);
        calls.set(0);
        assert_eq!(
            tessellate_patch_mesh_adaptive_admitted(&patches, Affine::IDENTITY, limits(), &|| false, &admit)
                .err(),
            Some(AdaptivePatchError::Admission)
        );
        assert_eq!(used.load(Ordering::Relaxed), 0);
        limit.set(required_peak);
        calls.set(0);
        let mesh =
            tessellate_patch_mesh_adaptive_admitted(&patches, Affine::IDENTITY, limits(), &|| false, &admit)
                .unwrap();
        drop(mesh);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        limit.set(usize::MAX);
        let mesh = tessellate_tensor_patch_mesh_adaptive_admitted(
            &[left, right],
            Affine::IDENTITY,
            limits(),
            &|| false,
            &admit,
        )
        .unwrap();
        assert!(!mesh.triangles().is_empty());
        drop(mesh);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        eprintln!(
            "dynamic admitted mesh: reservations={reservations}, retained={retained}, required_peak={required_peak}, final=0"
        );
    }
    #[test]
    fn admitted_mesh_cancels_every_checkpoint_without_retaining_capacity() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut source = patch(0., [0., 0., 1., 0.]);
        source.control_points[1].x -= 0.1;
        let used = Arc::new(AtomicUsize::new(0));
        let admit = |bytes| {
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Credit(used.clone(), bytes)) as Box<dyn std::any::Any + Send + Sync>)
        };
        let polls = std::cell::Cell::new(0);
        tessellate_tensor_patch_mesh_adaptive_admitted(
            std::slice::from_ref(&source),
            Affine::IDENTITY,
            limits(),
            &|| {
                polls.set(polls.get() + 1);
                false
            },
            &admit,
        )
        .unwrap();
        let total = polls.get();
        assert_eq!(used.load(Ordering::Relaxed), 0);
        for target in 1..=total {
            polls.set(0);
            assert_eq!(
                tessellate_tensor_patch_mesh_adaptive_admitted(
                    std::slice::from_ref(&source),
                    Affine::IDENTITY,
                    limits(),
                    &|| {
                        polls.set(polls.get() + 1);
                        polls.get() >= target
                    },
                    &admit
                )
                .err(),
                Some(AdaptivePatchError::Cancelled)
            );
            assert_eq!(used.load(Ordering::Relaxed), 0, "checkpoint {target}");
        }
        let mut invalid = source;
        invalid.colors[0] = smallvec::smallvec![0.;5];
        assert_eq!(
            tessellate_tensor_patch_mesh_adaptive_admitted(
                &[invalid],
                Affine::IDENTITY,
                limits(),
                &|| false,
                &|_| panic!("invalid components must refuse before admission")
            )
            .err(),
            Some(AdaptivePatchError::Invalid)
        );
        eprintln!("admitted adaptive cancellation checkpoints={total}; final=0");
    }
    #[test]
    fn mesh_cancellation_and_triangle_limit_preserve_existing_output_and_ranges() {
        let source = patch(0., [0.; 4]);
        let patches = [AdaptivePatchRef::Tensor(&source)];
        let mut seed = Vec::new();
        source
            .to_triangles_adaptive(Affine::IDENTITY, limits(), &mut seed, || false)
            .unwrap();
        let original = seed[0].clone();
        let polls = std::cell::Cell::new(0);
        let mut output = vec![original.clone()];
        let mut ranges = vec![99..101];
        tessellate_patch_mesh_adaptive_with_ranges(
            &patches,
            Affine::IDENTITY,
            limits(),
            &mut output,
            &mut ranges,
            || {
                polls.set(polls.get() + 1);
                false
            },
        )
        .unwrap();
        let total = polls.get();
        for target in 1..=total {
            polls.set(0);
            let mut output = vec![original.clone()];
            let mut ranges = vec![99..101];
            assert_eq!(
                tessellate_patch_mesh_adaptive_with_ranges(
                    &patches,
                    Affine::IDENTITY,
                    limits(),
                    &mut output,
                    &mut ranges,
                    || {
                        polls.set(polls.get() + 1);
                        polls.get() >= target
                    }
                )
                .err(),
                Some(AdaptivePatchError::Cancelled)
            );
            assert_eq!(output.len(), 1);
            assert_eq!(ranges, vec![99..101]);
            assert_eq!(output[0].p0.point, original.p0.point);
        }
        let mut output = vec![original];
        let mut ranges = vec![99..101];
        let mut tight = limits();
        tight.max_triangles = 1;
        assert_eq!(
            tessellate_patch_mesh_adaptive_with_ranges(
                &patches,
                Affine::IDENTITY,
                tight,
                &mut output,
                &mut ranges,
                || false
            )
            .err(),
            Some(AdaptivePatchError::TriangleLimit)
        );
        assert_eq!(output.len(), 1);
        assert_eq!(ranges, vec![99..101]);
    }
}
