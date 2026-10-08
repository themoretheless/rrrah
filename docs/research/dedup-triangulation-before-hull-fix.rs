//! Deterministic bounded Bowyer-Watson target triangulation proposals.
use crate::{geometry::Correspondence, piecewise_warp::PiecewiseError};
use rrrah_core::{MemoryBudget, SharedBuffer};
#[derive(Debug, Clone, Copy)]
pub struct TriangulationPolicy {
    pub maximum_points: usize,
    /// Includes the temporary supertriangle faces.
    pub maximum_triangles: usize,
    pub maximum_predicate_tests: u64,
}
fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn circle(a: [f64; 2], b: [f64; 2], c: [f64; 2], p: [f64; 2]) -> f64 {
    let a = [a[0] - p[0], a[1] - p[1]];
    let b = [b[0] - p[0], b[1] - p[1]];
    let c = [c[0] - p[0], c[1] - p[1]];
    let aa = a[0] * a[0] + a[1] * a[1];
    let bb = b[0] * b[0] + b[1] * b[1];
    let cc = c[0] * c[0] + c[1] * c[1];
    aa * (b[0] * c[1] - b[1] * c[0]) - bb * (a[0] * c[1] - a[1] * c[0]) + cc * (a[0] * b[1] - a[1] * b[0])
}
/// Target topology only; source orientation/nonoverlap must subsequently pass
/// PiecewiseWarp::from_triangles. Ordinary f64 predicates are not exact-predicate
/// certificates. Cocircular zero determinants retain the existing diagonal;
/// insertion follows input order. No source/pixel or copy admission is inferred.
pub fn triangulate_targets(
    points: &[Correspondence],
    dimensions: (u32, u32),
    policy: TriangulationPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<SharedBuffer<[usize; 3]>, PiecewiseError> {
    if cancel() {
        return Err(PiecewiseError::Cancelled);
    }
    let n = points.len();
    let cap = policy.maximum_triangles;
    if n < 3 || dimensions.0 == 0 || dimensions.1 == 0 {
        return Err(PiecewiseError::Invalid);
    }
    if n > policy.maximum_points || cap == 0 {
        return Err(PiecewiseError::Budget);
    }
    let vertices = n.checked_add(3).ok_or(PiecewiseError::Budget)?;
    let edge_cap = cap.checked_mul(3).ok_or(PiecewiseError::Budget)?;
    let scratch_bytes = vertices
        .checked_mul(std::mem::size_of::<[f64; 2]>())
        .and_then(|v| {
            cap.checked_mul(std::mem::size_of::<[usize; 3]>() + std::mem::size_of::<u8>())
                .and_then(|t| v.checked_add(t))
        })
        .and_then(|v| {
            edge_cap
                .checked_mul(std::mem::size_of::<(usize, usize)>())
                .and_then(|e| v.checked_add(e))
        })
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(PiecewiseError::Budget)?;
    let scratch_credit = budget
        .try_reserve(scratch_bytes)
        .map_err(|_| PiecewiseError::Budget)?;
    let mut coordinates: Vec<[f64; 2]> = Vec::new();
    coordinates
        .try_reserve_exact(vertices)
        .map_err(|_| PiecewiseError::Budget)?;
    let mut triangles: Vec<[usize; 3]> = Vec::new();
    triangles
        .try_reserve_exact(cap)
        .map_err(|_| PiecewiseError::Budget)?;
    let mut edges: Vec<(usize, usize)> = Vec::new();
    edges
        .try_reserve_exact(edge_cap)
        .map_err(|_| PiecewiseError::Budget)?;
    let mut bad: Vec<u8> = Vec::new();
    bad.try_reserve_exact(cap).map_err(|_| PiecewiseError::Budget)?;
    if coordinates.capacity() > vertices
        || triangles.capacity() > cap
        || edges.capacity() > edge_cap
        || bad.capacity() > cap
    {
        return Err(PiecewiseError::Budget);
    }
    let mut work = 0u64;
    let mut tick = || -> Result<(), PiecewiseError> {
        if cancel() {
            return Err(PiecewiseError::Cancelled);
        }
        work = work.checked_add(1).ok_or(PiecewiseError::Budget)?;
        if work > policy.maximum_predicate_tests {
            return Err(PiecewiseError::Budget);
        }
        Ok(())
    };
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    for p in points {
        tick()?;
        if p.target.iter().any(|v| !v.is_finite() || *v < 0.)
            || p.target[0] >= f64::from(dimensions.0)
            || p.target[1] >= f64::from(dimensions.1)
        {
            return Err(PiecewiseError::Invalid);
        }
        for axis in 0..2 {
            low[axis] = low[axis].min(p.target[axis]);
            high[axis] = high[axis].max(p.target[axis]);
        }
    }
    let scale = (high[0] - low[0]).max(high[1] - low[1]);
    if scale <= 0. {
        return Err(PiecewiseError::Invalid);
    }
    for p in points {
        let q = [(p.target[0] - low[0]) / scale, (p.target[1] - low[1]) / scale];
        for prior in &coordinates {
            tick()?;
            if *prior == q {
                return Err(PiecewiseError::Invalid);
            }
        }
        coordinates.push(q);
    }
    coordinates.extend_from_slice(&[[-32., -16.], [32., -16.], [0., 32.]]);
    triangles.push([n, n + 1, n + 2]);
    for index in 0..n {
        edges.clear();
        bad.clear();
        for face in &triangles {
            tick()?;
            let determinant = circle(
                coordinates[face[0]],
                coordinates[face[1]],
                coordinates[face[2]],
                coordinates[index],
            );
            if !determinant.is_finite() {
                return Err(PiecewiseError::Invalid);
            }
            let remove = determinant > 0.;
            bad.push(u8::from(remove));
            if !remove {
                continue;
            }
            for edge in [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])] {
                let mut found = None;
                for (i, prior) in edges.iter().enumerate() {
                    tick()?;
                    if *prior == edge || *prior == (edge.1, edge.0) {
                        found = Some(i);
                        break;
                    }
                }
                if let Some(i) = found {
                    edges.remove(i);
                } else {
                    if edges.len() == edge_cap {
                        return Err(PiecewiseError::Budget);
                    }
                    edges.push(edge);
                }
            }
        }
        if !bad.contains(&1) {
            return Err(PiecewiseError::Invalid);
        }
        let mut position = 0;
        triangles.retain(|_| {
            let keep = bad[position] == 0;
            position += 1;
            keep
        });
        for &(a, b) in &edges {
            tick()?;
            let area = orient(coordinates[a], coordinates[b], coordinates[index]);
            if !area.is_finite() || area.abs() <= 1e-14 {
                return Err(PiecewiseError::Invalid);
            }
            if triangles.len() == cap {
                return Err(PiecewiseError::Budget);
            }
            triangles.push(if area > 0. { [a, b, index] } else { [b, a, index] });
        }
    }
    triangles.retain(|face| face.iter().all(|i| *i < n));
    if triangles.is_empty() {
        return Err(PiecewiseError::Invalid);
    }
    if cancel() {
        return Err(PiecewiseError::Cancelled);
    }
    // Separate final payload, charged concurrently with scratch allocations.
    let bytes = triangles
        .len()
        .checked_mul(std::mem::size_of::<[usize; 3]>())
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(PiecewiseError::Budget)?;
    let final_credit = budget.try_reserve(bytes).map_err(|_| PiecewiseError::Budget)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(triangles.len())
        .map_err(|_| PiecewiseError::Budget)?;
    result.extend_from_slice(&triangles);
    if cancel() {
        return Err(PiecewiseError::Cancelled);
    }
    let result = final_credit
        .try_adopt(result)
        .map_err(|_| PiecewiseError::Budget)?;
    drop(coordinates);
    drop(triangles);
    drop(edges);
    drop(bad);
    drop(scratch_credit);
    Ok(result)
}
