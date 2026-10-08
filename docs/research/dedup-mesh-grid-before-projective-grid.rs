//! Managed integer-pixel mesh coordinate atlas. Holes remain absent samples.
use crate::piecewise_warp::{PiecewiseError, PiecewiseWarp};
use rrrah_core::{MemoryBudget, SharedBuffer};
#[derive(Debug, Clone, Copy)]
pub enum MeshDirection {
    TargetToSource,
    SourceToTarget,
}
#[derive(Debug, Clone, Copy)]
pub struct MeshGridPolicy {
    pub maximum_triangles: usize,
    pub maximum_sites: u64,
    pub maximum_face_sites: u64,
}
#[derive(Debug, Clone)]
pub struct MeshGrid {
    region: [u32; 4],
    coordinates: SharedBuffer<[f64; 2]>,
    covered: u64,
}
impl MeshGrid {
    pub fn region(&self) -> [u32; 4] {
        self.region
    }
    pub fn covered_sites(&self) -> u64 {
        self.covered
    }
    pub fn coordinate(&self, x: u32, y: u32) -> Option<[f64; 2]> {
        let dx = x.checked_sub(self.region[0])?;
        let dy = y.checked_sub(self.region[1])?;
        if dx >= self.region[2] || dy >= self.region[3] {
            return None;
        }
        let i = (u64::from(dy) * u64::from(self.region[2]) + u64::from(dx)) as usize;
        let p = self.coordinates[i];
        p.iter().all(|v| v.is_finite()).then_some(p)
    }
}
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn bounds(points: [[f64; 2]; 3], region: [u32; 4]) -> [u32; 4] {
    let end = [region[0] + region[2], region[1] + region[3]];
    let mut low = [0; 2];
    let mut high = [0; 2];
    for axis in 0..2 {
        // Include numerical edge tolerance, matching PiecewiseWarp point queries.
        let min = points.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min);
        let max = points.iter().map(|p| p[axis]).fold(f64::NEG_INFINITY, f64::max);
        let pad = (max - min).abs() * 2e-12;
        low[axis] = (min - pad)
            .ceil()
            .max(f64::from(region[axis]))
            .min(f64::from(end[axis])) as u32;
        high[axis] = ((max + pad).floor() + 1.)
            .max(f64::from(low[axis]))
            .min(f64::from(end[axis])) as u32;
    }
    [low[0], low[1], high[0], high[1]]
}
/// Rasterizes only accepted PiecewiseWarp faces, with complete conservative work
/// admission before allocation. No extrapolation or image/pixel-copy assertion.
/// Borrowed mesh payload remains separately charged by its owner.
pub fn build_mesh_grid(
    mesh: &PiecewiseWarp,
    region: [u32; 4],
    direction: MeshDirection,
    policy: MeshGridPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<MeshGrid, PiecewiseError> {
    if cancel() {
        return Err(PiecewiseError::Cancelled);
    }
    if region[2] == 0
        || region[3] == 0
        || region[0].checked_add(region[2]).is_none()
        || region[1].checked_add(region[3]).is_none()
    {
        return Err(PiecewiseError::Invalid);
    }
    if mesh.triangles().len() > policy.maximum_triangles {
        return Err(PiecewiseError::Budget);
    }
    let sites = u64::from(region[2]) * u64::from(region[3]);
    if sites > policy.maximum_sites {
        return Err(PiecewiseError::Budget);
    }
    let planes = |face: &crate::piecewise_warp::WarpTriangle| match direction {
        MeshDirection::TargetToSource => (face.target, face.source),
        MeshDirection::SourceToTarget => (face.source, face.target),
    };
    let mut face_sites = 0u64;
    for face in mesh.triangles() {
        if cancel() {
            return Err(PiecewiseError::Cancelled);
        }
        let (a, _) = planes(face);
        let b = bounds(a, region);
        face_sites = face_sites
            .checked_add(u64::from(b[2] - b[0]) * u64::from(b[3] - b[1]))
            .ok_or(PiecewiseError::Budget)?;
    }
    if face_sites > policy.maximum_face_sites {
        return Err(PiecewiseError::Budget);
    }
    let count = usize::try_from(sites).map_err(|_| PiecewiseError::Budget)?;
    let bytes = sites
        .checked_mul(std::mem::size_of::<[f64; 2]>() as u64)
        .ok_or(PiecewiseError::Budget)?;
    let credit = budget.try_reserve(bytes).map_err(|_| PiecewiseError::Budget)?;
    let mut coordinates = Vec::new();
    coordinates
        .try_reserve_exact(count)
        .map_err(|_| PiecewiseError::Budget)?;
    if coordinates.capacity() > count {
        return Err(PiecewiseError::Budget);
    }
    for _ in 0..count {
        if cancel() {
            return Err(PiecewiseError::Cancelled);
        }
        coordinates.push([f64::NAN; 2]);
    }
    let mut covered = 0;
    for face in mesh.triangles() {
        if cancel() {
            return Err(PiecewiseError::Cancelled);
        }
        let (a, b) = planes(face);
        let bound = bounds(a, region);
        let area = cross(a[0], a[1], a[2]);
        for y in bound[1]..bound[3] {
            for x in bound[0]..bound[2] {
                if cancel() {
                    return Err(PiecewiseError::Cancelled);
                }
                let p = [f64::from(x), f64::from(y)];
                let w = [
                    cross(a[1], a[2], p) / area,
                    cross(a[2], a[0], p) / area,
                    cross(a[0], a[1], p) / area,
                ];
                if w.iter().any(|v| *v < -1e-12) {
                    continue;
                }
                let q = [
                    w[0] * b[0][0] + w[1] * b[1][0] + w[2] * b[2][0],
                    w[0] * b[0][1] + w[1] * b[1][1] + w[2] * b[2][1],
                ];
                if q.iter().any(|v| !v.is_finite()) {
                    return Err(PiecewiseError::Invalid);
                }
                let i = (u64::from(y - region[1]) * u64::from(region[2]) + u64::from(x - region[0])) as usize;
                let prior = coordinates[i];
                if prior[0].is_finite() {
                    let scale = q.iter().chain(prior.iter()).map(|v| v.abs()).fold(1., f64::max);
                    if (q[0] - prior[0]).abs() > 1e-9 * scale || (q[1] - prior[1]).abs() > 1e-9 * scale {
                        return Err(PiecewiseError::Invalid);
                    }
                } else {
                    covered += 1;
                    coordinates[i] = q;
                }
            }
        }
    }
    if cancel() {
        return Err(PiecewiseError::Cancelled);
    }
    let coordinates = credit
        .try_adopt(coordinates)
        .map_err(|_| PiecewiseError::Budget)?;
    Ok(MeshGrid {
        region,
        coordinates,
        covered,
    })
}
