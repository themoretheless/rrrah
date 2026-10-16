//! Retained original device-color analytic and mesh shading. No display RGB table.
use crate::{
    color::ColorSpace,
    pattern::ShadingPattern,
    shading::{ShadingFunction, ShadingType},
};
use kurbo::{Affine, Point};

/// Explicit refusal during original-component shading capture or sampling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeShadingError {
    /// The caller cancelled the operation.
    Cancelled,
    /// A source type, color space or degenerate solve is not qualified.
    Unsupported,
    /// Nonfinite, malformed or out-of-range source data.
    Invalid,
    /// The retained source capacity was refused before cloning.
    Admission,
    /// The caller's bounded geometric query work was exhausted.
    WorkLimit,
}

/// Unconverted source components and source opacity; shape is applied by the caller.
pub struct NativeShadingSample {
    /// Retained shading components in the declared source color space.
    /// Tensor patches use tessellated triangle fields, not exact bicubic evaluation.
    pub components: [f32; 4],
    /// Number of active entries in `components`.
    pub count: usize,
    /// Pattern opacity before geometry and graphics-state mask coverage.
    pub opacity: f32,
}

/// Source-bound device rectangle with shared inverse-transform arithmetic.
/// Reuse avoids recomputing identical coordinate envelopes for every owner.
pub struct NativeAffineDomain<'a> {
    source:&'a NativeShadingSource,domain:kurbo::Rect,inputs:[AffineMeshWeight;2],
}
impl<'a> NativeAffineDomain<'a> {
    /// Prepare an immutable source/owner certificate with exactly full source
    /// opacity. Caller still proves geometric ownership, masks and group route.
    pub fn prepare_opaque_owner(&self,index:usize,cancelled:&dyn Fn()->bool,work:&mut usize)->Result<NativeOpaqueAffinePointEvidence<'a>,NativeShadingError> {
        if cancelled() {return Err(NativeShadingError::Cancelled);}
        *work=work.checked_sub(1).ok_or(NativeShadingError::WorkLimit)?;
        if self.source.opacity!=1. {return Err(NativeShadingError::Unsupported);}
        let evidence=self.prepare_owner(index,cancelled,work)?;
        if cancelled() {return Err(NativeShadingError::Cancelled);}
        Ok(NativeOpaqueAffinePointEvidence {evidence})
    }
    /// Prepare one owner using the same certified device-coordinate envelope.
    pub fn prepare_owner(&self,index:usize,cancelled:&dyn Fn()->bool,work:&mut usize)->Result<NativeAffinePointEvidence<'a>,NativeShadingError> {
        self.source.prepare_native_affine_point_evidence_in_domain(index,self.domain,Some(self.inputs),cancelled,work)
    }
}

/// Source-bound native affine owner with certified full source opacity.
/// Construction validates the source coordinate identity, unit vertex colors
/// and affine domain. Coverage, masks and group composition remain independent.
pub struct NativeOpaqueAffinePointEvidence<'a> {evidence:NativeAffinePointEvidence<'a>}
impl NativeOpaqueAffinePointEvidence<'_> {
    /// Original source color space retained by the certificate.
    pub fn source_color_space(&self)->&ColorSpace {self.evidence.source_color_space()}
    /// Original source owner index.
    pub fn owner_index(&self)->usize {self.evidence.owner_index()}
    /// Number of explicitly observed points.
    pub fn point_count(&self)->usize {self.evidence.point_count()}
    /// Bound on negative weight mass for observed points.
    pub fn max_projection_deficit_upper(&self)->f64 {self.evidence.max_projection_deficit_upper()}
    /// Geometry-only point observation; no source colors evaluated.
    pub fn observe(&mut self,point:Point,cancelled:&dyn Fn()->bool,work:&mut usize)->Result<(),NativeShadingError> {self.evidence.observe(point,cancelled,work)}
    /// Sample this same full-opacity owner and record its geometry evidence.
    pub fn sample_observed(&mut self,point:Point,cancelled:&dyn Fn()->bool,work:&mut usize)->Result<NativeShadingSample,NativeShadingError> {self.evidence.sample_observed(point,cancelled,work)}
    /// Retained original affine field gradient bounds.
    pub fn gradient_bounds(&self,cancelled:&dyn Fn()->bool,work:&mut usize)->Result<[[f64;2];4],NativeShadingError> {self.evidence.gradient_bounds(cancelled,work)}
    /// Discrepancy versus the retained affine triangle field for observed points.
    /// Excludes original tensor-patch tessellation and source decoding error.
    pub fn component_error_bound(&self,cancelled:&dyn Fn()->bool,work:&mut usize)->Result<[f64;4],NativeShadingError> {self.evidence.component_error_bound(cancelled,work)}
}

/// Prepared immutable source/owner binding plus evidence for successfully
/// observed points only. Unobserved/refused points are never certified.
pub struct NativeAffinePointEvidence<'a> {
    source:&'a NativeShadingSource,index:usize,domain:kurbo::Rect,
    evaluation:f64,max_deficit:f64,points:usize,
    weight_derivatives:[[MeshWeightInterval;2];3],
}
impl NativeAffinePointEvidence<'_> {
    /// Original retained source color space; no display conversion implied.
    pub fn source_color_space(&self)->&ColorSpace {&self.source.color_space}
    /// Selected original owner index, tied to the borrowed source.
    pub fn owner_index(&self)->usize {self.index}
    /// Number of successful observations.
    pub fn point_count(&self)->usize {self.points}
    /// Outward bound on negative computed weight mass of observed points.
    pub fn max_projection_deficit_upper(&self)->f64 {self.max_deficit}
    /// Sample this bound source/owner and record its geometry evidence only on
    /// success. Shares the caller's finite work allowance; no ownership search.
    pub fn sample_observed(&mut self,point:Point,cancelled:&dyn Fn()->bool,work:&mut usize)->Result<NativeShadingSample,NativeShadingError> {
        let sample=self.source.sample_mesh_triangle_with_budget(self.index,point,cancelled,work)?;
        self.observe(point,cancelled,work)?;Ok(sample)
    }
    /// Absolute component derivative bounds from the retained source/owner
    /// affine solve. This excludes projection, source rounding and integration.
    pub fn gradient_bounds(&self,cancelled:&dyn Fn()->bool,work:&mut usize)->Result<[[f64;2];4],NativeShadingError> {
        self.source.mesh_native_gradient_from_derivatives(self.index,self.weight_derivatives,cancelled,work)
    }
    /// Validate the device domain and accumulate selected-weight projection
    /// evidence, without sampling colors. Failure preserves prior evidence.
    pub fn observe(&mut self,point:Point,cancelled:&dyn Fn()->bool,remaining:&mut usize)->Result<(),NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        *remaining=remaining.checked_sub(1).ok_or(WorkLimit)?;
        if !point.x.is_finite() || !point.y.is_finite() || point.x<self.domain.x0 || point.x>self.domain.x1 || point.y<self.domain.y0 || point.y>self.domain.y1 {return Err(Invalid);}
        let deficit=self.source.mesh_triangle_projection_deficit(self.index,point,cancelled,remaining)?;
        // The diagnostic sum itself is rounded. Enclose its nonnegative
        // three-term sum before using it as a mathematical deficit bound.
        let upper=if deficit==0. {0.} else {(deficit+(4.*f64::EPSILON*deficit).next_up()+3.*f64::from_bits(1)).next_up()};
        let points=self.points.checked_add(1).ok_or(Invalid)?;
        if !upper.is_finite() {return Err(Invalid);}
        if cancelled() {return Err(Cancelled);}
        self.points=points;self.max_deficit=self.max_deficit.max(upper);Ok(())
    }
    /// Component discrepancy bound versus the exact affine field for all
    /// successful observations, including projection and source rounding.
    /// Excludes discrepancy from the original tensor patch; consult the separate
    /// non-certified patch_tessellation_stats diagnostics on the source.
    pub fn component_error_bound(&self,cancelled:&dyn Fn()->bool,remaining:&mut usize)->Result<[f64;4],NativeShadingError> {
        if self.points==0 {return Err(NativeShadingError::Unsupported);}
        let positive=((1.+self.evaluation).next_up()+self.max_deficit).next_up();
        self.source.mesh_native_component_bound_from_limits(self.index,self.evaluation,self.max_deficit,positive,cancelled,remaining)
    }
}

trait MeshTriangleInput {
    fn point(&self, vertex: usize) -> Point;
    fn components(&self, vertex: usize) -> &[f32];
}
impl MeshTriangleInput for crate::shading::Triangle {
    fn point(&self, vertex: usize) -> Point { [&self.p0,&self.p1,&self.p2][vertex].point }
    fn components(&self, vertex: usize) -> &[f32] { &[&self.p0,&self.p1,&self.p2][vertex].colors }
}
impl MeshTriangleInput for crate::shading::CompactAdaptiveTriangle {
    fn point(&self, vertex: usize) -> Point { self.points()[vertex] }
    fn components(&self, vertex: usize) -> &[f32] { &self.components()[vertex][..self.component_count()] }
}
/// Original source triangle geometry in device coordinates. The stable index
/// is its source order, not the spatial-index traversal order.
#[derive(Clone,Copy,Debug)]
pub struct NativeShadingTriangle {
    /// Stable original triangle index; larger indices cover earlier triangles.
    pub index:usize,
    /// Original triangle vertices transformed once to device coordinates.
    pub points:[Point;3],
}

/// Source metadata is retained only after its conservative capacity admission.
/// Repeated Arc references may be counted more than once, never deduplicated
/// into a smaller allowance. Parsed source metadata allocation precedes capture.
pub struct NativeShadingSource {
    /// Original source color space; resolve explicit alternate tints before blending.
    pub color_space: ColorSpace,
    function: Option<ShadingFunction>,
    mesh: Option<NativeMesh>,
    patch_tessellation: Option<crate::shading::AdaptivePatchStats>,
    inverse: Affine,
    forward: Affine,
    coords: [f64; 6],
    domain: [f32; 2],
    extend: [bool; 2],
    axial: bool,
    opacity: f32,
    background: Option<[f32; 4]>,
    count: usize,
    // Retained references drop before their admission owner.
    _credit: Box<dyn std::any::Any + Send + Sync>,
}
struct NativeTriangle {
    points: [Point; 3],
    colors: [[f32; 4]; 3],
}
// Conservative separating-axis query. Touching boundaries are retained;
// only strict separation rejects a triangle. No heap scratch or tolerance.
fn triangle_intersects_rect(points: [Point;3], rect: kurbo::Rect) -> Result<bool,NativeShadingError> {
    let corners = [Point::new(rect.x0,rect.y0),Point::new(rect.x1,rect.y0),
        Point::new(rect.x1,rect.y1),Point::new(rect.x0,rect.y1)];
    let origin = points[0];
    for axis in [kurbo::Vec2::new(1.,0.),kurbo::Vec2::new(0.,1.)].into_iter()
        .chain((0..3).map(|i| {let edge=points[(i+1)%3]-points[i];kurbo::Vec2::new(-edge.y,edge.x)})) {
        let project = |p:Point| {let d=p-origin;d.x*axis.x+d.y*axis.y};
        let triangle = points.map(project);
        let rectangle = corners.map(project);
        if triangle.iter().chain(rectangle.iter()).any(|v|!v.is_finite()) {
            return Err(NativeShadingError::Invalid);
        }
        let min = |values:&[f64]|values.iter().copied().fold(f64::INFINITY,f64::min);
        let max = |values:&[f64]|values.iter().copied().fold(f64::NEG_INFINITY,f64::max);
        if max(&triangle)<min(&rectangle) || max(&rectangle)<min(&triangle) {return Ok(false);}
    }
    Ok(true)
}
const LEAF_TRIANGLES: usize = 8;
struct MeshNode {
    bounds: kurbo::Rect,
    // capture_mesh limits geometry to1,048,576 triangles; node/range indices fitu32.
    range: std::ops::Range<u32>,
    children: Option<[u32; 2]>,
    latest: u32,
}
struct NativeMesh {
    triangles: Vec<NativeTriangle>,
    order: Vec<usize>,
    nodes: Vec<MeshNode>,
}
#[inline]
fn interpolate_mesh_components(colors:&[[f32;4];3],weights:[f64;3],count:usize)->[f32;4] {
    if count==4 {
        // Fixed independent lanes permit vectorization without reassociation,
        // fused arithmetic, lower precision, or a different work debit.
        let mut sums=[-0f64;4];
        for vertex in 0..3 {for channel in 0..4 {
            sums[channel]+=weights[vertex]*f64::from(colors[vertex][channel]);
        }}
        return sums.map(|value|value as f32);
    }
    let mut components=[0.;4];
    for (channel,out) in components.iter_mut().enumerate().take(count) {
        *out=(0..3).map(|i|weights[i]*f64::from(colors[i][channel])).sum::<f64>() as f32;
    }
    components
}
// Outward intervals enclose the rounded result of each ordinary f64
// operation. Refuse infinities rather than publishing an unbounded certificate.
#[derive(Clone,Copy)]
struct MeshWeightInterval {lo:f64,hi:f64}
impl MeshWeightInterval {
    fn new(lo:f64,hi:f64)->Option<Self> {(lo.is_finite() && hi.is_finite() && lo<=hi).then_some(Self {lo,hi})}
    fn exact(v:f64)->Option<Self> {Self::new(v,v)}
    fn op(self,other:Self,kind:u8,work:&mut usize,cancelled:&dyn Fn()->bool)->Option<Self> {
        if cancelled() {return None;}*work=work.checked_sub(1)?;
        let (lo,hi)=match kind {
            0=>(self.lo+other.lo,self.hi+other.hi),
            1=>(self.lo-other.hi,self.hi-other.lo),
            _=>{
                if kind==3 && other.lo<=0. && other.hi>=0. {return None;}
                let values=if kind==2 {[self.lo*other.lo,self.lo*other.hi,self.hi*other.lo,self.hi*other.hi]}
                    else {[self.lo/other.lo,self.lo/other.hi,self.hi/other.lo,self.hi/other.hi]};
                if values.iter().any(|v|!v.is_finite()) {return None;}
                (values.iter().copied().fold(f64::INFINITY,f64::min),values.iter().copied().fold(f64::NEG_INFINITY,f64::max))
            }
        };
        Self::new(lo.next_down(),hi.next_up())
    }
}

// The coefficients enclose the exact linear expression; error bounds the
// difference introduced by evaluating that expression in the sampler's order.
#[derive(Clone,Copy)]
struct AffineMeshWeight {coeff:[MeshWeightInterval;3],range:MeshWeightInterval,error:f64}
impl AffineMeshWeight {
    fn input(axis:usize,range:MeshWeightInterval)->Self {
        let mut coeff=[MeshWeightInterval {lo:0.,hi:0.};3];coeff[axis]=MeshWeightInterval {lo:1.,hi:1.};
        Self {coeff,range,error:0.}
    }
    fn constant(value:f64)->Option<Self> {
        let mut coeff=[MeshWeightInterval::exact(0.)?;3];coeff[2]=MeshWeightInterval::exact(value)?;
        Some(Self {coeff,range:MeshWeightInterval::exact(value)?,error:0.})
    }
    fn op(self,other:Self,kind:u8,work:&mut usize,cancelled:&dyn Fn()->bool)->Option<Self> {
        // Multiplication/division here only permit an exact scalar RHS.
        if kind>=2 && (other.error!=0. || other.range.lo!=other.range.hi
            || other.coeff[0].lo!=0. || other.coeff[0].hi!=0.
            || other.coeff[1].lo!=0. || other.coeff[1].hi!=0.) {return None;}
        let range=self.range.op(other.range,kind,work,cancelled)?;
        let mut coeff=self.coeff;
        for (i,value) in coeff.iter_mut().enumerate() {
            *value=self.coeff[i].op(if kind<2 {other.coeff[i]} else {other.range},kind,work,cancelled)?;
        }
        let magnitude=range.lo.abs().max(range.hi.abs());
        let rounding=(magnitude.next_up()-magnitude).next_up();
        let propagated=match kind {
            0|1=>(self.error+other.error).next_up(),
            2=>(self.error*other.range.lo.abs()).next_up(),
            _=>(self.error/other.range.lo.abs()).next_up(),
        };
        let error=(propagated+rounding).next_up();
        error.is_finite().then_some(Self {coeff,range,error})
    }
    fn over_vertices(self,vertices:&[Point],work:&mut usize,cancelled:&dyn Fn()->bool)->Option<[f64;2]> {
        let mut lo=f64::INFINITY;let mut hi=f64::NEG_INFINITY;
        for point in vertices {
            let x=self.coeff[0].op(MeshWeightInterval::exact(point.x)?,2,work,cancelled)?;
            let y=self.coeff[1].op(MeshWeightInterval::exact(point.y)?,2,work,cancelled)?;
            let v=x.op(y,0,work,cancelled)?.op(self.coeff[2],0,work,cancelled)?;
            lo=lo.min(v.lo);hi=hi.max(v.hi);
        }
        let lo=(lo-self.error).next_down();let hi=(hi+self.error).next_up();
        MeshWeightInterval::new(lo,hi).map(|v|[v.lo,v.hi])
    }
}

impl NativeTriangle {
    fn bounds(&self) -> kurbo::Rect {
        let mut bounds = kurbo::Rect::from_points(self.points[0], self.points[1]);
        bounds = bounds.union_pt(self.points[2]);
        bounds
    }
    fn weights(&self, point: Point) -> Result<Option<[f64; 3]>, NativeShadingError> {
        let [a, b, c] = self.points;
        let cross = |x: kurbo::Vec2, y: kurbo::Vec2| x.x * y.y - x.y * y.x;
        let denominator = cross(b - a, c - a);
        if !denominator.is_finite() {
            return Err(NativeShadingError::Invalid);
        }
        if denominator == 0.0 {
            return Ok(None);
        }
        let weights = [
            cross(b - point, c - point) / denominator,
            cross(c - point, a - point) / denominator,
            cross(a - point, b - point) / denominator,
        ];
        if weights.iter().any(|v| !v.is_finite()) {
            return Err(NativeShadingError::Invalid);
        }
        Ok(if weights.iter().any(|v| *v < 0.0) {
            None
        } else {
            Some(weights)
        })
    }
}
impl NativeMesh {
    fn build_nodes(
        triangles: &[NativeTriangle],
        order: &mut [usize],
        offset: usize,
        nodes: &mut Vec<MeshNode>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {
            return Err(Cancelled);
        }
        let mut bounds = triangles[order[0]].bounds();
        let mut latest = order[0];
        for (i, &index) in order.iter().enumerate() {
            if i % 256 == 0 && cancelled() {
                return Err(Cancelled);
            }
            bounds = bounds.union(triangles[index].bounds());
            latest = latest.max(index);
        }
        let node = nodes.len();
        nodes.push(MeshNode {
            bounds,
            range: u32::try_from(offset).map_err(|_| Invalid)?..u32::try_from(offset + order.len()).map_err(|_| Invalid)?,
            children: None,
            latest: u32::try_from(latest).map_err(|_| Invalid)?,
        });
        if order.len() > LEAF_TRIANGLES {
            let x_axis = bounds.width() >= bounds.height();
            // Only the two median partitions are needed; fully sorting each
            // subtree repeats unnecessary O(n log n) work at every level.
            let middle = order.len() / 2;
            order.select_nth_unstable_by(middle,|&a, &b| {
                let center = |index: usize| {
                    let r = triangles[index].bounds();
                    if x_axis {
                        r.x0 * 0.5 + r.x1 * 0.5
                    } else {
                        r.y0 * 0.5 + r.y1 * 0.5
                    }
                };
                center(a).total_cmp(&center(b)).then(a.cmp(&b))
            });
            if cancelled() {
                return Err(Cancelled);
            }
            let (left, right) = order.split_at_mut(middle);
            let a = Self::build_nodes(triangles, left, offset, nodes, cancelled)?;
            let b = Self::build_nodes(triangles, right, offset + middle, nodes, cancelled)?;
            nodes[node].children = Some([u32::try_from(a).map_err(|_| Invalid)?, u32::try_from(b).map_err(|_| Invalid)?]);
        }
        Ok(node)
    }
    fn candidate(
        &self,
        point: Point,
        cancelled: &dyn Fn() -> bool,
        remaining: &mut usize,
    ) -> Result<Option<(usize, [f64; 3])>, NativeShadingError> {
        use NativeShadingError::*;
        if self.nodes.is_empty() {
            return Ok(None);
        }
        // Balanced median splitting of at most 2^20 triangles needs < 32 slots.
        let mut stack = [0usize; 32];
        let mut length = 1;
        let mut selected: Option<(usize, [f64; 3])> = None;
        while length > 0 {
            if cancelled() {
                return Err(Cancelled);
            }
            *remaining=remaining.checked_sub(1).ok_or(WorkLimit)?;
            length -= 1;
            let node = &self.nodes[stack[length]];
            let r = node.bounds;
            if point.x < r.x0
                || point.x > r.x1
                || point.y < r.y0
                || point.y > r.y1
                || selected.as_ref().is_some_and(|(index, _)| node.latest as usize <= *index)
            {
                continue;
            }
            if let Some([a, b]) = node.children {
                let [a,b] = [a as usize,b as usize];
                // Visit the child with later source geometry first for pruning.
                let children = if self.nodes[a].latest > self.nodes[b].latest {
                    [b, a]
                } else {
                    [a, b]
                };
                if length + 2 > stack.len() {
                    return Err(Invalid);
                }
                stack[length..length + 2].copy_from_slice(&children);
                length += 2;
            } else {
                for &index in &self.order[node.range.start as usize..node.range.end as usize] {
                    *remaining=remaining.checked_sub(1).ok_or(WorkLimit)?;
                    if cancelled() {
                        return Err(Cancelled);
                    }
                    if selected.as_ref().is_some_and(|(chosen, _)| index <= *chosen) {
                        continue;
                    }
                    if let Some(weights) = self.triangles[index].weights(point)? {
                        selected = Some((index, weights));
                    }
                }
            }
        }
        Ok(selected)
    }
}
impl NativeShadingSource {
    /// Retained tensor tessellation diagnostics, absent for original triangles
    /// and axial/radial sources. These floating safety-margin bounds are not an
    /// interval certificate and do not cover clipping or nonlinear conversion.
    pub fn patch_tessellation_stats(&self)->Option<crate::shading::AdaptivePatchStats> {
        self.patch_tessellation
    }

    /// Retain supported source geometry/color functions after capacity admission.
    /// Device and explicitly transformed Separation/DeviceN axial/radial, triangle
    /// and bounded adaptive tensor meshes are supported. Coons, profiles and
    /// transfers are refused. Source functions have bounded native evaluators.
    /// Tensor bounds apply to source components, not subsequent nonlinear transforms.
    pub fn capture(
        pattern: &ShadingPattern,
        cancelled: &dyn Fn() -> bool,
        admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
    ) -> Result<Self, NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {
            return Err(Cancelled);
        }
        if pattern.transfer_function.is_some() {
            return Err(Unsupported);
        }
        let color_bytes = pattern
            .shading
            .color_space
            .native_sample_retained_capacity(cancelled)
            .ok_or_else(|| if cancelled() { Cancelled } else { Unsupported })?;
        let count = usize::from(pattern.shading.color_space.num_components());
        if let ShadingType::TensorProductPatchMesh { patches, function } =
            pattern.shading.shading_type.as_ref()
        {
            if !pattern.opacity.is_finite()
                || !(0.0..=1.0).contains(&pattern.opacity)
                || pattern.matrix.as_coeffs().iter().any(|v| !v.is_finite())
                || pattern.matrix.determinant() == 0.
            {
                return Err(Invalid);
            }
            use crate::shading::{
                AdaptivePatchError, AdaptivePatchLimits, tessellate_tensor_patch_mesh_compact_admitted,
            };
            let mesh = tessellate_tensor_patch_mesh_compact_admitted(
                patches,
                pattern.matrix,
                AdaptivePatchLimits {
                    pixel_error: 1. / 64.,
                    component_error: 1. / 1024.,
                    max_depth: 12,
                    max_triangles: 1_048_576,
                },
                cancelled,
                admit,
            )
            .map_err(|error| {
                if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
                    eprintln!(
                        "NATIVE_TENSOR_CAPTURE patches={} refusal={error:?}",
                        patches.len()
                    );
                }
                match error {
                    AdaptivePatchError::Cancelled => Cancelled,
                    AdaptivePatchError::Admission => Admission,
                    AdaptivePatchError::Invalid => Invalid,
                    AdaptivePatchError::Depth | AdaptivePatchError::TriangleLimit => Unsupported,
                }
            })?;
            if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {
                eprintln!(
                    "NATIVE_TENSOR_CAPTURE patches={} triangles={} pixel_bound={} component_bound={}",
                    patches.len(),
                    mesh.stats().triangles,
                    mesh.stats().max_pixel_bound,
                    mesh.stats().max_component_bound
                );
            }
            // Adaptive vertices already live in device coordinates. Applying the
            // original inverse again would incorrectly transform them twice.
            let mut device_pattern = pattern.clone();
            device_pattern.matrix = Affine::IDENTITY;
            let mut source=Self::capture_mesh(
                &device_pattern,
                mesh.triangles(),
                function.as_ref(),
                count,
                color_bytes,
                cancelled,
                admit,
            )?;
            source.patch_tessellation=Some(mesh.stats());
            return Ok(source);
        }
        if let ShadingType::TriangleMesh { triangles, function } = pattern.shading.shading_type.as_ref() {
            return Self::capture_mesh(
                pattern,
                triangles,
                function.as_ref(),
                count,
                color_bytes,
                cancelled,
                admit,
            );
        }
        let ShadingType::RadialAxial {
            coords,
            domain,
            function,
            extend,
            axial,
        } = pattern.shading.shading_type.as_ref()
        else {
            return Err(Unsupported);
        };
        if !(1..=4).contains(&count)
            || !pattern.opacity.is_finite()
            || !(0.0..=1.0).contains(&pattern.opacity)
            || coords.iter().any(|v| !v.is_finite())
            || domain.iter().any(|v| !v.is_finite())
            || domain[0] >= domain[1]
            || pattern.matrix.as_coeffs().iter().any(|v| !v.is_finite())
            || pattern.matrix.determinant() == 0.0
        {
            return Err(Invalid);
        }
        if *axial {
            if coords[0] == coords[2] && coords[1] == coords[3] {
                return Err(Invalid);
            }
        } else if coords[2] < 0.0 || coords[5] < 0.0 || coords[0..3] == coords[3..] {
            return Err(Invalid);
        }
        let inverse = pattern.matrix.inverse();
        if inverse.as_coeffs().iter().any(|v| !v.is_finite()) {
            return Err(Invalid);
        }
        let background = if pattern.paint_background {
            if let Some(bg) = &pattern.shading.background {
                if bg.len() != count || bg.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) {
                    return Err(Invalid);
                }
                let mut values = [0.; 4];
                values[0..count].copy_from_slice(bg);
                Some(values)
            } else {
                None
            }
        } else {
            None
        };
        let capacity = function
            .native_retained_capacity(cancelled)
            .ok_or_else(|| if cancelled() { Cancelled } else { Unsupported })?;
        let bytes = size_of::<Self>()
            .checked_add(color_bytes)
            .and_then(|v| v.checked_add(capacity))
            .ok_or(Invalid)?;
        let credit = admit(bytes).ok_or(Admission)?;
        if cancelled() {
            return Err(Cancelled);
        }
        Ok(Self {
            color_space: pattern.shading.color_space.clone(),
            function: Some(function.clone()),
            mesh: None,
            patch_tessellation: None,
            inverse,
            forward:pattern.matrix,
            coords: coords.map(f64::from),
            domain: *domain,
            extend: *extend,
            axial: *axial,
            opacity: pattern.opacity,
            background,
            count,
            _credit: credit,
        })
    }
    fn capture_mesh<T: MeshTriangleInput>(
        pattern: &ShadingPattern,
        triangles: &[T],
        function: Option<&ShadingFunction>,
        count: usize,
        color_bytes: usize,
        cancelled: &dyn Fn() -> bool,
        admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
    ) -> Result<Self, NativeShadingError> {
        use NativeShadingError::*;
        if !(1..=4).contains(&count)
            || triangles.len() > 1_048_576
            || !pattern.opacity.is_finite()
            || !(0.0..=1.0).contains(&pattern.opacity)
            || pattern.matrix.as_coeffs().iter().any(|v| !v.is_finite())
            || pattern.matrix.determinant() == 0.0
        {
            return Err(Invalid);
        }
        let inverse = pattern.matrix.inverse();
        if inverse.as_coeffs().iter().any(|v| !v.is_finite()) {
            return Err(Invalid);
        }
        let input_count = if function.is_some() { 1 } else { count };
        for triangle in triangles {
            if cancelled() {
                return Err(Cancelled);
            }
            for vertex in 0..3 {
                let point = triangle.point(vertex);
                let components = triangle.components(vertex);
                if !point.x.is_finite()
                    || !point.y.is_finite()
                    || components.len() != input_count
                    || components.iter()
                        .any(|v| !v.is_finite() || (function.is_none() && !(0.0..=1.0).contains(v)))
                {
                    return Err(Invalid);
                }
            }
        }
        let function_bytes = match function {
            Some(f) => f
                .native_retained_capacity(cancelled)
                .ok_or_else(|| if cancelled() { Cancelled } else { Unsupported })?,
            None => 0,
        };
        let node_capacity = if triangles.is_empty() {
            0
        } else {
            triangles
                .len()
                .div_ceil(LEAF_TRIANGLES)
                .checked_next_power_of_two()
                .and_then(|leaves| leaves.checked_mul(2))
                .and_then(|nodes| nodes.checked_sub(1))
                .ok_or(Invalid)?
        };
        let bytes = triangles
            .len()
            .checked_mul(size_of::<NativeTriangle>() + size_of::<usize>())
            .and_then(|v| {
                node_capacity
                    .checked_mul(size_of::<MeshNode>())
                    .and_then(|n| v.checked_add(n))
            })
            .and_then(|v| v.checked_add(size_of::<Self>()))
            .and_then(|v| v.checked_add(color_bytes))
            .and_then(|v| v.checked_add(function_bytes))
            .ok_or(Invalid)?;
        let credit = admit(bytes).ok_or(Admission)?;
        let mut mesh = Vec::new();
        mesh.try_reserve_exact(triangles.len()).map_err(|_| Admission)?;
        // Allocators may expose a larger capacity than requested. Never publish
        // retained capacity beyond the admitted allowance.
        if mesh.capacity() > triangles.len() {
            return Err(Admission);
        }
        for triangle in triangles {
            if cancelled() {
                return Err(Cancelled);
            }
            let mut colors = [[0.; 4]; 3];
            for (vertex, dst) in colors.iter_mut().enumerate() {
                dst[..input_count].copy_from_slice(triangle.components(vertex));
            }
            mesh.push(NativeTriangle {
                points: std::array::from_fn(|vertex| triangle.point(vertex)),
                colors,
            });
        }
        let mut order = Vec::new();
        order.try_reserve_exact(triangles.len()).map_err(|_| Admission)?;
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(node_capacity).map_err(|_| Admission)?;
        if order.capacity() > triangles.len() || nodes.capacity() > node_capacity {
            return Err(Admission);
        }
        order.extend(0..triangles.len());
        if !order.is_empty() {
            NativeMesh::build_nodes(&mesh, &mut order, 0, &mut nodes, cancelled)?;
        }
        let mesh = NativeMesh {
            triangles: mesh,
            order,
            nodes,
        };
        let background = if pattern.paint_background {
            match &pattern.shading.background {
                Some(bg) => {
                    if bg.len() != count || bg.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) {
                        return Err(Invalid);
                    }
                    let mut values = [0.; 4];
                    values[0..count].copy_from_slice(bg);
                    Some(values)
                }
                None => None,
            }
        } else {
            None
        };
        if cancelled() {
            return Err(Cancelled);
        }
        Ok(Self {
            color_space: pattern.shading.color_space.clone(),
            function: function.cloned(),
            mesh: Some(mesh),
            patch_tessellation: None,
            inverse,
            forward:pattern.matrix,
            coords: [0.; 6],
            domain: [0., 1.],
            extend: [false; 2],
            axial: false,
            opacity: pattern.opacity,
            background,
            count,
            _credit: credit,
        })
    }
    /// Whether each retained triangle has an affine component field in the
    /// destination's native coordinates, before f32 rounding. This is only a
    /// source capability: callers must still prove clipped ownership, constant
    /// opacity, and an affine group/blend lifecycle before merging integrals.
    /// Axial/radial functions, tint functions and color transforms refuse.
    pub fn mesh_has_affine_native_field(&self,destination:&ColorSpace)->bool {
        use crate::color::BlendingModel;
        let Some(model)=self.color_space.blending_model() else {return false;};
        let channels=match model {BlendingModel::Gray=>1,BlendingModel::Rgb=>3,BlendingModel::Cmyk=>4};
        self.mesh.is_some() && self.function.is_none() && self.count==channels
            && self.opacity.is_finite() && (0. ..=1.).contains(&self.opacity)
            && self.color_space.shares_blending_coordinates(destination)
    }

    /// Native affine mesh samples have fixed full opacity. Outside-domain samples
    /// may still be absent; callers must check sampled coverage and group masks.
    pub fn mesh_has_opaque_native_field(&self,destination:&ColorSpace)->bool {
        self.opacity==1. && self.mesh_has_affine_native_field(destination)
    }

    /// Conservative component intervals for successful selected-triangle samples
    /// in identical native destination coordinates, plus constant source opacity.
    /// This certifies a range, not affine integration or geometric ownership.
    /// Functions, color conversion and invalid vertex components refuse. Inactive
    /// channels are zero. No persistent storage or color evaluation is performed.
    pub fn mesh_native_component_bounds(&self,index:usize,destination:&ColorSpace,
        cancelled:&dyn Fn()->bool,remaining:&mut usize,
    )->Result<([[f64;2];4],f32),NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        *remaining=remaining.checked_sub(1).ok_or(WorkLimit)?;
        if !self.mesh_has_affine_native_field(destination) {return Err(Unsupported);}
        let triangle=self.mesh.as_ref().ok_or(Unsupported)?.triangles.get(index).ok_or(Invalid)?;
        let mut bounds=[[0.;2];4];
        for (channel,bound) in bounds.iter_mut().enumerate().take(self.count) {
            if cancelled() {return Err(Cancelled);}
            *remaining=remaining.checked_sub(3).ok_or(WorkLimit)?;
            let values=triangle.colors.map(|color|color[channel]);
            if values.iter().any(|v|!v.is_finite() || !(0. ..=1.).contains(v)) {return Err(Invalid);}
            let low=values.iter().copied().fold(f32::INFINITY,f32::min);
            let high=values.iter().copied().fold(f32::NEG_INFINITY,f32::max);
            // Selected sampling projects to nonnegative weights and normalizes
            // them in f64. Three divisions/products/additions perturb extrema
            // by less than16*f64::EPSILON relatively, far below one f32 ulp.
            // One outward f32 step includes normalization and final rounding;
            // clamp only to the range required for a successful native sample.
            *bound=[f64::from(low.next_down().max(0.)),f64::from(high.next_up().min(1.))];
        }
        if cancelled() {return Err(Cancelled);}
        Ok((bounds,self.opacity))
    }

    /// Device-space lines at the two axial parameter boundaries (t=0 and t=1).
    /// Points define infinite lines, not finite coverage segments. Under shear
    /// they are transformed source normals, not device-space axis normals.
    /// None means non-axial or non-finite transformed lines. No allocation or color conversion.
    pub fn axial_boundary_lines(&self)->Option<[[Point;2];2]> {
        if self.mesh.is_some() || !self.axial {return None;}
        let start=Point::new(self.coords[0],self.coords[1]);
        let end=Point::new(self.coords[2],self.coords[3]);
        let delta=end-start;let normal=kurbo::Vec2::new(-delta.y,delta.x);
        let lines=[[self.forward*start,self.forward*(start+normal)],
            [self.forward*end,self.forward*(end+normal)]];
        if lines.iter().flatten().any(|p|!p.x.is_finite() || !p.y.is_finite()) {return None;}
        Some(lines)
    }

    /// Evaluate the component field of a caller-selected mesh triangle.
    /// The caller must clip its returned device-space geometry first. This
    /// deliberately performs no second point-in-triangle search: rounding a
    /// clipped fragment centroid through the inverse transform can put it just
    /// outside an edge. Stable vertex differences preserve constant fields.
    /// Barycentric coefficients are projected to the convex simplex after
    /// coordinate rounding. This API requires an already clipped owner and is
    /// not a general extrapolating point sampler. No color conversion occurs.
    pub fn sample_mesh_triangle(
        &self, index: usize, point: Point, cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeShadingSample, NativeShadingError> {
        let mut remaining=usize::MAX;
        self.sample_mesh_triangle_impl(index,point,cancelled,&mut remaining,false)
    }
    /// Evaluate an already clipped mesh owner while debiting caller source work.
    /// This does not search for ownership or permit arbitrary extrapolation.
    pub fn sample_mesh_triangle_with_budget(&self,index:usize,point:Point,cancelled:&dyn Fn()->bool,
        remaining:&mut usize)->Result<NativeShadingSample,NativeShadingError> {
        self.sample_mesh_triangle_impl(index,point,cancelled,remaining,true)
    }
    fn sample_mesh_triangle_impl(&self,index:usize,point:Point,cancelled:&dyn Fn()->bool,
        remaining:&mut usize,shared:bool)->Result<NativeShadingSample,NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() { return Err(Cancelled); }
        *remaining=remaining.checked_sub(1).ok_or(WorkLimit)?;
        let triangle = self.mesh.as_ref().ok_or(Unsupported)?.triangles.get(index).ok_or(Invalid)?;
        let (_,weights)=self.mesh_triangle_weights(index,point)?;
        let input_count = if self.function.is_some() {1} else {self.count};
        *remaining=remaining.checked_sub(input_count).ok_or(WorkLimit)?;
        let mut components=interpolate_mesh_components(&triangle.colors,weights,input_count);
        if let Some(function) = &self.function {
            let input = components[0];
            if (if shared {function.eval_native_with_budget(input,&mut components[..self.count],cancelled,remaining)}
                else {function.eval_native_bounded(input,&mut components[..self.count],cancelled)}).is_none() {
                return Err(if cancelled() {Cancelled} else {Unsupported});
            }
        }
        if cancelled() { return Err(Cancelled); }
        if components[..self.count].iter().any(|v|!v.is_finite() || !(0. ..=1.).contains(v)) { return Err(Invalid); }
        Ok(NativeShadingSample {components,count:self.count,opacity:self.opacity})
    }

    /// Outward intervals for all three computed, unprojected barycentric weights
    /// over a device rectangle. Includes inverse-transform and weight-operation
    /// rounding. This is a domain diagnostic, not an affine integration proof.
    /// Degeneracy, overflow, work exhaustion and cancellation refuse safely.
    pub fn mesh_triangle_weight_bounds(&self,index:usize,rect:kurbo::Rect,
        cancelled:&dyn Fn()->bool,remaining:&mut usize)->Result<[[f64;2];3],NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        *remaining=remaining.checked_sub(1).ok_or(WorkLimit)?;
        let triangle=self.mesh.as_ref().ok_or(Unsupported)?.triangles.get(index).ok_or(Invalid)?;
        let result=(|| {
            let x=MeshWeightInterval::new(rect.x0,rect.x1)?;let y=MeshWeightInterval::new(rect.y0,rect.y1)?;
            let exact=MeshWeightInterval::exact;
            let mut op=|a:MeshWeightInterval,b:MeshWeightInterval,k|a.op(b,k,remaining,cancelled);
            let [xx,yx,xy,yy,tx,ty]=self.inverse.as_coeffs();
            let px0=op(exact(xx)?,x,2)?;let px1=op(exact(xy)?,y,2)?;
            let px=op(px0,px1,0)?;let px=op(px,exact(tx)?,0)?;
            let py0=op(exact(yx)?,x,2)?;let py1=op(exact(yy)?,y,2)?;
            let py=op(py0,py1,0)?;let py=op(py,exact(ty)?,0)?;
            let [a,b,c]=triangle.points;let b=b-a;let c=c-a;
            let denominator=b.x*c.y-b.y*c.x;
            if !denominator.is_finite() || denominator==0. {return None;}
            let dx=op(px,exact(a.x)?,1)?;let dy=op(py,exact(a.y)?,1)?;
            let u=op(dx,exact(c.y)?,2)?;let v=op(dy,exact(c.x)?,2)?;
            let wb=op(u,v,1)?;let wb=op(wb,exact(denominator)?,3)?;
            let u=op(exact(b.x)?,dy,2)?;let v=op(exact(b.y)?,dx,2)?;
            let wc=op(u,v,1)?;let wc=op(wc,exact(denominator)?,3)?;
            let wa=op(exact(1.)?,wb,1)?;let wa=op(wa,wc,1)?;
            Some([wa,wb,wc].map(|v|[v.lo,v.hi]))
        })();
        if cancelled() {return Err(Cancelled);}
        result.ok_or(if *remaining==0 {WorkLimit} else {Invalid})
    }

    /// Bounds computed raw weights over the convex hull of 1..=64 finite
    /// device vertices, preserving linear coordinate correlation. A separate
    /// rounding envelope covers evaluation over the hull's bounding rectangle.
    /// This does not certify that caller fragment centroids lie in that hull.
    pub fn mesh_triangle_weight_bounds_over_vertices(&self,index:usize,vertices:&[Point],
        cancelled:&dyn Fn()->bool,remaining:&mut usize)->Result<[[f64;2];3],NativeShadingError> {
        let fields=self.mesh_triangle_affine_weight_fields(index,vertices,cancelled,remaining)?;
        let mut bounds=[[0.;2];3];
        for (out,field) in bounds.iter_mut().zip(fields) {
            *out=field.over_vertices(vertices,remaining,cancelled).ok_or_else(||if cancelled() {NativeShadingError::Cancelled} else if *remaining==0 {NativeShadingError::WorkLimit} else {NativeShadingError::Invalid})?;
        }
        if cancelled() {return Err(NativeShadingError::Cancelled);}
        Ok(bounds)
    }
    fn native_affine_device_inputs(&self,vertices:&[Point],cancelled:&dyn Fn()->bool,remaining:&mut usize)->Option<[AffineMeshWeight;2]> {
            let mut bbox=[f64::INFINITY,f64::NEG_INFINITY,f64::INFINITY,f64::NEG_INFINITY];
            for p in vertices {
                if cancelled() {return None;}*remaining=remaining.checked_sub(1)?;
                if !p.x.is_finite() || !p.y.is_finite() {return None;}
                bbox[0]=bbox[0].min(p.x);bbox[1]=bbox[1].max(p.x);bbox[2]=bbox[2].min(p.y);bbox[3]=bbox[3].max(p.y);
            }
            let x=AffineMeshWeight::input(0,MeshWeightInterval::new(bbox[0],bbox[1])?);
            let y=AffineMeshWeight::input(1,MeshWeightInterval::new(bbox[2],bbox[3])?);
            let constant=AffineMeshWeight::constant;
            let mut op=|a:AffineMeshWeight,b:AffineMeshWeight,k|a.op(b,k,remaining,cancelled);
            let [xx,yx,xy,yy,tx,ty]=self.inverse.as_coeffs();
            let px0=op(x,constant(xx)?,2)?;let px1=op(y,constant(xy)?,2)?;
            let px=op(px0,px1,0)?;let px=op(px,constant(tx)?,0)?;
            let py0=op(x,constant(yx)?,2)?;let py1=op(y,constant(yy)?,2)?;
            let py=op(py0,py1,0)?;let py=op(py,constant(ty)?,0)?;
            Some([px,py])
    }

    fn mesh_triangle_affine_weight_fields(&self,index:usize,vertices:&[Point],
        cancelled:&dyn Fn()->bool,remaining:&mut usize)->Result<[AffineMeshWeight;3],NativeShadingError> {
        self.mesh_triangle_affine_weight_fields_with_inputs(index,vertices,None,cancelled,remaining)
    }
    fn mesh_triangle_affine_weight_fields_with_inputs(&self,index:usize,vertices:&[Point],inputs:Option<[AffineMeshWeight;2]>,
        cancelled:&dyn Fn()->bool,remaining:&mut usize)->Result<[AffineMeshWeight;3],NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        *remaining=remaining.checked_sub(1).ok_or(WorkLimit)?;
        if vertices.is_empty() || vertices.len()>64 {return Err(Invalid);}
        let triangle=self.mesh.as_ref().ok_or(Unsupported)?.triangles.get(index).ok_or(Invalid)?;
        let result=(|| {
            let [px,py]=if let Some(inputs)=inputs {inputs} else {self.native_affine_device_inputs(vertices,cancelled,remaining)?};
            let constant=AffineMeshWeight::constant;
            let mut op=|a:AffineMeshWeight,b:AffineMeshWeight,k|a.op(b,k,remaining,cancelled);
            let [a,b,c]=triangle.points;let b=b-a;let c=c-a;
            let denominator=b.x*c.y-b.y*c.x;
            if !denominator.is_finite() || denominator==0. {return None;}
            let dx=op(px,constant(a.x)?,1)?;let dy=op(py,constant(a.y)?,1)?;
            let u=op(dx,constant(c.y)?,2)?;let v=op(dy,constant(c.x)?,2)?;
            let wb=op(u,v,1)?;let wb=op(wb,constant(denominator)?,3)?;
            let u=op(dy,constant(b.x)?,2)?;let v=op(dx,constant(b.y)?,2)?;
            let wc=op(u,v,1)?;let wc=op(wc,constant(denominator)?,3)?;
            let wa=op(constant(1.)?,wb,1)?;let wa=op(wa,wc,1)?;
            Some([wa,wb,wc])
        })();
        if cancelled() {return Err(Cancelled);}
        result.ok_or(if *remaining==0 {WorkLimit} else {Invalid})
    }

    /// Absolute device-x/device-y derivative bounds of the corresponding exact
    /// native affine component field. They do not include sampler projection or
    /// rounding, which require the separate pointwise component error envelope.
    pub fn mesh_native_affine_gradient_bounds(&self,index:usize,vertices:&[Point],
        destination:&ColorSpace,cancelled:&dyn Fn()->bool,remaining:&mut usize,
    )->Result<[[f64;2];4],NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        if !self.mesh_has_affine_native_field(destination) {return Err(Unsupported);}
        let fields=self.mesh_triangle_affine_weight_fields(index,vertices,cancelled,remaining)?;
        self.mesh_native_gradient_from_derivatives(index,fields.map(|f|[f.coeff[0],f.coeff[1]]),cancelled,remaining)
    }
    fn mesh_native_gradient_from_derivatives(&self,index:usize,derivatives:[[MeshWeightInterval;2];3],
        cancelled:&dyn Fn()->bool,remaining:&mut usize)->Result<[[f64;2];4],NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        let triangle=&self.mesh.as_ref().ok_or(Unsupported)?.triangles[index];
        let mut bounds=[[0.;2];4];
        for (channel,out) in bounds.iter_mut().enumerate().take(self.count) {
            let values=triangle.colors.map(|color|color[channel]);
            if values.iter().any(|v|!v.is_finite() || !(0. ..=1.).contains(v)) {return Err(Invalid);}
            for (axis,bound) in out.iter_mut().enumerate() {
                let mut derivative=MeshWeightInterval::exact(0.).ok_or(Invalid)?;
                for (field,value) in derivatives.iter().zip(values) {
                    let value=MeshWeightInterval::exact(f64::from(value)).ok_or(Invalid)?;
                    let term=field[axis].op(value,2,remaining,cancelled).ok_or_else(||if cancelled() {Cancelled} else if *remaining==0 {WorkLimit} else {Invalid})?;
                    derivative=derivative.op(term,0,remaining,cancelled).ok_or_else(||if cancelled() {Cancelled} else if *remaining==0 {WorkLimit} else {Invalid})?;
                }
                *bound=derivative.lo.abs().max(derivative.hi.abs()).next_up();
                if !bound.is_finite() {return Err(Invalid);}
            }
        }
        if cancelled() {return Err(Cancelled);}Ok(bounds)
    }

    /// Pointwise absolute component error versus the corresponding exact affine
    /// expression over a supplied vertex hull. Includes raw-weight evaluation,
    /// negative-weight projection, normalization and source f32 rounding.
    /// Native coordinate identity and finite0..1vertex components are required.
    /// Caller must separately prove fragment/centroid domain membership and
    /// propagate these bounds through composition and moment accumulation.
    pub fn mesh_native_affine_component_error_bounds(&self,index:usize,vertices:&[Point],
        destination:&ColorSpace,cancelled:&dyn Fn()->bool,remaining:&mut usize,
    )->Result<[f64;4],NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        if !self.mesh_has_affine_native_field(destination) {return Err(Unsupported);}
        let fields=self.mesh_triangle_affine_weight_fields(index,vertices,cancelled,remaining)?;
        let add=|a:f64,b:f64|(a+b).next_up();
        let mut evaluation=0.;let mut deficit=0.;let mut positive=0.;
        for field in fields {
            let bound=field.over_vertices(vertices,remaining,cancelled).ok_or_else(||if cancelled() {Cancelled} else if *remaining==0 {WorkLimit} else {Invalid})?;
            evaluation=add(evaluation,field.error);
            deficit=add(deficit,(-bound[0]).max(0.));
            positive=add(positive,bound[1].max(0.));
        }
        self.mesh_native_component_bound_from_limits(index,evaluation,deficit,positive,cancelled,remaining)
    }
    fn mesh_native_component_bound_from_limits(&self,index:usize,evaluation:f64,deficit:f64,positive:f64,
        cancelled:&dyn Fn()->bool,remaining:&mut usize)->Result<[f64;4],NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        let add=|a:f64,b:f64|(a+b).next_up();let mul=|a:f64,b:f64|(a*b).next_up();let div=|a:f64,b:f64|(a/b).next_up();
        *remaining=remaining.checked_sub(34).ok_or(WorkLimit)?;
        // Exact affine weights sum to1. Their computed sum differs by at most
        // evaluation; clipping adds deficit. Bound the rounded positive sum,
        // then the L1 error from division by that rounded sum.
        let tiny=f64::from_bits(1);
        let sum_error=add(mul(4.*f64::EPSILON,positive),4.*tiny);
        let total_lower=((1.-evaluation).next_down()-sum_error).next_down();
        if !total_lower.is_finite() || total_lower<=0.5 {return Err(Unsupported);}
        let normalized_error=add(div(sum_error,total_lower),add(mul(6.*f64::EPSILON,div(positive,total_lower)),3.*tiny));
        let l1=add(mul(2.,add(evaluation,deficit)),normalized_error);
        let arithmetic=mul(16.*f64::EPSILON,add(add(div(positive,total_lower),normalized_error),1.));
        // Exact positive normalized colors are convex combinations in[0,1].
        // Normalization/accumulation can exceed1 only by this outward bound.
        // Below1 the largest f32 half-spacing is2^-25. Up to1+2^-25,
        // nearest-f32 rounding still selects1, with error at most2^-25.
        let half_unit_spacing=f64::from(f32::EPSILON)/4.;
        let cast_error=if add(normalized_error,arithmetic)<=half_unit_spacing {half_unit_spacing} else {f64::from(f32::EPSILON)};
        let triangle=&self.mesh.as_ref().ok_or(Unsupported)?.triangles[index];
        let mut result=[0.;4];
        for (channel,out) in result.iter_mut().enumerate().take(self.count) {
            if cancelled() {return Err(Cancelled);}
            *remaining=remaining.checked_sub(3).ok_or(WorkLimit)?;
            let values=triangle.colors.map(|c|c[channel]);
            if values.iter().any(|v|!v.is_finite() || !(0. ..=1.).contains(v)) {return Err(Invalid);}
            let low=values.iter().copied().fold(f32::INFINITY,f32::min);
            let high=values.iter().copied().fold(f32::NEG_INFINITY,f32::max);
            let span=(f64::from(high)-f64::from(low)).next_up();
            // Subtract one constant vertex color before applying the L1 bound;
            // only normalization affects that constant. The certified convex
            // range determines the final nearest-f32 rounding allowance.
            *out=add(add(mul(span,l1),normalized_error),add(arithmetic,cast_error));
            if !out.is_finite() {return Err(Invalid);}
        }
        if cancelled() {return Err(Cancelled);}
        Ok(result)
    }

    /// Prepare a source/owner-bound rounding envelope over a closed device
    /// rectangle. Later observations explicitly validate each evaluated point;
    /// no assumption that points remain inside the source triangle is required.
    pub fn prepare_native_affine_point_evidence(&self,index:usize,domain:kurbo::Rect,
        destination:&ColorSpace,cancelled:&dyn Fn()->bool,remaining:&mut usize,
    )->Result<NativeAffinePointEvidence<'_>,NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        if !self.mesh_has_affine_native_field(destination) {return Err(Unsupported);}
        self.prepare_native_affine_point_evidence_in_domain(index,domain,None,cancelled,remaining)
    }
    /// Prepare the common inverse-transform envelope once for a device rectangle.
    pub fn prepare_native_affine_domain(&self,domain:kurbo::Rect,destination:&ColorSpace,
        cancelled:&dyn Fn()->bool,remaining:&mut usize)->Result<NativeAffineDomain<'_>,NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        if !self.mesh_has_affine_native_field(destination) {return Err(Unsupported);}
        if [domain.x0,domain.y0,domain.x1,domain.y1].iter().any(|v|!v.is_finite()) || domain.x0>domain.x1 || domain.y0>domain.y1 {return Err(Invalid);}
        let vertices=[Point::new(domain.x0,domain.y0),Point::new(domain.x1,domain.y0),Point::new(domain.x1,domain.y1),Point::new(domain.x0,domain.y1)];
        let inputs=self.native_affine_device_inputs(&vertices,cancelled,remaining);
        if cancelled() {return Err(Cancelled);}
        let inputs=inputs.ok_or(if *remaining==0 {WorkLimit} else {Invalid})?;
        Ok(NativeAffineDomain {source:self,domain,inputs})
    }
    fn prepare_native_affine_point_evidence_in_domain(&self,index:usize,domain:kurbo::Rect,inputs:Option<[AffineMeshWeight;2]>,
        cancelled:&dyn Fn()->bool,remaining:&mut usize)->Result<NativeAffinePointEvidence<'_>,NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        if [domain.x0,domain.y0,domain.x1,domain.y1].iter().any(|v|!v.is_finite()) || domain.x0>domain.x1 || domain.y0>domain.y1 {return Err(Invalid);}
        let vertices=[Point::new(domain.x0,domain.y0),Point::new(domain.x1,domain.y0),Point::new(domain.x1,domain.y1),Point::new(domain.x0,domain.y1)];
        let fields=self.mesh_triangle_affine_weight_fields_with_inputs(index,&vertices,inputs,cancelled,remaining)?;
        *remaining=remaining.checked_sub(3+3*self.count).ok_or(WorkLimit)?;
        let triangle=&self.mesh.as_ref().ok_or(Unsupported)?.triangles[index];
        if triangle.colors.iter().any(|c|c[..self.count].iter().any(|v|!v.is_finite() || !(0. ..=1.).contains(v))) {return Err(Invalid);}
        let evaluation=fields.iter().fold(0f64,|sum,f|(sum+f.error).next_up());
        if !evaluation.is_finite() || evaluation>=0.25 {return Err(Unsupported);}
        if cancelled() {return Err(Cancelled);}
        *remaining=remaining.checked_sub(12).ok_or(WorkLimit)?;
        Ok(NativeAffinePointEvidence {source:self,index,domain,evaluation,max_deficit:0.,points:0,
            weight_derivatives:fields.map(|f|[f.coeff[0],f.coeff[1]])})
    }

    /// Sum of negative computed barycentric weights before selected-owner
    /// projection. Diagnostic only: zero does not certify exact affine arithmetic
    /// or containment over an integration domain. Performs no color sampling.
    pub fn mesh_triangle_projection_deficit(&self,index:usize,point:Point,
        cancelled:&dyn Fn()->bool,remaining:&mut usize)->Result<f64,NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        *remaining=remaining.checked_sub(1).ok_or(WorkLimit)?;
        let (raw,_)=self.mesh_triangle_weights(index,point)?;
        let deficit=raw.iter().map(|&v|(-v).max(0.)).sum::<f64>();
        if cancelled() {return Err(Cancelled);}
        if !deficit.is_finite() {return Err(Invalid);}
        Ok(deficit)
    }

    // Shared geometry-only solve: evidence and color evaluation must agree on
    // finite projection/normalization. No component or function is evaluated.
    fn mesh_triangle_weights(&self,index:usize,point:Point)->Result<([f64;3],[f64;3]),NativeShadingError> {
        use NativeShadingError::*;
        let triangle=self.mesh.as_ref().ok_or(Unsupported)?.triangles.get(index).ok_or(Invalid)?;
        let p=self.inverse*point;let [a,b,c]=triangle.points;
        let cross=|a:kurbo::Vec2,b:kurbo::Vec2|a.x*b.y-a.y*b.x;
        let denominator=cross(b-a,c-a);
        if !p.x.is_finite() || !p.y.is_finite() || !denominator.is_finite() || denominator==0. {return Err(Invalid);}
        let wb=cross(p-a,c-a)/denominator;let wc=cross(b-a,p-a)/denominator;
        let raw=[1.-wb-wc,wb,wc];
        if raw.iter().any(|v|!v.is_finite()) {return Err(Invalid);}
        let projected=raw.map(|w|w.max(0.));let total=projected.iter().sum::<f64>();
        if !total.is_finite() || total<=0. {return Err(Invalid);}
        Ok((raw,projected.map(|w|w/total)))
    }

    /// Visit mesh triangles whose source bounds intersect a device rectangle.
    /// Uses a fixed 32-slot index stack, no heap scratch, and explicit work and
    /// cancellation checks. Bounds are conservative under affine transforms;
    /// a device-space separating-axis test removes disjoint triangles.
    /// Callers must clip the returned geometry. Traversal is not source order.
    /// Discard accumulated callback state on error, including callback refusal.
    pub fn visit_mesh_triangles(
        &self,rect:kurbo::Rect,work_limit:usize,cancelled:&dyn Fn()->bool,
        visit:&mut dyn FnMut(NativeShadingTriangle)->Result<(),NativeShadingError>,
    )->Result<usize,NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {return Err(Cancelled);}
        if [rect.x0,rect.y0,rect.x1,rect.y1].iter().any(|v|!v.is_finite()) || rect.width()<=0. || rect.height()<=0. {return Err(Invalid);}
        let mesh=self.mesh.as_ref().ok_or(Unsupported)?;
        let query=self.inverse.transform_rect_bbox(rect);
        if [query.x0,query.y0,query.x1,query.y1].iter().any(|v|!v.is_finite()) {return Err(Invalid);}
        if mesh.nodes.is_empty() {return Ok(0);}
        let mut stack=[0usize;32];let mut length=1;let mut remaining=work_limit;let mut count=0usize;
        let overlaps=|a:kurbo::Rect|a.x0<=query.x1 && a.x1>=query.x0 && a.y0<=query.y1 && a.y1>=query.y0;
        while length>0 {
            if cancelled() {return Err(Cancelled);}
            remaining=remaining.checked_sub(1).ok_or(WorkLimit)?;
            length-=1;let node=&mesh.nodes[stack[length]];
            if !overlaps(node.bounds) {continue;}
            if let Some(children)=node.children {
                if length+2>stack.len() {return Err(Invalid);}
                stack[length..length+2].copy_from_slice(&children.map(|index| index as usize));length+=2;
            } else {
                for &index in &mesh.order[node.range.start as usize..node.range.end as usize] {
                    if cancelled() {return Err(Cancelled);}
                    remaining=remaining.checked_sub(1).ok_or(WorkLimit)?;
                    let triangle=&mesh.triangles[index];
                    if !overlaps(triangle.bounds()) {continue;}
                    // A zero-area source triangle cannot own covered area.
                    // Filter before transforming; device arithmetic can create
                    // spurious slivers from exactly collinear source vertices.
                    let [a,b,c] = triangle.points;
                    let ab = b-a; let ac = c-a;
                    let denominator = ab.x*ac.y-ab.y*ac.x;
                    if !denominator.is_finite() {return Err(Invalid);}
                    if denominator == 0. {continue;}
                    let points=triangle.points.map(|point|self.forward*point);
                    if points.iter().any(|p|!p.x.is_finite() || !p.y.is_finite()) {return Err(Invalid);}
                    if !triangle_intersects_rect(points,rect)? {continue;}
                    visit(NativeShadingTriangle {index,points})?;
                    if cancelled() {return Err(Cancelled);}
                    count=count.checked_add(1).ok_or(Invalid)?;
                }
            }
        }
        Ok(count)
    }

    fn sample_mesh(
        &self,
        mesh: &NativeMesh,
        point: Point,
        cancelled: &dyn Fn() -> bool,
        remaining: &mut usize,
        shared: bool,
    ) -> Result<Option<NativeShadingSample>, NativeShadingError> {
        use NativeShadingError::*;
        if let Some((index, weights)) = mesh.candidate(point, cancelled,remaining)? {
            let triangle = &mesh.triangles[index];
            let input_count = if self.function.is_some() { 1 } else { self.count };
            *remaining=remaining.checked_sub(input_count).ok_or(WorkLimit)?;
            let mut components=interpolate_mesh_components(&triangle.colors,weights,input_count);
            if let Some(function) = &self.function {
                let input = components[0];
                if (if shared {function.eval_native_with_budget(input,&mut components[0..self.count],cancelled,remaining)}
                    else {function.eval_native_bounded(input,&mut components[0..self.count],cancelled)})
                    .is_none()
                {
                    return Err(if cancelled() { Cancelled } else { Unsupported });
                }
            }
            if components[0..self.count]
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            {
                return Err(Invalid);
            }
            if cancelled() {
                return Err(Cancelled);
            }
            return Ok(Some(NativeShadingSample {
                components,
                count: self.count,
                opacity: self.opacity,
            }));
        }
        if cancelled() {
            return Err(Cancelled);
        }
        Ok(self.background.map(|components| NativeShadingSample {
            components,
            count: self.count,
            opacity: self.opacity,
        }))
    }
    /// Sample at a device-space point. None means outside the painted domain,
    /// while errors are terminal native-evaluation refusals, not transparent pixels.
    /// The caller must independently apply source BBox, path shape and clip coverage.
    pub fn sample(
        &self,
        point: Point,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<NativeShadingSample>, NativeShadingError> {
        let mut remaining=usize::MAX;
        self.sample_impl(point,cancelled,&mut remaining,false)
    }
    /// Share source-node, triangle, component and function work across samples.
    pub fn sample_with_budget(&self,point:Point,cancelled:&dyn Fn()->bool,remaining:&mut usize)
        ->Result<Option<NativeShadingSample>,NativeShadingError> {
        self.sample_impl(point,cancelled,remaining,true)
    }
    fn sample_impl(&self,point:Point,cancelled:&dyn Fn()->bool,remaining:&mut usize,shared:bool)
        ->Result<Option<NativeShadingSample>,NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {
            return Err(Cancelled);
        }
        *remaining=remaining.checked_sub(1).ok_or(WorkLimit)?;
        let p = self.inverse * point;
        if !p.x.is_finite() || !p.y.is_finite() {
            return Err(Invalid);
        }
        if let Some(mesh) = &self.mesh {
            return self.sample_mesh(mesh, p, cancelled,remaining,shared);
        }
        let t = if self.axial {
            let dx = self.coords[2] - self.coords[0];
            let dy = self.coords[3] - self.coords[1];
            let t = ((p.x - self.coords[0]) * dx + (p.y - self.coords[1]) * dy) / (dx * dx + dy * dy);
            if !t.is_finite() {
                return Err(Invalid);
            }
            if (!self.extend[0] && t < 0.0) || (!self.extend[1] && t > 1.0) {
                None
            } else {
                Some(t)
            }
        } else {
            self.radial_position(p)?
        };
        let Some(t) = t else {
            return Ok(self.background.map(|components| NativeShadingSample {
                components,
                count: self.count,
                opacity: self.opacity,
            }));
        };
        let t = t.clamp(0.0, 1.0);
        let input =
            (f64::from(self.domain[0]) + t * (f64::from(self.domain[1]) - f64::from(self.domain[0]))) as f32;
        if !input.is_finite() {
            return Err(Invalid);
        }
        let mut components = [0.; 4];
        let function=self.function.as_ref().ok_or(Unsupported)?;
        if (if shared {function.eval_native_with_budget(input,&mut components[..self.count],cancelled,remaining)}
            else {function.eval_native_bounded(input,&mut components[..self.count],cancelled)}).is_none()
        {
            return Err(if cancelled() { Cancelled } else { Unsupported });
        }
        if components[0..self.count]
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(Invalid);
        }
        if cancelled() {
            return Err(Cancelled);
        }
        Ok(Some(NativeShadingSample {
            components,
            count: self.count,
            opacity: self.opacity,
        }))
    }
    fn radial_position(&self, p: Point) -> Result<Option<f64>, NativeShadingError> {
        let [x0, y0, r0, x1, y1, r1] = self.coords;
        let (px, py, dx, dy, dr) = (p.x - x0, p.y - y0, x1 - x0, y1 - y0, r1 - r0);
        let a = dx * dx + dy * dy - dr * dr;
        let b = -2.0 * (px * dx + py * dy + r0 * dr);
        let c = px * px + py * py - r0 * r0;
        if !a.is_finite() || !b.is_finite() || !c.is_finite() {
            return Err(NativeShadingError::Invalid);
        }
        let roots = if a == 0.0 {
            if b == 0.0 {
                if c == 0.0 {
                    return Err(NativeShadingError::Unsupported);
                } else {
                    return Ok(None);
                }
            }
            [-c / b, -c / b]
        } else {
            let disc = b * b - 4.0 * a * c;
            if !disc.is_finite() {
                return Err(NativeShadingError::Invalid);
            }
            if disc < 0.0 {
                return Ok(None);
            }
            // Stable quadratic roots, avoiding cancellation in one branch.
            let q = -0.5 * (b + disc.sqrt().copysign(b));
            if q == 0.0 {
                [-b / (2.0 * a); 2]
            } else {
                [q / a, c / q]
            }
        };
        let mut selected: Option<f64> = None;
        for t in roots {
            if !t.is_finite() {
                return Err(NativeShadingError::Invalid);
            }
            if (!self.extend[0] && t < 0.0) || (!self.extend[1] && t > 1.0) || r0 + dr * t < 0.0 {
                continue;
            }
            selected = Some(selected.map_or(t, |old| old.max(t)));
        }
        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cache::Cache, shading::Shading};
    use hayro_syntax::object::{Dict, FromBytes};
    use std::{
        cell::Cell,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    fn pattern(bytes: &[u8]) -> ShadingPattern {
        let dict = Dict::from_bytes(bytes).unwrap();
        ShadingPattern {
            paint_background: false,
            shading: Arc::new(Shading::new(&dict, None, &Cache::new()).unwrap()),
            matrix: Affine::IDENTITY,
            opacity: 0.5,
            transfer_function: None,
        }
    }
    pub(super) fn axial() -> ShadingPattern {
        pattern(b"<< /ShadingType 2 /ColorSpace /DeviceCMYK /Coords [0 0 8 0] /Domain [0 1] /Function << /FunctionType 2 /Domain [0 1] /C0 [0.125 0.25 0.5 0.75] /C1 [0.625 0.75 0 0.25] /N 1 >> /Extend [false false] >>")
    }
    pub(super) fn no_credit(_: usize) -> Option<Box<dyn std::any::Any + Send + Sync>> {
        Some(Box::new(()))
    }
    pub(super) fn native_triangle(colors: [[f32; 4]; 3], count: usize) -> crate::shading::Triangle {
        use crate::shading::{TensorProductPatch, Triangle};
        let patch = TensorProductPatch {
            control_points: std::array::from_fn(|i| Point::new((i % 4) as f64, (i / 4) as f64)),
            colors: std::array::from_fn(|_| smallvec::smallvec![0.; 4]),
        };
        let mut generated = Vec::new();
        patch.to_triangles(&mut generated);
        let seed = generated.pop().unwrap();
        let mut vertices = [seed.p0, seed.p1, seed.p2];
        for ((vertex, point), color) in vertices
            .iter_mut()
            .zip([Point::ZERO, Point::new(8., 0.), Point::new(0., 8.)])
            .zip(colors)
        {
            vertex.point = point;
            vertex.colors.clear();
            vertex.colors.extend_from_slice(&color[0..count]);
        }
        let [a, b, c] = vertices;
        Triangle::new(a, b, c)
    }
    #[test]
    fn native_mesh_rectangle_query_affine_work_callback_and_every_cancel() {
        let triangle=native_triangle([[0.25;4];3],4);
        let original=[triangle.p0.point,triangle.p1.point,triangle.p2.point];
        let mut p=axial();p.matrix=Affine::new([2.,0.5,0.25,3.,20.,-7.]);
        Arc::make_mut(&mut p.shading).shading_type=Arc::new(ShadingType::TriangleMesh {triangles:vec![triangle],function:None});
        let source=NativeShadingSource::capture(&p,&||false,&|_|Some(Box::new(()))).unwrap();
        let center=p.matrix*Point::new(1.,1.);
        let rect=kurbo::Rect::new(center.x-0.1,center.y-0.1,center.x+0.1,center.y+0.1);
        let polls=Cell::new(0);let mut found=Vec::new();
        assert_eq!(source.visit_mesh_triangles(rect,128,&||{polls.set(polls.get()+1);false},&mut |t|{found.push(t);Ok(())}),Ok(1));
        assert_eq!(found[0].index,0);assert_eq!(found[0].points,original.map(|point|p.matrix*point));
        for stop in 1..=polls.get() {
            let current=Cell::new(0);
            assert_eq!(source.visit_mesh_triangles(rect,128,&||{current.set(current.get()+1);current.get()==stop},&mut |_|Ok(())),Err(NativeShadingError::Cancelled));
        }
        assert_eq!(source.visit_mesh_triangles(rect,0,&||false,&mut |_|Ok(())),Err(NativeShadingError::WorkLimit));
        assert_eq!(source.visit_mesh_triangles(rect,128,&||false,&mut |_|Err(NativeShadingError::Admission)),Err(NativeShadingError::Admission));
        assert_eq!(source.visit_mesh_triangles(kurbo::Rect::new(1.,1.,0.,0.),128,&||false,&mut |_|Ok(())),Err(NativeShadingError::Invalid));
        assert_eq!(source.visit_mesh_triangles(kurbo::Rect::new(f64::NAN,0.,1.,1.),128,&||false,&mut |_|Ok(())),Err(NativeShadingError::Invalid));
        assert_eq!(source.visit_mesh_triangles(kurbo::Rect::new(-100.,-100.,-99.,-99.),128,&||false,&mut |_|Ok(())),Ok(0));
        println!("MESH_RECT_QUERY cancellation_checkpoints={}",polls.get());
    }

    #[test]
    fn triangle_rectangle_query_rejects_bbox_false_positives_and_keeps_edges() {
        let triangle = [Point::ZERO,Point::new(8.,0.),Point::new(0.,8.)];
        assert!(!triangle_intersects_rect(triangle,kurbo::Rect::new(7.,7.,8.,8.)).unwrap());
        assert!(triangle_intersects_rect(triangle,kurbo::Rect::new(3.,3.,4.,4.)).unwrap());
        assert!(triangle_intersects_rect(triangle,kurbo::Rect::new(8.,0.,9.,1.)).unwrap());
        assert!(!triangle_intersects_rect(triangle,kurbo::Rect::new(8.000000000001,0.,9.,1.)).unwrap());
        let thin = [Point::ZERO,Point::new(8.,0.),Point::new(0.,1e-12)];
        assert!(triangle_intersects_rect(thin,kurbo::Rect::new(0.,0.,1.,1.)).unwrap());
        let mut p = axial();
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: vec![native_triangle([[0.25;4];3],4)], function: None,
        });
        let source=NativeShadingSource::capture(&p,&||false,&no_credit).unwrap();
        assert_eq!(source.visit_mesh_triangles(kurbo::Rect::new(7.,7.,8.,8.),128,&||false,
            &mut |_|panic!("disjoint bounding-box candidate must not reach callback")),Ok(0));
    }

    #[test]
    fn rectangle_query_excludes_zero_area_but_keeps_thin_source_triangles() {
        let mut degenerate = native_triangle([[0.25;4];3],4);
        degenerate.p2.point = degenerate.p0.point;
        let mut thin = native_triangle([[0.5;4];3],4);
        thin.p2.point = Point::new(0.,1e-12);
        let mut p = axial();
        p.matrix = Affine::new([2.,0.5,0.25,3.,20.,-7.]);
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: vec![degenerate,thin], function: None,
        });
        let source = NativeShadingSource::capture(&p,&||false,&no_credit).unwrap();
        let mut indices = Vec::new();
        let count = source.visit_mesh_triangles(kurbo::Rect::new(-100.,-100.,100.,100.),128,
            &||false,&mut |triangle| {indices.push(triangle.index);Ok(())}).unwrap();
        assert_eq!(count,1);
        assert_eq!(indices,[1]);
    }

    #[test]
    fn batched_cmyk_interpolation_matches_scalar_bits_including_signed_zero() {
        let mut state=0x59a05cb7u32;
        for case in 0..1024 {
            let mut next=|| {state=state.wrapping_mul(1664525).wrapping_add(1013904223);(state>>8) as f32/16777216.};
            let mut colors=std::array::from_fn(|_|std::array::from_fn(|_|next()));
            if case==0 {colors=[[-0.;4];3];}
            if case==1 {colors=[[f32::from_bits(1);4];3];}
            if case==2 {colors=[[f32::MIN_POSITIVE;4];3];}
            if case==3 {colors=[[1.;4];3];}
            let weights=[f64::from(next()),f64::from(next()),f64::from(next())];
            let total=weights.iter().sum::<f64>();let weights=weights.map(|w|w/total);
            for count in [1,3,4] {
                let mut expected=[0f32;4];
                for channel in 0..count {
                    expected[channel]=(0..3).map(|v|weights[v]*f64::from(colors[v][channel])).sum::<f64>() as f32;
                }
                assert_eq!(interpolate_mesh_components(&colors,weights,count).map(f32::to_bits),expected.map(f32::to_bits));
            }
        }
    }

    #[test]
    #[ignore = "explicit native mesh interpolation timing"]
    fn native_selected_mesh_interpolation_benchmark() {
        let mut p=axial();
        Arc::make_mut(&mut p.shading).shading_type=Arc::new(ShadingType::TriangleMesh {
            triangles:vec![native_triangle([[0.125,0.25,0.5,0.75],[0.625,0.75,0.125,0.25],[0.25,0.125,0.75,0.5]],4)],function:None,
        });
        let source=NativeShadingSource::capture(&p,&||false,&no_credit).unwrap();
        let iterations=5_000_000usize;let mut work=usize::MAX;let mut checksum=[0f64;4];
        let started=std::time::Instant::now();
        for i in 0..iterations {
            let point=Point::new((i&1023) as f64/256.,((i>>10)&1023) as f64/256.);
            let sample=std::hint::black_box(&source).sample_mesh_triangle_with_budget(0,std::hint::black_box(point),&||false,&mut work).unwrap();
            for (sum,value) in checksum.iter_mut().zip(std::hint::black_box(sample.components)) {*sum+=f64::from(value);}
        }
        let elapsed=started.elapsed();
        assert_eq!(usize::MAX-work,iterations*5);
        println!("NATIVE_MESH_INTERPOLATION_BENCH iterations={iterations} elapsed_ns={} work={} checksum={checksum:?}",elapsed.as_nanos(),usize::MAX-work);
    }

    #[test]
    fn affine_mesh_capability_refuses_functions_transforms_and_invalid_metadata() {
        let axial_source=NativeShadingSource::capture(&axial(),&||false,&no_credit).unwrap();
        assert!(!axial_source.mesh_has_affine_native_field(&axial_source.color_space));
        let mut p=axial();
        Arc::make_mut(&mut p.shading).shading_type=Arc::new(ShadingType::TriangleMesh {
            triangles:vec![native_triangle([[0.125,0.25,0.5,0.75],[0.625,0.75,0.125,0.25],[0.25,0.125,0.75,0.5]],4)],function:None,
        });
        let mut source=NativeShadingSource::capture(&p,&||false,&no_credit).unwrap();
        assert!(source.mesh_has_affine_native_field(&source.color_space));
        let rgb=crate::color::Color::from_rgba(crate::color::AlphaColor::WHITE);
        assert!(!source.mesh_has_affine_native_field(rgb.color_space()));
        source.count=3;assert!(!source.mesh_has_affine_native_field(&source.color_space));source.count=4;
        source.opacity=f32::NAN;assert!(!source.mesh_has_affine_native_field(&source.color_space));source.opacity=0.5;
        source.function=axial_source.function;
        assert!(!source.mesh_has_affine_native_field(&source.color_space));
        source.function=None;source.mesh=None;
        assert!(!source.mesh_has_affine_native_field(&source.color_space));
    }

    #[test]
    fn mesh_component_bounds_cover_rounded_samples_and_refuse_unsafe_sources() {
        let mut pattern=axial();
        let colors=[[0.,f32::from_bits(1),0.5,1.],[0.25,f32::MIN_POSITIVE,0.5,0.75],[1.,0.125,0.5,0.]];
        Arc::make_mut(&mut pattern.shading).shading_type=Arc::new(ShadingType::TriangleMesh {triangles:vec![native_triangle(colors,4)],function:None});
        let source=NativeShadingSource::capture(&pattern,&||false,&no_credit).unwrap();
        let mut budget=13;
        let (bounds,opacity)=source.mesh_native_component_bounds(0,&source.color_space,&||false,&mut budget).unwrap();
        assert_eq!(budget,0);assert_eq!(opacity,0.5);
        for y in -8..=32 {for x in -8..=32 {
            let point=Point::new(f64::from(x)/2.,f64::from(y)/2.);
            let sample=source.sample_mesh_triangle(0,point,&||false).unwrap();
            for channel in 0..4 {let value=f64::from(sample.components[channel]);assert!(bounds[channel][0]<=value && value<=bounds[channel][1]);}
        }}
        assert!(matches!(source.mesh_native_component_bounds(0,&source.color_space,&||false,&mut 12),Err(NativeShadingError::WorkLimit)));
        assert!(matches!(source.mesh_native_component_bounds(0,&source.color_space,&||true,&mut 13),Err(NativeShadingError::Cancelled)));
        assert!(matches!(source.mesh_native_component_bounds(1,&source.color_space,&||false,&mut 13),Err(NativeShadingError::Invalid)));
        let axial_source=NativeShadingSource::capture(&axial(),&||false,&no_credit).unwrap();
        assert!(matches!(axial_source.mesh_native_component_bounds(0,&axial_source.color_space,&||false,&mut 13),Err(NativeShadingError::Unsupported)));
        let mut source=source;
        source.mesh.as_mut().unwrap().triangles[0].colors[0][0]=f32::NAN;
        assert!(matches!(source.mesh_native_component_bounds(0,&source.color_space,&||false,&mut 13),Err(NativeShadingError::Invalid)));
    }

    #[test]
    fn mesh_weight_intervals_enclose_all_rectangle_samples_and_bound_projection() {
        let mut pattern=axial();
        Arc::make_mut(&mut pattern.shading).shading_type=Arc::new(ShadingType::TriangleMesh {triangles:vec![native_triangle([[0.;4],[1.;4],[0.;4]],4)],function:None});
        let mut source=NativeShadingSource::capture(&pattern,&||false,&no_credit).unwrap();
        for points in [
            [Point::ZERO,Point::new(8.,0.),Point::new(0.,8.)],
            [Point::new(0.1,-0.3),Point::new(3.7,0.2),Point::new(-0.8,2.9)],
            [Point::new(0.1,-0.3),Point::new(-0.8,2.9),Point::new(3.7,0.2)],
            [Point::ZERO,Point::new(1.,1.),Point::new(2.,2.000000000001)],
        ] {
        source.mesh.as_mut().unwrap().triangles[0].points=points;
        for inverse in [kurbo::Affine::IDENTITY,kurbo::Affine::new([1.25,-0.5,0.75,2.,-3.,4.]),kurbo::Affine::new([-1.,0.,0.,1.,8.,0.])] {
            source.inverse=inverse;
            for rect in [kurbo::Rect::new(-4.,-2.,8.,8.),kurbo::Rect::new(1.,1.,2.,2.),kurbo::Rect::new(0.,0.,0.,0.)] {
                let mut work=1000;
                let bounds=source.mesh_triangle_weight_bounds(0,rect,&||false,&mut work).unwrap();
                let used=1000-work;assert!(used>1);
                let vertices=[Point::new(rect.x0,rect.y0),Point::new(rect.x1,rect.y0),Point::new(rect.x1,rect.y1),Point::new(rect.x0,rect.y1)];
                let affine_bounds=source.mesh_triangle_weight_bounds_over_vertices(0,&vertices,&||false,&mut 1000).unwrap();
                for y in 0..=16 {for x in 0..=16 {
                    let point=Point::new(rect.x0+rect.width()*f64::from(x)/16.,rect.y0+rect.height()*f64::from(y)/16.);
                    let p=source.inverse*point;let [a,b,c]=points;let b=b-a;let c=c-a;
                    let cross=|a:kurbo::Vec2,b:kurbo::Vec2|a.x*b.y-a.y*b.x;
                    let denominator=cross(b,c);let wb=cross(p-a,c)/denominator;let wc=cross(b,p-a)/denominator;
                    let raw=[1.-wb-wc,wb,wc];
                    for (value,[lo,hi]) in raw.into_iter().zip(bounds) {assert!(lo<=value && value<=hi,"{value:?} outside {lo:?}..{hi:?}");}
                    for (value,[lo,hi]) in raw.into_iter().zip(affine_bounds) {assert!(lo<=value && value<=hi,"affine: {value:?} outside {lo:?}..{hi:?}");}
                    let deficit=source.mesh_triangle_projection_deficit(0,point,&||false,&mut 1).unwrap();
                    let upper=bounds.iter().fold(0f64,|sum,[lo,_]|(sum+(-lo).max(0.)).next_up());
                    assert!(deficit<=upper);
                }}
                for budget in 0..used {assert!(source.mesh_triangle_weight_bounds(0,rect,&||false,&mut {budget}).is_err());}
            }
        }
        }
        assert!(matches!(source.mesh_triangle_weight_bounds(0,kurbo::Rect::ZERO,&||true,&mut 1000),Err(NativeShadingError::Cancelled)));
        assert!(source.mesh_triangle_weight_bounds(0,kurbo::Rect::new(2.,0.,1.,1.),&||false,&mut 1000).is_err());
        assert!(source.mesh_triangle_weight_bounds(0,kurbo::Rect::new(f64::MAX,0.,f64::MAX,1.),&||false,&mut 1000).is_err());
        source.mesh.as_mut().unwrap().triangles[0].points=[Point::ZERO;3];
        assert!(source.mesh_triangle_weight_bounds(0,kurbo::Rect::ZERO,&||false,&mut 1000).is_err());
    }

    #[test]
    fn affine_weight_domain_keeps_skinny_triangle_correlation_and_refuses_all_short_budgets() {
        let mut pattern=axial();
        Arc::make_mut(&mut pattern.shading).shading_type=Arc::new(ShadingType::TriangleMesh {triangles:vec![native_triangle([[0.;4],[1.;4],[0.;4]],4)],function:None});
        let mut source=NativeShadingSource::capture(&pattern,&||false,&no_credit).unwrap();source.inverse=kurbo::Affine::IDENTITY;
        let points=[Point::ZERO,Point::new(1.,1.),Point::new(2.,2.000000001)];
        source.mesh.as_mut().unwrap().triangles[0].points=points;
        let mut work=1000;
        let bounds=source.mesh_triangle_weight_bounds_over_vertices(0,&points,&||false,&mut work).unwrap();
        let used=1000-work;
        for [lo,hi] in bounds {assert!(lo>=-0.001 && hi<=1.001,"{lo}..{hi}");}
        let rect=source.mesh_triangle_weight_bounds(0,kurbo::Rect::new(0.,0.,2.,2.000000001),&||false,&mut 1000).unwrap();
        assert!(rect.iter().any(|[lo,hi]|hi-lo>1e9));
        for budget in 0..used {assert!(source.mesh_triangle_weight_bounds_over_vertices(0,&points,&||false,&mut {budget}).is_err());}
        assert!(source.mesh_triangle_weight_bounds_over_vertices(0,&[],&||false,&mut 1000).is_err());
        assert!(source.mesh_triangle_weight_bounds_over_vertices(0,&[Point::ZERO;65],&||false,&mut 1000).is_err());
        let polls=std::cell::Cell::new(0);
        source.mesh_triangle_weight_bounds_over_vertices(0,&points,&||{polls.set(polls.get()+1);false},&mut 1000).unwrap();
        for stop in 1..=polls.get() {
            let count=std::cell::Cell::new(0);
            assert!(source.mesh_triangle_weight_bounds_over_vertices(0,&points,&||{count.set(count.get()+1);count.get()>=stop},&mut 1000).is_err());
        }
    }

    #[test]
    fn affine_gradient_bounds_enclose_known_device_derivatives_and_refuse_short_work() {
        let mut pattern=axial();
        Arc::make_mut(&mut pattern.shading).shading_type=Arc::new(ShadingType::TriangleMesh {triangles:vec![native_triangle([[0.,0.5,0.125,1.],[1.,0.5,0.875,0.],[0.,0.5,0.125,1.]],4)],function:None});
        let mut source=NativeShadingSource::capture(&pattern,&||false,&no_credit).unwrap();
        let vertices=[Point::ZERO,Point::new(8.,0.),Point::new(0.,8.)];
        for inverse in [kurbo::Affine::IDENTITY,kurbo::Affine::new([1.25,-0.5,0.75,2.,-3.,4.]),kurbo::Affine::new([-1.,0.,0.,1.,8.,0.])] {
            source.inverse=inverse;let mut work=1000;
            let bounds=source.mesh_native_affine_gradient_bounds(0,&vertices,&source.color_space,&||false,&mut work).unwrap();
            let coefficients=inverse.as_coeffs();
            for (channel,gain) in [1f64,0.,0.75,-1.].into_iter().enumerate() {
                for (axis,coefficient) in [coefficients[0],coefficients[2]].into_iter().enumerate() {
                    let exact=(gain*coefficient/8.).abs();assert!(bounds[channel][axis]>=exact);assert!(bounds[channel][axis]-exact<1e-10);
                }
            }
            for budget in 0..1000-work {assert!(source.mesh_native_affine_gradient_bounds(0,&vertices,&source.color_space,&||false,&mut {budget}).is_err());}
            let polls=std::cell::Cell::new(0);source.mesh_native_affine_gradient_bounds(0,&vertices,&source.color_space,&||{polls.set(polls.get()+1);false},&mut 1000).unwrap();
            for stop in 1..=polls.get() {let count=std::cell::Cell::new(0);assert!(source.mesh_native_affine_gradient_bounds(0,&vertices,&source.color_space,&||{count.set(count.get()+1);count.get()>=stop},&mut 1000).is_err());}
        }
    }

    #[test]
    fn convex_unit_f32_cast_bound_covers_binades_ties_and_small_overshoot() {
        let bound=f64::from(f32::EPSILON)/4.;
        let mut bits=vec![0,1,2,0x007f_fffe,0x007f_ffff];
        for exponent in 1..127u32 {for mantissa in [0,1,2,0x003f_ffff,0x007f_fffe,0x007f_ffff] {bits.push((exponent<<23)|mantissa);}}
        for bits in bits {
            let a=f64::from(f32::from_bits(bits));let b=f64::from(f32::from_bits(bits+1));
            let midpoint=(a+b)/2.;
            for value in [a,b,midpoint,midpoint.next_down(),midpoint.next_up()] {
                assert!(value>=0. && value<=1.);
                assert!((f64::from(value as f32)-value).abs()<=bound);
            }
        }
        for value in [1.,(1.+bound).next_down(),1.+bound] {
            assert_eq!(value as f32,1.);assert!((f64::from(value as f32)-value).abs()<=bound);
        }
    }

    #[test]
    fn affine_component_error_bounds_cover_projection_rounding_and_refuse_short_work() {
        let mut pattern=axial();
        Arc::make_mut(&mut pattern.shading).shading_type=Arc::new(ShadingType::TriangleMesh {triangles:vec![native_triangle([[0.,0.5,0.125,1.],[1.,0.5,0.875,0.],[0.,0.5,0.125,1.]],4)],function:None});
        let source=NativeShadingSource::capture(&pattern,&||false,&no_credit).unwrap();
        for vertices in [[Point::ZERO,Point::new(8.,0.),Point::new(0.,8.)],[Point::new(-4.,0.),Point::new(4.,0.),Point::new(0.,4.)]] {
            let mut work=1000;
            let bounds=source.mesh_native_affine_component_error_bounds(0,&vertices,&source.color_space,&||false,&mut work).unwrap();
            let used=1000-work;
            for y in 0..=32 {for x in 0..=32-y {
                let u=f64::from(x)/32.;let v=f64::from(y)/32.;
                let point=vertices[0]+(vertices[1]-vertices[0])*u+(vertices[2]-vertices[0])*v;
                let sample=source.sample_mesh_triangle(0,point,&||false).unwrap();
                let wb=point.x/8.;let ideal=[wb,0.5,0.125+0.75*wb,1.-wb];
                for i in 0..4 {assert!((f64::from(sample.components[i])-ideal[i]).abs()<=bounds[i]);}
            }}
            for budget in 0..used {assert!(source.mesh_native_affine_component_error_bounds(0,&vertices,&source.color_space,&||false,&mut {budget}).is_err());}
            assert!(bounds[1]<4e-8); // Constant channel only needs rounding/normalization allowance.
            let polls=std::cell::Cell::new(0);
            source.mesh_native_affine_component_error_bounds(0,&vertices,&source.color_space,&||{polls.set(polls.get()+1);false},&mut 1000).unwrap();
            for stop in 1..=polls.get() {
                let count=std::cell::Cell::new(0);
                assert!(source.mesh_native_affine_component_error_bounds(0,&vertices,&source.color_space,&||{count.set(count.get()+1);count.get()>=stop},&mut 1000).is_err());
            }

        }
        assert!(matches!(source.mesh_native_affine_component_error_bounds(0,&[Point::ZERO],&source.color_space,&||true,&mut 1000),Err(NativeShadingError::Cancelled)));
    }

    #[test]
    fn selected_mesh_owner_identity_does_not_certify_affine_integration_across_projection() {
        let mut pattern=axial();
        Arc::make_mut(&mut pattern.shading).shading_type=Arc::new(ShadingType::TriangleMesh {
            triangles:vec![native_triangle([[0.;4],[1.;4],[0.;4]],4)],function:None,
        });
        let source=NativeShadingSource::capture(&pattern,&||false,&no_credit).unwrap();
        assert!(source.mesh_has_affine_native_field(&source.color_space));
        let sample=|point|source.sample_mesh_triangle(0,point,&||false).unwrap().components[0];
        let left=Point::new(-4.,0.);let right=Point::new(4.,0.);
        let midpoint=Point::new(0.,0.);
        // Both samples explicitly use the same retained owner. Exterior weight
        // projection makes their midpoint identity insufficient for integration.
        assert_eq!(sample(left),0.);assert_eq!(sample(right),0.5);
        let average=(f64::from(sample(left))+f64::from(sample(right)))/2.;
        assert_eq!(average,0.25);assert_eq!(sample(midpoint),0.);
        let mut diagnostic_work=3;
        assert_eq!(source.mesh_triangle_projection_deficit(0,left,&||false,&mut diagnostic_work).unwrap(),0.5);
        assert_eq!(source.mesh_triangle_projection_deficit(0,right,&||false,&mut diagnostic_work).unwrap(),0.);
        assert_eq!(source.mesh_triangle_projection_deficit(0,midpoint,&||false,&mut diagnostic_work).unwrap(),0.);
        assert_eq!(diagnostic_work,0);
        assert!(matches!(source.mesh_triangle_projection_deficit(0,left,&||false,&mut diagnostic_work),Err(NativeShadingError::WorkLimit)));
        assert!(matches!(source.mesh_triangle_projection_deficit(0,left,&||true,&mut 1),Err(NativeShadingError::Cancelled)));
        assert!(matches!(source.mesh_triangle_projection_deficit(1,left,&||false,&mut 1),Err(NativeShadingError::Invalid)));
        assert!(matches!(source.mesh_triangle_projection_deficit(0,Point::new(f64::NAN,0.),&||false,&mut 1),Err(NativeShadingError::Invalid)));

        assert!(source.sample(left,&||false).unwrap().is_none());
        // The analogous fully contained segment does preserve its affine mean.
        assert_eq!((sample(Point::new(0.,0.))+sample(right))/2.,sample(Point::new(2.,0.)));
        let (bounds,_)=source.mesh_native_component_bounds(0,&source.color_space,&||false,&mut 13).unwrap();
        assert!(bounds[0][0]<=0. && bounds[0][1]>=0.5);
    }

    #[test]
    fn selected_triangle_field_preserves_affine_components_and_constant_edge_values() {
        let colors = [[0.25,0.5,0.75,0.125], [0.75,0.5,0.25,0.375], [0.25,1.,0.5,0.625]];
        let mut p = axial();
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: vec![native_triangle(colors,4)], function: None,
        });
        let source = NativeShadingSource::capture(&p,&||false,&no_credit).unwrap();
        let sample = source.sample_mesh_triangle(0,Point::new(2.,2.),&||false).unwrap();
        let mut work=100;
        let budgeted=source.sample_mesh_triangle_with_budget(0,Point::new(2.,2.),&||false,&mut work).unwrap();
        assert_eq!(budgeted.components,sample.components);assert_eq!(budgeted.opacity,sample.opacity);
        let cost=100-work;assert!(cost>1);let mut shared=2*cost-1;
        assert!(source.sample_mesh_triangle_with_budget(0,Point::new(2.,2.),&||false,&mut shared).is_ok());
        assert!(source.sample_mesh_triangle_with_budget(0,Point::new(2.,2.),&||false,&mut shared).is_err());
        assert!(matches!(source.sample_mesh_triangle_with_budget(0,Point::new(2.,2.),&||false,&mut 0),Err(NativeShadingError::WorkLimit)));
        assert!(matches!(source.sample_mesh_triangle_with_budget(0,Point::new(2.,2.),&||true,&mut 100),Err(NativeShadingError::Cancelled)));

        assert_eq!(sample.components,[0.375,0.625,0.5625,0.3125]);
        assert_eq!((sample.count,sample.opacity),(4,0.5));
        assert_eq!(source.sample_mesh_triangle(1,Point::ZERO,&||false).err(),Some(NativeShadingError::Invalid));
        assert_eq!(source.sample_mesh_triangle(0,Point::ZERO,&||true).err(),Some(NativeShadingError::Cancelled));
        assert_eq!(source.sample_mesh_triangle(0,Point::new(f64::NAN,0.),&||false).err(),Some(NativeShadingError::Invalid));
        assert_eq!(source.sample_mesh_triangle(0,Point::new(f64::INFINITY,100.),&||false).err(),Some(NativeShadingError::Invalid));
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: vec![native_triangle([[0.25,0.5,0.75,0.125];3],4)], function: None,
        });
        let source = NativeShadingSource::capture(&p,&||false,&no_credit).unwrap();
        // Field evaluation has an explicit selected/clipped-owner contract;
        // the hit-test API still rejects a point just outside that triangle.
        let point = Point::new(-1e-12,1.);
        assert!(source.sample(point,&||false).unwrap().is_none());
        assert_eq!(source.sample_mesh_triangle(0,point,&||false).unwrap().components,[0.25,0.5,0.75,0.125]);
    }

    #[test]
    fn native_mesh_index_matches_source_order_with_bounded_sparse_query_work() {
        let mut triangles = Vec::new();
        for index in 0..1024 {
            let value = (index % 256) as f32 / 256.;
            let mut triangle = native_triangle([[value; 4]; 3], 4);
            let offset = kurbo::Vec2::new((index % 32) as f64 * 16., (index / 32) as f64 * 16.);
            for vertex in [&mut triangle.p0, &mut triangle.p1, &mut triangle.p2] {
                vertex.point += offset;
            }
            triangles.push(crate::shading::Triangle::new(
                triangle.p0,
                triangle.p1,
                triangle.p2,
            ));
        }
        // Later, distant-in-source-order overlaps must win regardless of BVH order.
        for index in (0..1024).step_by(17) {
            let mut triangle = triangles[index].clone();
            for vertex in [&mut triangle.p0, &mut triangle.p1, &mut triangle.p2] {
                vertex.colors = smallvec::smallvec![0.75;4];
            }
            triangles.push(triangle);
        }
        let mut p = axial();
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: triangles.clone(),
            function: None,
        });
        let source = NativeShadingSource::capture(&p, &|| false, &|bytes| {
            (bytes <= 256 * 1024).then(|| Box::new(()) as Box<dyn std::any::Any + Send + Sync>)
        })
        .expect("1085-triangle native source and BVH must fit a 256 KiB allowance");
        let mut max_polls = 0;
        for index in 0..1024 {
            let x=(index%32) as f64*16.+1.;let y=(index/32) as f64*16.+1.;
            let rect=kurbo::Rect::new(x,y,x+1.,y+1.);
            let mut found=Vec::new();
            source.visit_mesh_triangles(rect,128,&||false,&mut |triangle| {found.push(triangle.index);Ok(())}).unwrap();
            found.sort_unstable();
            let expected=triangles.iter().enumerate().filter_map(|(i,t)| {
                let bounds=kurbo::Rect::from_points(t.p0.point,t.p1.point).union_pt(t.p2.point);
                (bounds.x0<=rect.x1 && bounds.x1>=rect.x0 && bounds.y0<=rect.y1 && bounds.y1>=rect.y0).then_some(i)
            }).collect::<Vec<_>>();
            assert_eq!(found,expected,"rectangle query {index}");
            let point = Point::new((index % 32) as f64 * 16. + 1., (index / 32) as f64 * 16. + 1.);
            let expected = triangles
                .iter()
                .rev()
                .find(|t| t.contains_point(point))
                .unwrap()
                .p0
                .colors[0];
            let polls = Cell::new(0);
            let sample = source
                .sample(point, &|| {
                    polls.set(polls.get() + 1);
                    false
                })
                .unwrap()
                .unwrap();
            assert_eq!(sample.components, [expected; 4]);
            max_polls = max_polls.max(polls.get());
            assert!(polls.get() < 128, "sparse query must not scan all 1085 triangles");
        }
        eprintln!(
            "native index: triangles={}, queries=1024, max cancellation checkpoints/query={max_polls}",
            triangles.len()
        );
        assert!(source.sample(Point::new(-1., -1.), &|| false).unwrap().is_none());
        // Within a triangle bounding box but beyond its hypotenuse.
        assert!(source.sample(Point::new(7., 7.), &|| false).unwrap().is_none());
    }
    #[test]
    fn native_tensor_capture_preserves_transformed_cmyk_and_releases_intermediate_credit() {
        use crate::shading::TensorProductPatch;
        struct Credit(Arc<AtomicUsize>, usize);
        impl Drop for Credit {
            fn drop(&mut self) {
                self.0.fetch_sub(self.1, Ordering::Relaxed);
            }
        }
        let mut p = axial();
        let indices = [[0, 1, 2, 3], [11, 12, 13, 4], [10, 15, 14, 5], [9, 8, 7, 6]];
        let mut points = [Point::ZERO; 16];
        for u in 0..4 {
            for v in 0..4 {
                points[indices[u][v]] = Point::new(u as f64 * 8. / 3., v as f64 * 8. / 3.);
            }
        }
        let patch = TensorProductPatch {
            control_points: points,
            colors: [
                smallvec::smallvec![0.125, 0.25, 0.5, 0.75],
                smallvec::smallvec![0.125, 0.25, 0.5, 0.75],
                smallvec::smallvec![0.625, 0.75, 0., 0.25],
                smallvec::smallvec![0.625, 0.75, 0., 0.25],
            ],
        };
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TensorProductPatchMesh {
            patches: vec![patch],
            function: None,
        });
        p.matrix = Affine::translate((10., 20.)) * Affine::scale(2.);
        let used = Arc::new(AtomicUsize::new(0));
        let calls = Cell::new(0);
        let refuse = Cell::new(usize::MAX);
        let admit = |bytes| {
            calls.set(calls.get() + 1);
            if calls.get() == refuse.get() {
                return None;
            }
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Credit(used.clone(), bytes)) as Box<dyn std::any::Any + Send + Sync>)
        };
        let source = NativeShadingSource::capture(&p, &|| false, &admit).unwrap();
        let stats=source.patch_tessellation_stats().expect("tensor origin retained");
        assert_eq!(stats.triangles,source.mesh.as_ref().unwrap().triangles.len());
        assert!(stats.max_pixel_bound.is_finite() && stats.max_pixel_bound<=1./64.);
        assert!(stats.max_component_bound.is_finite() && stats.max_component_bound<=1./1024.);
        let plain=NativeShadingSource::capture(&axial(),&||false,&|_|Some(Box::new(()))).unwrap();
        assert!(plain.patch_tessellation_stats().is_none());
        assert_eq!(
            source
                .sample(Point::new(18., 22.), &|| false)
                .unwrap()
                .unwrap()
                .components,
            [0.375, 0.5, 0.25, 0.5]
        );
        assert!(source.sample(Point::ZERO, &|| false).unwrap().is_none());
        let admissions = calls.get();
        assert!(used.load(Ordering::Relaxed) > 0);
        drop(source);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        for target in 1..=admissions {
            calls.set(0);
            refuse.set(target);
            assert_eq!(
                NativeShadingSource::capture(&p, &|| false, &admit).err(),
                Some(NativeShadingError::Admission)
            );
            assert_eq!(used.load(Ordering::Relaxed), 0);
        }
        refuse.set(usize::MAX);
        let polls = Cell::new(0);
        NativeShadingSource::capture(
            &p,
            &|| {
                polls.set(polls.get() + 1);
                false
            },
            &admit,
        )
        .unwrap();
        let total = polls.get();
        for target in 1..=total {
            polls.set(0);
            assert_eq!(
                NativeShadingSource::capture(
                    &p,
                    &|| {
                        polls.set(polls.get() + 1);
                        polls.get() >= target
                    },
                    &admit
                )
                .err(),
                Some(NativeShadingError::Cancelled)
            );
            assert_eq!(used.load(Ordering::Relaxed), 0);
        }
    }
    #[test]
    fn original_triangle_mesh_transform_overlap_and_function_preserve_cmyk() {
        let triangle = native_triangle([[0., 0., 0., 1.], [1., 0., 0., 0.], [0., 1., 1., 0.]], 4);
        let mut p = axial();
        p.matrix = Affine::translate((10., 20.)) * Affine::scale(2.);
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: vec![triangle.clone()],
            function: None,
        });
        let source = NativeShadingSource::capture(&p, &|| false, &no_credit).unwrap();
        assert_eq!(
            source
                .sample(Point::new(18., 22.), &|| false)
                .unwrap()
                .unwrap()
                .components,
            [0.5, 0.125, 0.125, 0.375]
        );
        assert!(source.sample(Point::new(40., 40.), &|| false).unwrap().is_none());
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: vec![triangle, native_triangle([[0.25; 4]; 3], 4)],
            function: None,
        });
        let source = NativeShadingSource::capture(&p, &|| false, &no_credit).unwrap();
        assert_eq!(
            source
                .sample(Point::new(18., 22.), &|| false)
                .unwrap()
                .unwrap()
                .components,
            [0.25; 4]
        );
        let mut p = axial();
        let ShadingType::RadialAxial { function, .. } = p.shading.shading_type.as_ref() else {
            panic!()
        };
        let function = function.clone();
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: vec![native_triangle([[0.; 4], [1.; 4], [0.; 4]], 1)],
            function: Some(function),
        });
        let source = NativeShadingSource::capture(&p, &|| false, &no_credit).unwrap();
        assert_eq!(
            source
                .sample(Point::new(4., 1.), &|| false)
                .unwrap()
                .unwrap()
                .components,
            [0.375, 0.5, 0.25, 0.5]
        );
    }
    #[test]
    fn triangle_capture_refuses_before_allocation_and_releases_cancelled_credit() {
        struct Credit(Arc<AtomicUsize>, usize);
        impl Drop for Credit {
            fn drop(&mut self) {
                self.0.fetch_sub(self.1, Ordering::Relaxed);
            }
        }
        let mut p = axial();
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: vec![native_triangle([[0.5; 4]; 3], 4); 16],
            function: None,
        });
        assert_eq!(
            NativeShadingSource::capture(&p, &|| false, &|_| None).err(),
            Some(NativeShadingError::Admission)
        );
        let used = Arc::new(AtomicUsize::new(0));
        let admit = |bytes| {
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Credit(used.clone(), bytes)) as Box<dyn std::any::Any + Send + Sync>)
        };
        let polls = Cell::new(0);
        NativeShadingSource::capture(
            &p,
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
                NativeShadingSource::capture(
                    &p,
                    &|| {
                        polls.set(polls.get() + 1);
                        polls.get() >= target
                    },
                    &admit
                )
                .err(),
                Some(NativeShadingError::Cancelled)
            );
            assert_eq!(used.load(Ordering::Relaxed), 0);
        }
        let source = NativeShadingSource::capture(&p, &|| false, &admit).unwrap();
        assert_eq!(
            source.sample(Point::new(1., 1.), &|| true).err(),
            Some(NativeShadingError::Cancelled)
        );
        drop(source);
        assert_eq!(used.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn native_source_samples_share_actual_work_and_preserve_legacy_values() {
        let source=NativeShadingSource::capture(&axial(),&||false,&no_credit).unwrap();
        let point=Point::new(2.,0.);let legacy=source.sample(point,&||false).unwrap().unwrap();
        let mut remaining=1000;
        let shared=source.sample_with_budget(point,&||false,&mut remaining).unwrap().unwrap();
        assert_eq!(shared.components,legacy.components);assert_eq!(shared.opacity,legacy.opacity);
        let cost=1000-remaining;assert!(cost>1);
        let mut budget=2*cost-1;
        assert!(source.sample_with_budget(point,&||false,&mut budget).is_ok());
        assert!(source.sample_with_budget(point,&||false,&mut budget).is_err());
        assert!(matches!(source.sample_with_budget(point,&||false,&mut 0),Err(NativeShadingError::WorkLimit)));
        assert!(matches!(source.sample_with_budget(point,&||true,&mut 1000),Err(NativeShadingError::Cancelled)));
    }
    #[test]
    fn axial_boundary_lines_preserve_parameter_under_shear_and_reflection() {
        for matrix in [Affine::IDENTITY,Affine::new([2.,0.25,0.75,1.,10.,-4.]),
            Affine::new([-0.5,0.25,0.75,2.,-10.,4.])] {
            let mut p=axial();p.matrix=matrix;
            let source=NativeShadingSource::capture(&p,&||false,&no_credit).unwrap();
            let lines=source.axial_boundary_lines().unwrap();
            for (t,line) in lines.iter().enumerate() {
                for point in line {let original=matrix.inverse()**point;
                    assert!((original.x/8.-t as f64).abs()<1e-12);}
            }
        }
        let radial=pattern(b"<< /ShadingType 3 /ColorSpace /DeviceGray /Coords [0 0 0 0 0 8] /Function << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >> >>");
        assert!(NativeShadingSource::capture(&radial,&||false,&no_credit).unwrap().axial_boundary_lines().is_none());
    }

    #[test]
    fn original_cmyk_axial_transform_and_extension_have_exact_samples() {
        let mut p = axial();
        p.matrix = Affine::translate((10., 20.)) * Affine::scale(2.);
        let source = NativeShadingSource::capture(&p, &|| false, &no_credit).unwrap();
        let sample = source.sample(Point::new(18., 22.), &|| false).unwrap().unwrap();
        assert_eq!(sample.components, [0.375, 0.5, 0.25, 0.5]);
        assert_eq!(sample.count, 4);
        assert_eq!(sample.opacity, 0.5);
        assert!(source.sample(Point::new(9., 20.), &|| false).unwrap().is_none());
        assert!(source.sample(Point::new(27., 20.), &|| false).unwrap().is_none());
        let mut p = axial();
        let shading = Arc::make_mut(&mut p.shading);
        let ShadingType::RadialAxial {
            coords,
            domain,
            function,
            ..
        } = shading.shading_type.as_ref()
        else {
            panic!()
        };
        shading.shading_type = Arc::new(ShadingType::RadialAxial {
            coords: *coords,
            domain: *domain,
            function: function.clone(),
            extend: [true, true],
            axial: true,
        });
        let source = NativeShadingSource::capture(&p, &|| false, &no_credit).unwrap();
        assert_eq!(
            source
                .sample(Point::new(-1., 0.), &|| false)
                .unwrap()
                .unwrap()
                .components,
            [0.125, 0.25, 0.5, 0.75]
        );
        assert_eq!(
            source
                .sample(Point::new(9., 0.), &|| false)
                .unwrap()
                .unwrap()
                .components,
            [0.625, 0.75, 0., 0.25]
        );
    }
    #[test]
    fn radial_concentric_translated_and_linear_cone_keep_original_gray() {
        for (coords, point, expected) in [
            ("0 0 0 0 0 8", Point::new(4., 0.), 0.5),
            ("0 0 8 0 0 0", Point::new(4., 0.), 0.5),
            ("0 0 1 4 0 1", Point::new(3., 0.), 1.0),
            ("0 0 0 4 0 4", Point::new(4., 0.), 0.5),
        ] {
            let bytes = format!(
                "<< /ShadingType 3 /ColorSpace /DeviceGray /Coords [{coords}] /Function << /FunctionType 2 /Domain [0 1] /N 1 >> /Extend [false false] >>"
            );
            let p = pattern(bytes.as_bytes());
            let source = NativeShadingSource::capture(&p, &|| false, &no_credit).unwrap();
            assert_eq!(
                source.sample(point, &|| false).unwrap().unwrap().components[0],
                expected
            );
        }
    }
    #[test]
    fn retained_source_admission_and_cancellation_release_all_credit() {
        struct Credit(Arc<AtomicUsize>, usize);
        impl Drop for Credit {
            fn drop(&mut self) {
                self.0.fetch_sub(self.1, Ordering::Relaxed);
            }
        }
        let used = Arc::new(AtomicUsize::new(0));
        let p = axial();
        let admit = |bytes| {
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Credit(used.clone(), bytes)) as Box<dyn std::any::Any + Send + Sync>)
        };
        let source = NativeShadingSource::capture(&p, &|| false, &admit).unwrap();
        assert!(used.load(Ordering::Relaxed) > size_of::<NativeShadingSource>());
        assert_eq!(
            source.sample(Point::ZERO, &|| true).err(),
            Some(NativeShadingError::Cancelled)
        );
        drop(source);
        assert_eq!(used.load(Ordering::Relaxed), 0);
        assert_eq!(
            NativeShadingSource::capture(&p, &|| false, &|_| None).err(),
            Some(NativeShadingError::Admission)
        );
        let polls = Cell::new(0);
        NativeShadingSource::capture(
            &p,
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
            let result = NativeShadingSource::capture(
                &p,
                &|| {
                    polls.set(polls.get() + 1);
                    polls.get() >= target
                },
                &admit,
            );
            assert_eq!(result.err(), Some(NativeShadingError::Cancelled));
            assert_eq!(used.load(Ordering::Relaxed), 0);
        }
    }
}

#[cfg(test)]
mod point_evidence_tests {
    use super::*;
    use super::tests::{axial,native_triangle,no_credit};
    use std::sync::Arc;
    #[test]
    fn opaque_owner_certificate_requires_exact_opacity_and_preserves_source_binding() {
        for opacity in [0.,0.5,1.-f32::EPSILON,1.] {
            let mut pattern=axial();pattern.opacity=opacity;
            Arc::make_mut(&mut pattern.shading).shading_type=Arc::new(crate::shading::ShadingType::TriangleMesh {triangles:vec![native_triangle([[0.25,0.5,0.75,0.125];3],4)],function:None});
            let source=NativeShadingSource::capture(&pattern,&||false,&no_credit).unwrap();
            let domain=source.prepare_native_affine_domain(kurbo::Rect::new(0.,0.,8.,8.),&source.color_space,&||false,&mut 1000).unwrap();
            if opacity!=1. {assert!(matches!(domain.prepare_opaque_owner(0,&||false,&mut 1000),Err(NativeShadingError::Unsupported)));continue;}
            let mut work=1000;let mut evidence=domain.prepare_opaque_owner(0,&||false,&mut work).unwrap();let used=1000-work;
            assert_eq!(evidence.owner_index(),0);assert_eq!(evidence.point_count(),0);
            let sample=evidence.sample_observed(Point::new(2.,2.),&||false,&mut 7).unwrap();assert_eq!(sample.opacity,1.);assert_eq!(sample.components,[0.25,0.5,0.75,0.125]);assert_eq!(evidence.point_count(),1);
            for budget in 0..used {assert!(domain.prepare_opaque_owner(0,&||false,&mut {budget}).is_err());}
            let polls=std::cell::Cell::new(0);domain.prepare_opaque_owner(0,&||{polls.set(polls.get()+1);false},&mut 1000).unwrap();
            for stop in 1..=polls.get() {let n=std::cell::Cell::new(0);assert!(domain.prepare_opaque_owner(0,&||{n.set(n.get()+1);n.get()>=stop},&mut 1000).is_err());}
            assert!(domain.prepare_opaque_owner(1,&||false,&mut 1000).is_err());
        }
    }
    #[test]
    fn shared_domain_preserves_evidence_bits_and_refuses_short_work_or_cancel() {
        let mut pattern=axial();
        Arc::make_mut(&mut pattern.shading).shading_type=Arc::new(crate::shading::ShadingType::TriangleMesh {triangles:vec![native_triangle([[0.,0.5,0.125,1.],[1.,0.5,0.875,0.],[0.,0.5,0.125,1.]],4)],function:None});
        let source=NativeShadingSource::capture(&pattern,&||false,&no_credit).unwrap();
        let rect=kurbo::Rect::new(-4.,-4.,12.,12.);let mut work=10000;
        let shared=source.prepare_native_affine_domain(rect,&source.color_space,&||false,&mut work).unwrap();let setup=10000-work;
        for budget in 0..setup {assert!(source.prepare_native_affine_domain(rect,&source.color_space,&||false,&mut {budget}).is_err());}
        assert!(source.prepare_native_affine_domain(rect,&source.color_space,&||true,&mut 10000).is_err());
        assert!(source.prepare_native_affine_domain(kurbo::Rect::new(2.,0.,1.,1.),&source.color_space,&||false,&mut 10000).is_err());
        assert!(shared.prepare_owner(1,&||false,&mut 10000).is_err());
        let mut oldwork=10000;let mut old=source.prepare_native_affine_point_evidence(0,rect,&source.color_space,&||false,&mut oldwork).unwrap();
        let mut newwork=10000;let mut new=shared.prepare_owner(0,&||false,&mut newwork).unwrap();assert!(newwork>oldwork);assert_eq!(old.evaluation.to_bits(),new.evaluation.to_bits());
        let used=10000-newwork;for budget in 0..used {assert!(shared.prepare_owner(0,&||false,&mut {budget}).is_err());}
        let polls=std::cell::Cell::new(0);shared.prepare_owner(0,&||{polls.set(polls.get()+1);false},&mut 10000).unwrap();
        for stop in 1..=polls.get() {let n=std::cell::Cell::new(0);assert!(shared.prepare_owner(0,&||{n.set(n.get()+1);n.get()>=stop},&mut 10000).is_err());}
        for p in [Point::ZERO,Point::new(-4.,2.),Point::new(12.,12.)] {old.observe(p,&||false,&mut 2).unwrap();new.observe(p,&||false,&mut 2).unwrap();}
        let vertices=[Point::new(rect.x0,rect.y0),Point::new(rect.x1,rect.y0),Point::new(rect.x1,rect.y1),Point::new(rect.x0,rect.y1)];
        let reference=source.mesh_native_affine_gradient_bounds(0,&vertices,&source.color_space,&||false,&mut 10000).unwrap();
        let mut work=10000;let actual=new.gradient_bounds(&||false,&mut work).unwrap();
        assert_eq!(reference.map(|a|a.map(f64::to_bits)),actual.map(|a|a.map(f64::to_bits)));
        assert!(actual[0][0]>=0.125);assert!(actual[2][0]>=0.09375);
        for budget in 0..10000-work {assert!(new.gradient_bounds(&||false,&mut {budget}).is_err());}
        assert!(new.gradient_bounds(&||true,&mut 10000).is_err());
        let count=new.point_count();
        for budget in 0..7 {assert!(new.sample_observed(Point::new(2.,2.),&||false,&mut {budget}).is_err());assert_eq!(new.point_count(),count);}
        let expected=source.sample_mesh_triangle_with_budget(0,Point::new(2.,2.),&||false,&mut 5).unwrap();
        let actual=new.sample_observed(Point::new(2.,2.),&||false,&mut 7).unwrap();assert_eq!(expected.components.map(f32::to_bits),actual.components.map(f32::to_bits));
        old.observe(Point::new(2.,2.),&||false,&mut 2).unwrap();

        assert_eq!(old.max_projection_deficit_upper().to_bits(),new.max_projection_deficit_upper().to_bits());
        assert_eq!(old.component_error_bound(&||false,&mut 1000).unwrap().map(f64::to_bits),new.component_error_bound(&||false,&mut 1000).unwrap().map(f64::to_bits));
    }
    #[test]
    fn geometry_projection_matches_native_sampling_without_component_reads() {
        let mut pattern=axial();
        Arc::make_mut(&mut pattern.shading).shading_type=Arc::new(crate::shading::ShadingType::TriangleMesh {triangles:vec![native_triangle([[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.]],4)],function:None});
        let source=NativeShadingSource::capture(&pattern,&||false,&no_credit).unwrap();
        for (point,expected) in [(Point::new(2.,2.),[0.5,0.25,0.25]),(Point::new(-4.,0.),[1.,0.,0.]),(Point::new(8.,8.),[0.,0.5,0.5])] {
            let (raw,weights)=source.mesh_triangle_weights(0,point).unwrap();assert_eq!(weights,expected);
            let sample=source.sample_mesh_triangle_with_budget(0,point,&||false,&mut 5).unwrap();
            for i in 0..3 {assert_eq!(sample.components[i].to_bits(),(expected[i] as f32).to_bits());}
            let deficit=source.mesh_triangle_projection_deficit(0,point,&||false,&mut 1).unwrap();
            assert_eq!(deficit,raw.iter().map(|v|(-v).max(0.)).sum::<f64>());
        }
        for point in [Point::new(f64::NAN,0.),Point::new(f64::INFINITY,0.),Point::new(f64::MAX,f64::MAX)] {
            assert!(source.mesh_triangle_weights(0,point).is_err());
            assert!(source.mesh_triangle_projection_deficit(0,point,&||false,&mut 1).is_err());
            assert!(source.sample_mesh_triangle_with_budget(0,point,&||false,&mut 5).is_err());
        }
        assert!(source.mesh_triangle_weights(1,Point::ZERO).is_err());
    }
    #[test]
    fn observed_point_certificate_covers_projected_exterior_samples_without_hull_assumptions() {
        let mut pattern=axial();
        Arc::make_mut(&mut pattern.shading).shading_type=Arc::new(crate::shading::ShadingType::TriangleMesh {triangles:vec![native_triangle([[0.,0.5,0.125,1.],[1.,0.5,0.875,0.],[0.,0.5,0.125,1.]],4)],function:None});
        let source=NativeShadingSource::capture(&pattern,&||false,&no_credit).unwrap();
        let mut evidence=source.prepare_native_affine_point_evidence(0,kurbo::Rect::new(-4.,-4.,12.,12.),&source.color_space,&||false,&mut 1000).unwrap();
        assert!(evidence.component_error_bound(&||false,&mut 100).is_err());
        let mut samples=Vec::new();
        for y in -4..=12 {for x in -4..=12 {
            let point=Point::new(f64::from(x),f64::from(y));evidence.observe(point,&||false,&mut 2).unwrap();
            samples.push((source.sample_mesh_triangle(0,point,&||false).unwrap().components,point.x/8.));
        }}
        assert_eq!(evidence.point_count(),289);assert_eq!(evidence.owner_index(),0);assert!(evidence.max_projection_deficit_upper()>0.);
        let bounds=evidence.component_error_bound(&||false,&mut 100).unwrap();
        for (actual,wb) in samples {for (i,ideal) in [wb,0.5,0.125+0.75*wb,1.-wb].into_iter().enumerate() {assert!((f64::from(actual[i])-ideal).abs()<=bounds[i]);}}
        let saved=(evidence.point_count(),evidence.max_projection_deficit_upper());
        assert!(evidence.observe(Point::new(13.,0.),&||false,&mut 2).is_err());
        assert_eq!(saved,(evidence.point_count(),evidence.max_projection_deficit_upper()));
        for budget in 0..2 {assert!(evidence.observe(Point::ZERO,&||false,&mut {budget}).is_err());assert_eq!(saved,(evidence.point_count(),evidence.max_projection_deficit_upper()));}
        let polls=std::cell::Cell::new(0);evidence.observe(Point::ZERO,&||{polls.set(polls.get()+1);false},&mut 2).unwrap();
        let saved=(evidence.point_count(),evidence.max_projection_deficit_upper());
        for stop in 1..=polls.get() {let count=std::cell::Cell::new(0);assert!(evidence.observe(Point::ZERO,&||{count.set(count.get()+1);count.get()>=stop},&mut 2).is_err());assert_eq!(saved,(evidence.point_count(),evidence.max_projection_deficit_upper()));}
    }
    #[test]
    fn point_evidence_preparation_and_finish_refuse_all_short_budgets_and_cancel_checkpoints() {
        let mut pattern=axial();
        Arc::make_mut(&mut pattern.shading).shading_type=Arc::new(crate::shading::ShadingType::TriangleMesh {triangles:vec![native_triangle([[0.25;4];3],4)],function:None});
        let source=NativeShadingSource::capture(&pattern,&||false,&no_credit).unwrap();let domain=kurbo::Rect::new(-1.,-1.,9.,9.);
        let mut work=1000;source.prepare_native_affine_point_evidence(0,domain,&source.color_space,&||false,&mut work).unwrap();
        for budget in 0..1000-work {assert!(source.prepare_native_affine_point_evidence(0,domain,&source.color_space,&||false,&mut {budget}).is_err());}
        let polls=std::cell::Cell::new(0);source.prepare_native_affine_point_evidence(0,domain,&source.color_space,&||{polls.set(polls.get()+1);false},&mut 1000).unwrap();
        for stop in 1..=polls.get() {let count=std::cell::Cell::new(0);assert!(source.prepare_native_affine_point_evidence(0,domain,&source.color_space,&||{count.set(count.get()+1);count.get()>=stop},&mut 1000).is_err());}
        let mut evidence=source.prepare_native_affine_point_evidence(0,domain,&source.color_space,&||false,&mut 1000).unwrap();evidence.observe(Point::ZERO,&||false,&mut 2).unwrap();
        let mut work=100;evidence.component_error_bound(&||false,&mut work).unwrap();
        for budget in 0..100-work {assert!(evidence.component_error_bound(&||false,&mut {budget}).is_err());}
        let polls=std::cell::Cell::new(0);evidence.component_error_bound(&||{polls.set(polls.get()+1);false},&mut 100).unwrap();
        for stop in 1..=polls.get() {let count=std::cell::Cell::new(0);assert!(evidence.component_error_bound(&||{count.set(count.get()+1);count.get()>=stop},&mut 100).is_err());}
        assert_eq!(evidence.point_count(),1);
        assert!(source.prepare_native_affine_point_evidence(0,kurbo::Rect::new(1.,0.,0.,1.),&source.color_space,&||false,&mut 1000).is_err());
        assert!(source.prepare_native_affine_point_evidence(0,kurbo::Rect::new(0.,0.,f64::INFINITY,1.),&source.color_space,&||false,&mut 1000).is_err());
        assert!(source.prepare_native_affine_point_evidence(1,domain,&source.color_space,&||false,&mut 1000).is_err());
    }

}
