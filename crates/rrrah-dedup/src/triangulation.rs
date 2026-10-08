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
    coordinates.extend_from_slice(&[[-1e6, -5e5], [1e6, -5e5], [0., 1e6]]);
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
    // A finite supertriangle can leave a dent in the final hull. Reject such
    // a topology rather than returning a successful incomplete domain. Every
    // boundary edge must support the entire point set on its interior side.
    for face in &triangles {
        for (a, b) in [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])] {
            let mut incidences = 0;
            for other in &triangles {
                tick()?;
                if other.contains(&a) && other.contains(&b) {
                    incidences += 1;
                }
            }
            if incidences > 2 {
                return Err(PiecewiseError::Invalid);
            }
            if incidences == 1 {
                for point in &coordinates[..n] {
                    tick()?;
                    if orient(coordinates[a], coordinates[b], *point) < -1e-14 {
                        return Err(PiecewiseError::Invalid);
                    }
                }
            }
        }
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

/// Limits for geometry-only local diagonal proposals. Full source/target
/// conformity must still pass `PiecewiseWarp::from_triangles` afterwards.
#[derive(Debug, Clone, Copy)]
pub struct DiagonalPolicy {
    pub maximum_points: usize,
    pub maximum_triangles: usize,
    pub maximum_flips: usize,
    pub maximum_tests: u64,
    pub minimum_twice_area: f64,
}
/// Preserve all landmarks and faces while replacing a local diagonal when both
/// new source faces have positive area and the target quadrilateral is unchanged.
/// A deterministic first viable neighbor is tried; this is not a complete search
/// over all possible topologies. Refusal returns no partial output.
pub fn propose_source_diagonals(
    points: &[Correspondence],
    faces: &[[usize; 3]],
    policy: DiagonalPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<SharedBuffer<[usize; 3]>, PiecewiseError> {
    if cancel() {
        return Err(PiecewiseError::Cancelled);
    }
    if points.len() > policy.maximum_points || faces.len() > policy.maximum_triangles {
        return Err(PiecewiseError::Budget);
    }
    if points.len() < 3
        || faces.is_empty()
        || !policy.minimum_twice_area.is_finite()
        || policy.minimum_twice_area <= 0.
    {
        return Err(PiecewiseError::Invalid);
    }
    let mut tests = 0u64;
    let mut tick = || -> Result<(), PiecewiseError> {
        if cancel() {
            return Err(PiecewiseError::Cancelled);
        }
        tests = tests.checked_add(1).ok_or(PiecewiseError::Budget)?;
        if tests > policy.maximum_tests {
            return Err(PiecewiseError::Budget);
        }
        Ok(())
    };
    for point in points {
        tick()?;
        if point.source.iter().chain(&point.target).any(|v| !v.is_finite()) {
            return Err(PiecewiseError::Invalid);
        }
    }
    let area = |f: [usize; 3], source: bool| {
        let p = f.map(|i| {
            if source {
                points[i].source
            } else {
                points[i].target
            }
        });
        orient(p[0], p[1], p[2])
    };
    for &face in faces {
        tick()?;
        if face.iter().any(|i| *i >= points.len())
            || face[0] == face[1]
            || face[1] == face[2]
            || face[2] == face[0]
        {
            return Err(PiecewiseError::Invalid);
        }
        if !area(face, false).is_finite()
            || !area(face, true).is_finite()
            || area(face, false) <= policy.minimum_twice_area
        {
            return Err(PiecewiseError::Invalid);
        }
    }
    let bytes = faces
        .len()
        .checked_mul(std::mem::size_of::<[usize; 3]>())
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(PiecewiseError::Budget)?;
    let credit = budget.try_reserve(bytes).map_err(|_| PiecewiseError::Budget)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(faces.len())
        .map_err(|_| PiecewiseError::Budget)?;
    if output.capacity() > faces.len() {
        return Err(PiecewiseError::Budget);
    }
    output.extend_from_slice(faces);
    let mut flips = 0usize;
    loop {
        let mut bad = None;
        for (i, &face) in output.iter().enumerate() {
            tick()?;
            if area(face, true) <= policy.minimum_twice_area {
                bad = Some(i);
                break;
            }
        }
        let Some(i) = bad else { break };
        if flips == policy.maximum_flips {
            return Err(PiecewiseError::Budget);
        }
        let f = output[i];
        let mut replacement = None;
        for (j, &g) in output.iter().enumerate() {
            tick()?;
            if i == j {
                continue;
            }
            let mut shared = [0usize; 2];
            let mut count = 0;
            for v in f {
                if g.contains(&v) {
                    if count < 2 {
                        shared[count] = v;
                    }
                    count += 1;
                }
            }
            if count != 2 {
                continue;
            }
            let c = *f
                .iter()
                .find(|v| !shared.contains(v))
                .ok_or(PiecewiseError::Invalid)?;
            let d = *g
                .iter()
                .find(|v| !shared.contains(v))
                .ok_or(PiecewiseError::Invalid)?;
            let mut proposals = [[c, d, shared[0]], [d, c, shared[1]]];
            let mut target_area = 0.;
            let mut viable = true;
            for h in &mut proposals {
                tick()?;
                if area(*h, false) < 0. {
                    h.swap(0, 1);
                }
                let target = area(*h, false);
                let source = area(*h, true);
                if !target.is_finite()
                    || !source.is_finite()
                    || target <= policy.minimum_twice_area
                    || source <= policy.minimum_twice_area
                {
                    viable = false;
                    break;
                }
                target_area += target;
            }
            let original = area(f, false) + area(g, false);
            // Equality of positive areas certifies a target convex diagonal
            // locally to this explicit floating-point tolerance.
            if viable
                && original.is_finite()
                && (target_area - original).abs() <= 1e-10 * original.abs().max(1.)
            {
                replacement = Some((j, proposals));
                break;
            }
        }
        let Some((j, [a, b])) = replacement else {
            return Err(PiecewiseError::Invalid);
        };
        output[i] = a;
        output[j] = b;
        flips += 1;
    }
    if cancel() {
        return Err(PiecewiseError::Cancelled);
    }
    credit.try_adopt(output).map_err(|_| PiecewiseError::Budget)
}
