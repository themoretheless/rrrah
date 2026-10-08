//! Retained original device-color axial/radial and triangle shading. No display RGB table.
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
}

/// Unconverted source components and source opacity; shape is applied by the caller.
pub struct NativeShadingSample {
    /// Original components in the declared source color space.
    pub components: [f32; 4],
    /// Number of active entries in `components`.
    pub count: usize,
    /// Pattern opacity before geometry and graphics-state mask coverage.
    pub opacity: f32,
}

/// Source metadata is retained only after its conservative capacity admission.
/// Repeated Arc references may be counted more than once, never deduplicated
/// into a smaller allowance. Parsed source metadata allocation precedes capture.
pub struct NativeShadingSource {
    /// Original plain-device source color space.
    pub color_space: ColorSpace,
    function: Option<ShadingFunction>,
    mesh: Option<NativeMesh>,
    inverse: Affine,
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
struct MeshNode {
    bounds: kurbo::Rect,
    range: std::ops::Range<usize>,
    children: Option<[usize; 2]>,
    latest: usize,
}
struct NativeMesh {
    triangles: Vec<NativeTriangle>,
    order: Vec<usize>,
    nodes: Vec<MeshNode>,
}
impl NativeTriangle {
    fn bounds(&self) -> kurbo::Rect {
        let mut bounds = kurbo::Rect::from_points(self.points[0], self.points[1]);
        bounds = bounds.union_pt(self.points[2]); bounds
    }
    fn weights(&self, point: Point) -> Result<Option<[f64;3]>, NativeShadingError> {
        let [a,b,c] = self.points;
        let cross = |x: kurbo::Vec2, y: kurbo::Vec2| x.x*y.y-x.y*y.x;
        let denominator = cross(b-a,c-a);
        if !denominator.is_finite() { return Err(NativeShadingError::Invalid); }
        if denominator == 0.0 { return Ok(None); }
        let weights = [cross(b-point,c-point)/denominator,
            cross(c-point,a-point)/denominator,cross(a-point,b-point)/denominator];
        if weights.iter().any(|v| !v.is_finite()) { return Err(NativeShadingError::Invalid); }
        Ok(if weights.iter().any(|v| *v<0.0) {None} else {Some(weights)})
    }
}
impl NativeMesh {
    fn build_nodes(triangles: &[NativeTriangle], order: &mut [usize], offset: usize,
        nodes: &mut Vec<MeshNode>, cancelled: &dyn Fn()->bool) -> Result<usize,NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() { return Err(Cancelled); }
        let mut bounds = triangles[order[0]].bounds();
        let mut latest = order[0];
        for (i,&index) in order.iter().enumerate() {
            if i%256==0 && cancelled() { return Err(Cancelled); }
            bounds = bounds.union(triangles[index].bounds()); latest=latest.max(index);
        }
        let node = nodes.len();
        nodes.push(MeshNode {bounds,range:offset..offset+order.len(),children:None,latest});
        if order.len()>8 {
            let x_axis = bounds.width() >= bounds.height();
            order.sort_unstable_by(|&a,&b| {
                let center = |index:usize| {
                    let r=triangles[index].bounds();
                    if x_axis {r.x0*0.5+r.x1*0.5} else {r.y0*0.5+r.y1*0.5}
                };
                center(a).total_cmp(&center(b)).then(a.cmp(&b))
            });
            if cancelled() { return Err(Cancelled); }
            let middle = order.len()/2;
            let (left,right) = order.split_at_mut(middle);
            let a=Self::build_nodes(triangles,left,offset,nodes,cancelled)?;
            let b=Self::build_nodes(triangles,right,offset+middle,nodes,cancelled)?;
            nodes[node].children=Some([a,b]);
        }
        Ok(node)
    }
    fn candidate(&self, point: Point, cancelled: &dyn Fn()->bool)
        -> Result<Option<(usize,[f64;3])>,NativeShadingError> {
        use NativeShadingError::*;
        if self.nodes.is_empty() { return Ok(None); }
        // Balanced median splitting of at most 2^20 triangles needs < 32 slots.
        let mut stack=[0usize;32]; let mut length=1;
        let mut selected: Option<(usize,[f64;3])>=None;
        while length>0 {
            if cancelled() { return Err(Cancelled); }
            length-=1; let node=&self.nodes[stack[length]];
            let r=node.bounds;
            if point.x<r.x0 || point.x>r.x1 || point.y<r.y0 || point.y>r.y1
                || selected.as_ref().is_some_and(|(index,_)| node.latest<=*index) { continue; }
            if let Some([a,b])=node.children {
                // Visit the child with later source geometry first for pruning.
                let children=if self.nodes[a].latest>self.nodes[b].latest {[b,a]} else {[a,b]};
                if length+2>stack.len() { return Err(Invalid); }
                stack[length..length+2].copy_from_slice(&children); length+=2;
            } else {
                for &index in &self.order[node.range.clone()] {
                    if cancelled() { return Err(Cancelled); }
                    if selected.as_ref().is_some_and(|(chosen,_)| index<=*chosen) {continue;}
                    if let Some(weights)=self.triangles[index].weights(point)? {selected=Some((index,weights));}
                }
            }
        }
        Ok(selected)
    }
}
impl NativeShadingSource {
    /// Retain supported source geometry/color functions after capacity admission.
    /// Plain-device axial/radial and triangle meshes are supported. Patch meshes,
    /// profiles, transfers and functions other than Type2/Type3 are refused.
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
            .native_device_retained_capacity()
            .ok_or(Unsupported)?;
        let count = usize::from(pattern.shading.color_space.num_components());
        if let ShadingType::TriangleMesh { triangles, function } = pattern.shading.shading_type.as_ref() {
            return Self::capture_mesh(pattern, triangles, function.as_ref(), count, color_bytes, cancelled, admit);
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
        if !matches!(count, 1 | 3 | 4)
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
            inverse,
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
    fn capture_mesh(
        pattern: &ShadingPattern,
        triangles: &[crate::shading::Triangle],
        function: Option<&ShadingFunction>,
        count: usize,
        color_bytes: usize,
        cancelled: &dyn Fn() -> bool,
        admit: &dyn Fn(usize) -> Option<Box<dyn std::any::Any + Send + Sync>>,
    ) -> Result<Self, NativeShadingError> {
        use NativeShadingError::*;
        if !matches!(count, 1 | 3 | 4) || triangles.len() > 1_048_576
            || !pattern.opacity.is_finite() || !(0.0..=1.0).contains(&pattern.opacity)
            || pattern.matrix.as_coeffs().iter().any(|v| !v.is_finite())
            || pattern.matrix.determinant() == 0.0 { return Err(Invalid); }
        let inverse = pattern.matrix.inverse();
        if inverse.as_coeffs().iter().any(|v| !v.is_finite()) { return Err(Invalid); }
        let input_count = if function.is_some() { 1 } else { count };
        for triangle in triangles {
            if cancelled() { return Err(Cancelled); }
            for vertex in [&triangle.p0, &triangle.p1, &triangle.p2] {
                if !vertex.point.x.is_finite() || !vertex.point.y.is_finite()
                    || vertex.colors.len() != input_count
                    || vertex.colors.iter().any(|v| !v.is_finite()
                        || (function.is_none() && !(0.0..=1.0).contains(v))) { return Err(Invalid); }
            }
        }
        let function_bytes = match function {
            Some(f) => f.native_retained_capacity(cancelled)
                .ok_or_else(|| if cancelled() { Cancelled } else { Unsupported })?,
            None => 0,
        };
        let node_capacity = triangles.len().checked_mul(2).ok_or(Invalid)?;
        let bytes = triangles.len().checked_mul(size_of::<NativeTriangle>() + size_of::<usize>())
            .and_then(|v| node_capacity.checked_mul(size_of::<MeshNode>()).and_then(|n|v.checked_add(n)))
            .and_then(|v| v.checked_add(size_of::<Self>()))
            .and_then(|v| v.checked_add(color_bytes))
            .and_then(|v| v.checked_add(function_bytes)).ok_or(Invalid)?;
        let credit = admit(bytes).ok_or(Admission)?;
        let mut mesh = Vec::new();
        mesh.try_reserve_exact(triangles.len()).map_err(|_| Admission)?;
        // Allocators may expose a larger capacity than requested. Never publish
        // retained capacity beyond the admitted allowance.
        if mesh.capacity() > triangles.len() { return Err(Admission); }
        for triangle in triangles {
            if cancelled() { return Err(Cancelled); }
            let vertices = [&triangle.p0, &triangle.p1, &triangle.p2];
            let mut colors = [[0.; 4]; 3];
            for (dst, vertex) in colors.iter_mut().zip(vertices) {
                dst[0..input_count].copy_from_slice(&vertex.colors);
            }
            mesh.push(NativeTriangle { points: vertices.map(|v| v.point), colors });
        }
        let mut order = Vec::new();
        order.try_reserve_exact(triangles.len()).map_err(|_|Admission)?;
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(node_capacity).map_err(|_|Admission)?;
        if order.capacity()>triangles.len() || nodes.capacity()>node_capacity {return Err(Admission);}
        order.extend(0..triangles.len());
        if !order.is_empty() { NativeMesh::build_nodes(&mesh,&mut order,0,&mut nodes,cancelled)?; }
        let mesh = NativeMesh {triangles:mesh,order,nodes};
        let background = if pattern.paint_background {
            match &pattern.shading.background {
                Some(bg) => {
                    if bg.len() != count || bg.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) { return Err(Invalid); }
                    let mut values = [0.; 4]; values[0..count].copy_from_slice(bg); Some(values)
                }
                None => None,
            }
        } else { None };
        if cancelled() { return Err(Cancelled); }
        Ok(Self {
            color_space: pattern.shading.color_space.clone(), function: function.cloned(),
            mesh: Some(mesh), inverse, coords: [0.; 6], domain: [0., 1.],
            extend: [false; 2], axial: false, opacity: pattern.opacity,
            background, count, _credit: credit,
        })
    }
    fn sample_mesh(&self, mesh: &NativeMesh, point: Point, cancelled: &dyn Fn() -> bool)
        -> Result<Option<NativeShadingSample>, NativeShadingError> {
        use NativeShadingError::*;
        if let Some((index,weights)) = mesh.candidate(point,cancelled)? {
            let triangle = &mesh.triangles[index];
            let mut components = [0.;4];
            let input_count = if self.function.is_some() { 1 } else { self.count };
            for channel in 0..input_count {
                components[channel] = (0..3).map(|i| weights[i]*f64::from(triangle.colors[i][channel])).sum::<f64>() as f32;
            }
            if let Some(function) = &self.function {
                let input = components[0];
                if function.eval_native_bounded(input, &mut components[0..self.count], cancelled).is_none() {
                    return Err(if cancelled() { Cancelled } else { Unsupported });
                }
            }
            if components[0..self.count].iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) { return Err(Invalid); }
            if cancelled() { return Err(Cancelled); }
            return Ok(Some(NativeShadingSample { components, count: self.count, opacity: self.opacity }));
        }
        if cancelled() { return Err(Cancelled); }
        Ok(self.background.map(|components| NativeShadingSample { components, count: self.count, opacity: self.opacity }))
    }
    /// Sample at a device-space point. None means outside the painted domain,
    /// while errors are terminal native-evaluation refusals, not transparent pixels.
    /// The caller must independently apply source BBox, path shape and clip coverage.
    pub fn sample(
        &self,
        point: Point,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<NativeShadingSample>, NativeShadingError> {
        use NativeShadingError::*;
        if cancelled() {
            return Err(Cancelled);
        }
        let p = self.inverse * point;
        if !p.x.is_finite() || !p.y.is_finite() {
            return Err(Invalid);
        }
        if let Some(mesh) = &self.mesh { return self.sample_mesh(mesh, p, cancelled); }
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
        if self
            .function.as_ref().ok_or(Unsupported)?
            .eval_native_bounded(input, &mut components[0..self.count], cancelled)
            .is_none()
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
    fn axial() -> ShadingPattern {
        pattern(b"<< /ShadingType 2 /ColorSpace /DeviceCMYK /Coords [0 0 8 0] /Domain [0 1] /Function << /FunctionType 2 /Domain [0 1] /C0 [0.125 0.25 0.5 0.75] /C1 [0.625 0.75 0 0.25] /N 1 >> /Extend [false false] >>")
    }
    fn no_credit(_: usize) -> Option<Box<dyn std::any::Any + Send + Sync>> {
        Some(Box::new(()))
    }
    fn native_triangle(colors: [[f32; 4]; 3], count: usize) -> crate::shading::Triangle {
        use crate::shading::{TensorProductPatch, Triangle};
        let patch = TensorProductPatch {
            control_points: std::array::from_fn(|i| Point::new((i%4) as f64, (i/4) as f64)),
            colors: std::array::from_fn(|_| smallvec::smallvec![0.; 4]),
        };
        let mut generated = Vec::new(); patch.to_triangles(&mut generated);
        let seed = generated.pop().unwrap();
        let mut vertices = [seed.p0, seed.p1, seed.p2];
        for ((vertex, point), color) in vertices.iter_mut()
            .zip([Point::ZERO, Point::new(8.,0.), Point::new(0.,8.)]).zip(colors) {
            vertex.point = point;
            vertex.colors.clear(); vertex.colors.extend_from_slice(&color[0..count]);
        }
        let [a,b,c] = vertices; Triangle::new(a,b,c)
    }
    #[test]
    fn native_mesh_index_matches_source_order_with_bounded_sparse_query_work() {
        let mut triangles=Vec::new();
        for index in 0..1024 {
            let value=(index%256) as f32/256.;
            let mut triangle=native_triangle([[value;4];3],4);
            let offset=kurbo::Vec2::new((index%32) as f64*16.,(index/32) as f64*16.);
            for vertex in [&mut triangle.p0,&mut triangle.p1,&mut triangle.p2] {vertex.point+=offset;}
            triangles.push(crate::shading::Triangle::new(triangle.p0,triangle.p1,triangle.p2));
        }
        // Later, distant-in-source-order overlaps must win regardless of BVH order.
        for index in (0..1024).step_by(17) {
            let mut triangle=triangles[index].clone();
            for vertex in [&mut triangle.p0,&mut triangle.p1,&mut triangle.p2] {vertex.colors=smallvec::smallvec![0.75;4];}
            triangles.push(triangle);
        }
        let mut p=axial();
        Arc::make_mut(&mut p.shading).shading_type=Arc::new(ShadingType::TriangleMesh {triangles:triangles.clone(),function:None});
        let source=NativeShadingSource::capture(&p,&||false,&no_credit).unwrap();
        let mut max_polls=0;
        for index in 0..1024 {
            let point=Point::new((index%32) as f64*16.+1.,(index/32) as f64*16.+1.);
            let expected=triangles.iter().rev().find(|t|t.contains_point(point)).unwrap().p0.colors[0];
            let polls=Cell::new(0);
            let sample=source.sample(point,&||{polls.set(polls.get()+1);false}).unwrap().unwrap();
            assert_eq!(sample.components,[expected;4]);
            max_polls=max_polls.max(polls.get());
            assert!(polls.get()<128,"sparse query must not scan all 1085 triangles");
        }
        eprintln!("native index: triangles={}, queries=1024, max cancellation checkpoints/query={max_polls}",triangles.len());
        assert!(source.sample(Point::new(-1.,-1.),&||false).unwrap().is_none());
        // Within a triangle bounding box but beyond its hypotenuse.
        assert!(source.sample(Point::new(7.,7.),&||false).unwrap().is_none());
    }
    #[test]
    fn original_triangle_mesh_transform_overlap_and_function_preserve_cmyk() {
        let triangle = native_triangle([[0.,0.,0.,1.],[1.,0.,0.,0.],[0.,1.,1.,0.]],4);
        let mut p = axial();
        p.matrix = Affine::translate((10.,20.)) * Affine::scale(2.);
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: vec![triangle.clone()], function: None,
        });
        let source = NativeShadingSource::capture(&p, &||false, &no_credit).unwrap();
        assert_eq!(source.sample(Point::new(18.,22.), &||false).unwrap().unwrap().components, [0.5,0.125,0.125,0.375]);
        assert!(source.sample(Point::new(40.,40.), &||false).unwrap().is_none());
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: vec![triangle, native_triangle([[0.25;4];3],4)], function: None,
        });
        let source = NativeShadingSource::capture(&p, &||false, &no_credit).unwrap();
        assert_eq!(source.sample(Point::new(18.,22.), &||false).unwrap().unwrap().components, [0.25;4]);
        let mut p = axial();
        let ShadingType::RadialAxial { function, .. } = p.shading.shading_type.as_ref() else { panic!() };
        let function = function.clone();
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: vec![native_triangle([[0.;4],[1.;4],[0.;4]],1)], function: Some(function),
        });
        let source = NativeShadingSource::capture(&p, &||false, &no_credit).unwrap();
        assert_eq!(source.sample(Point::new(4.,1.), &||false).unwrap().unwrap().components, [0.375,0.5,0.25,0.5]);
    }
    #[test]
    fn triangle_capture_refuses_before_allocation_and_releases_cancelled_credit() {
        struct Credit(Arc<AtomicUsize>, usize);
        impl Drop for Credit { fn drop(&mut self) { self.0.fetch_sub(self.1, Ordering::Relaxed); } }
        let mut p = axial();
        Arc::make_mut(&mut p.shading).shading_type = Arc::new(ShadingType::TriangleMesh {
            triangles: vec![native_triangle([[0.5;4];3],4);16], function: None,
        });
        assert_eq!(NativeShadingSource::capture(&p, &||false, &|_|None).err(), Some(NativeShadingError::Admission));
        let used = Arc::new(AtomicUsize::new(0));
        let admit = |bytes| {
            used.fetch_add(bytes, Ordering::Relaxed);
            Some(Box::new(Credit(used.clone(),bytes)) as Box<dyn std::any::Any+Send+Sync>)
        };
        let polls = Cell::new(0);
        NativeShadingSource::capture(&p, &|| { polls.set(polls.get()+1); false }, &admit).unwrap();
        let total = polls.get(); assert_eq!(used.load(Ordering::Relaxed),0);
        for target in 1..=total {
            polls.set(0);
            assert_eq!(NativeShadingSource::capture(&p, &|| {
                polls.set(polls.get()+1); polls.get()>=target
            }, &admit).err(), Some(NativeShadingError::Cancelled));
            assert_eq!(used.load(Ordering::Relaxed),0);
        }
        let source = NativeShadingSource::capture(&p, &||false, &admit).unwrap();
        assert_eq!(source.sample(Point::new(1.,1.), &||true).err(),Some(NativeShadingError::Cancelled));
        drop(source); assert_eq!(used.load(Ordering::Relaxed),0);
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
