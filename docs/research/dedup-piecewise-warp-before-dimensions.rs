//! Bounded, conforming piecewise-affine geometry from supplied landmarks/faces.
//! Landmarks and triangulation need independent qualification; no copy decision.
use crate::geometry::Correspondence;
use rrrah_core::{MemoryBudget, SharedBuffer};

#[derive(Debug, Clone, Copy)]
pub struct PiecewisePolicy {
    pub maximum_points: usize,
    pub maximum_triangles: usize,
    /// Landmark pairs plus two triangle-plane pairs per face pair.
    pub maximum_pair_tests: u64,
    pub minimum_twice_area: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PiecewiseError {
    #[error("invalid, degenerate, inverted or overlapping piecewise geometry")]
    Invalid,
    #[error("piecewise memory or work budget exceeded")]
    Budget,
    #[error("piecewise geometry cancelled")]
    Cancelled,
}
#[derive(Debug, Clone, Copy)]
pub struct WarpTriangle {
    pub indices: [usize; 3],
    pub source: [[f64; 2]; 3],
    pub target: [[f64; 2]; 3],
}
#[derive(Debug, Clone)]
pub struct PiecewiseWarp {
    triangles: SharedBuffer<WarpTriangle>,
    source_dimensions: (u32, u32),
    target_dimensions: (u32, u32),
}
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn inside(p: [f64; 2], size: (u32, u32)) -> bool {
    p.iter().all(|v| v.is_finite())
        && p[0] >= 0.
        && p[1] >= 0.
        && p[0] < f64::from(size.0)
        && p[1] < f64::from(size.1)
}
fn pairs(count: usize) -> Option<u64> {
    let n = u64::try_from(count).ok()?;
    n.checked_mul(n.saturating_sub(1)).map(|v| v / 2)
}
fn nonconforming(a: &WarpTriangle, b: &WarpTriangle, source: bool) -> bool {
    let aa = if source { a.source } else { a.target };
    let bb = if source { b.source } else { b.target };
    for (vertices, ids, other, other_ids) in [(aa, a.indices, bb, b.indices), (bb, b.indices, aa, a.indices)]
    {
        let area = cross(other[0], other[1], other[2]);
        for (p, id) in vertices.into_iter().zip(ids) {
            if other_ids.contains(&id) {
                continue;
            }
            let weights = [
                cross(other[1], other[2], p) / area,
                cross(other[2], other[0], p) / area,
                cross(other[0], other[1], p) / area,
            ];
            // Includes unshared vertices on edges: reject T-junctions and
            // ambiguous numerical contacts instead of creating a seam.
            if weights.iter().all(|v| *v >= -1e-12) {
                return true;
            }
        }
    }
    for i in 0..3 {
        for j in 0..3 {
            let a0 = aa[i];
            let a1 = aa[(i + 1) % 3];
            let b0 = bb[j];
            let b1 = bb[(j + 1) % 3];
            let ab = [cross(a0, a1, b0), cross(a0, a1, b1)];
            let ba = [cross(b0, b1, a0), cross(b0, b1, a1)];
            if ab[0] * ab[1] < 0. && ba[0] * ba[1] < 0. {
                return true;
            }
        }
    }
    false
}
impl PiecewiseWarp {
    /// Construct supplied conforming faces. Source/target vertices are copied;
    /// borrowed input payloads/allocator overhead are not charged. Retained face
    /// capacity is charged through SharedBuffer, including clones. Work and
    /// memory are admitted before allocation. Both planes must be nonoverlapping
    /// and every face must preserve orientation; no rejected-face partial result.
    pub fn from_triangles(
        points: &[Correspondence],
        faces: &[[usize; 3]],
        source: (u32, u32),
        target: (u32, u32),
        policy: PiecewisePolicy,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, PiecewiseError> {
        if cancel() {
            return Err(PiecewiseError::Cancelled);
        }
        if points.len() < 3
            || faces.is_empty()
            || source.0 == 0
            || source.1 == 0
            || target.0 == 0
            || target.1 == 0
            || !policy.minimum_twice_area.is_finite()
            || policy.minimum_twice_area <= 0.
        {
            return Err(PiecewiseError::Invalid);
        }
        let work = pairs(points.len())
            .and_then(|v| {
                pairs(faces.len())
                    .and_then(|m| m.checked_mul(2))
                    .and_then(|m| v.checked_add(m))
            })
            .ok_or(PiecewiseError::Budget)?;
        if points.len() > policy.maximum_points
            || faces.len() > policy.maximum_triangles
            || work > policy.maximum_pair_tests
        {
            return Err(PiecewiseError::Budget);
        }
        for (i, p) in points.iter().enumerate() {
            if cancel() {
                return Err(PiecewiseError::Cancelled);
            }
            if !inside(p.source, source) || !inside(p.target, target) {
                return Err(PiecewiseError::Invalid);
            }
            for q in &points[..i] {
                if cancel() {
                    return Err(PiecewiseError::Cancelled);
                }
                if p.source == q.source || p.target == q.target {
                    return Err(PiecewiseError::Invalid);
                }
            }
        }
        let bytes = faces
            .len()
            .checked_mul(std::mem::size_of::<WarpTriangle>())
            .and_then(|v| u64::try_from(v).ok())
            .ok_or(PiecewiseError::Budget)?;
        let credit = budget.try_reserve(bytes).map_err(|_| PiecewiseError::Budget)?;
        let mut triangles: Vec<WarpTriangle> = Vec::new();
        triangles
            .try_reserve_exact(faces.len())
            .map_err(|_| PiecewiseError::Budget)?;
        for indices in faces {
            if cancel() {
                return Err(PiecewiseError::Cancelled);
            }
            if indices.iter().any(|i| *i >= points.len())
                || indices[0] == indices[1]
                || indices[0] == indices[2]
                || indices[1] == indices[2]
            {
                return Err(PiecewiseError::Invalid);
            }
            let mut face = WarpTriangle {
                indices: *indices,
                source: indices.map(|i| points[i].source),
                target: indices.map(|i| points[i].target),
            };
            let a = cross(face.source[0], face.source[1], face.source[2]);
            let b = cross(face.target[0], face.target[1], face.target[2]);
            if a.abs() < policy.minimum_twice_area || b.abs() < policy.minimum_twice_area || a * b <= 0. {
                return Err(PiecewiseError::Invalid);
            }
            if b < 0. {
                face.indices.swap(1, 2);
                face.source.swap(1, 2);
                face.target.swap(1, 2);
            }
            for prior in &triangles {
                if cancel() {
                    return Err(PiecewiseError::Cancelled);
                }
                let mut a = face.indices;
                let mut b = prior.indices;
                a.sort_unstable();
                b.sort_unstable();
                if a == b || nonconforming(&face, prior, false) {
                    return Err(PiecewiseError::Invalid);
                }
                if cancel() {
                    return Err(PiecewiseError::Cancelled);
                }
                if nonconforming(&face, prior, true) {
                    return Err(PiecewiseError::Invalid);
                }
            }
            triangles.push(face);
        }
        if cancel() {
            return Err(PiecewiseError::Cancelled);
        }
        let triangles = credit.try_adopt(triangles).map_err(|_| PiecewiseError::Budget)?;
        Ok(Self {
            triangles,
            source_dimensions: source,
            target_dimensions: target,
        })
    }
    pub fn triangles(&self) -> &[WarpTriangle] {
        &self.triangles
    }
    pub fn map_target(
        &self,
        p: [f64; 2],
        maximum_triangle_tests: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Option<[f64; 2]>, PiecewiseError> {
        self.map(p, false, maximum_triangle_tests, cancel)
    }
    pub fn map_source(
        &self,
        p: [f64; 2],
        maximum_triangle_tests: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Option<[f64; 2]>, PiecewiseError> {
        self.map(p, true, maximum_triangle_tests, cancel)
    }
    fn map(
        &self,
        p: [f64; 2],
        reverse: bool,
        maximum: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Option<[f64; 2]>, PiecewiseError> {
        if cancel() {
            return Err(PiecewiseError::Cancelled);
        }
        if p.iter().any(|v| !v.is_finite()) {
            return Err(PiecewiseError::Invalid);
        }
        if self.triangles.len() > maximum {
            return Err(PiecewiseError::Budget);
        }
        let dimensions = if reverse {
            self.source_dimensions
        } else {
            self.target_dimensions
        };
        if !inside(p, dimensions) {
            if cancel() {
                return Err(PiecewiseError::Cancelled);
            }
            return Ok(None);
        }
        let mut result: Option<[f64; 2]> = None;
        for face in self.triangles.iter() {
            if cancel() {
                return Err(PiecewiseError::Cancelled);
            }
            let (a, b) = if reverse {
                (face.source, face.target)
            } else {
                (face.target, face.source)
            };
            let area = cross(a[0], a[1], a[2]);
            let weights = [
                cross(a[1], a[2], p) / area,
                cross(a[2], a[0], p) / area,
                cross(a[0], a[1], p) / area,
            ];
            if weights.iter().any(|v| *v < -1e-12) {
                continue;
            }
            let q: [f64; 2] = std::array::from_fn(|axis| {
                weights[0] * b[0][axis] + weights[1] * b[1][axis] + weights[2] * b[2][axis]
            });
            if q.iter().any(|v| !v.is_finite()) {
                return Err(PiecewiseError::Invalid);
            }
            if let Some(prior) = result {
                let size = if reverse {
                    self.target_dimensions
                } else {
                    self.source_dimensions
                };
                if (q[0] - prior[0]).abs() > 1e-9 * f64::from(size.0)
                    || (q[1] - prior[1]).abs() > 1e-9 * f64::from(size.1)
                {
                    return Err(PiecewiseError::Invalid);
                }
            }
            result = Some(q);
        }
        if cancel() {
            return Err(PiecewiseError::Cancelled);
        }
        Ok(result)
    }
}
