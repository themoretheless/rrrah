//! Explicit local piecewise geometric/pixel support, never file identity.
use crate::{
    geometry::Correspondence,
    linear::LinearRgbaView,
    local_rank::{LocalRankError, LocalRankEvidence, LocalRankPolicy},
    mesh_grid::{MeshDirection, MeshGridPolicy, build_mesh_grid},
    mesh_rank::compare_mesh_rank_region,
    piecewise_warp::{PiecewiseError, PiecewiseWarp},
};
use rrrah_core::MemoryBudget;
#[derive(Debug, Clone, Copy)]
pub struct PiecewiseLocalRankPolicy {
    pub local: LocalRankPolicy,
    pub maximum_triangles: usize,
    /// Aggregate worst-case source/target witness triangle queries.
    pub maximum_triangle_tests: u64,
    /// Aggregate worst-case triangle rasterization sites of both contexts.
    pub maximum_face_sites: u64,
}
fn inside(p: [f64; 2], r: [u32; 4]) -> bool {
    p[0] >= f64::from(r[0])
        && p[1] >= f64::from(r[1])
        && p[0] < f64::from(r[0]) + f64::from(r[2])
        && p[1] < f64::from(r[1]) + f64::from(r[3])
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn error(e: PiecewiseError) -> LocalRankError {
    match e {
        PiecewiseError::Invalid => LocalRankError::Invalid,
        PiecewiseError::Budget => LocalRankError::Budget,
        PiecewiseError::Cancelled => LocalRankError::Cancelled,
    }
}
/// Borrowed mesh must already have full conformity admission. Retains all
/// supplied points; validates one-to-one distinctness even outside regions.
/// Every regional witness must agree with this mesh in both directions.
/// Both expanded contexts and all worst-case geometry work are prepaid.
/// Mesh memory remains separately charged by its owner; new scratch is released
/// before every return. Unsupported pixels in context may conservatively refuse.
#[allow(clippy::too_many_arguments)]
pub fn compare_piecewise_local_rank_region(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    mesh: &PiecewiseWarp,
    points: &[Correspondence],
    source_region: [u32; 4],
    target_region: [u32; 4],
    policy: PiecewiseLocalRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalRankEvidence, LocalRankError> {
    if cancel() {
        return Err(LocalRankError::Cancelled);
    }
    let p = policy.local;
    let fraction = |v: f64| v.is_finite() && v > 0. && v <= 1.;
    let positive = |v: f64| v.is_finite() && v > 0.;
    let rectangle = |r: [u32; 4], d: (u32, u32)| {
        r[2] > 0
            && r[3] > 0
            && r[0].checked_add(r[2]).is_some_and(|v| v <= d.0)
            && r[1].checked_add(r[3]).is_some_and(|v| v <= d.1)
    };
    if mesh.dimensions() != (source.dimensions(), target.dimensions())
        || !rectangle(source_region, source.dimensions())
        || !rectangle(target_region, target.dimensions())
        || p.minimum_witnesses < 4
        || p.maximum_points < p.minimum_witnesses
        || !positive(p.minimum_point_separation)
        || !positive(p.source_tolerance)
        || !positive(p.target_tolerance)
        || !fraction(p.minimum_coverage)
        || !fraction(p.minimum_agreement)
        || !fraction(p.rank.minimum_contrast)
        || p.rank.minimum_pairs == 0
        || p.rank.radius == 0
    {
        return Err(LocalRankError::Invalid);
    }
    let n = u64::try_from(points.len()).map_err(|_| LocalRankError::Budget)?;
    let t = u64::try_from(mesh.triangles().len()).map_err(|_| LocalRankError::Budget)?;
    let checks = n
        .checked_mul(n.saturating_sub(1))
        .and_then(|v| v.checked_div(2))
        .and_then(|v| v.checked_add(n))
        .ok_or(LocalRankError::Budget)?;
    let tests = n
        .checked_mul(t)
        .and_then(|v| v.checked_mul(2))
        .ok_or(LocalRankError::Budget)?;
    let pad = p
        .rank
        .radius
        .checked_add(p.filter_radius)
        .ok_or(LocalRankError::Budget)?;
    let context = |r: [u32; 4], d: (u32, u32)| {
        let x = r[0].saturating_sub(pad);
        let y = r[1].saturating_sub(pad);
        let right = (r[0] + r[2]).saturating_add(pad).min(d.0);
        let bottom = (r[1] + r[3]).saturating_add(pad).min(d.1);
        [x, y, right - x, bottom - y]
    };
    let sr = context(source_region, source.dimensions());
    let tr = context(target_region, target.dimensions());
    let sites = (u64::from(sr[2]) * u64::from(sr[3]))
        .checked_add(u64::from(tr[2]) * u64::from(tr[3]))
        .ok_or(LocalRankError::Budget)?;
    let reads = sites.checked_mul(5).ok_or(LocalRankError::Budget)?;
    let face_sites = sites.checked_mul(t).ok_or(LocalRankError::Budget)?;
    if points.len() > p.maximum_points
        || mesh.triangles().len() > policy.maximum_triangles
        || checks > p.maximum_point_checks
        || tests > policy.maximum_triangle_tests
        || sites > p.rank.maximum_sites
        || reads > p.rank.maximum_pixel_reads
        || face_sites > policy.maximum_face_sites
    {
        return Err(LocalRankError::Budget);
    }
    let mut witnesses = 0;
    for (i, point) in points.iter().enumerate() {
        if cancel() {
            return Err(LocalRankError::Cancelled);
        }
        if point
            .source
            .iter()
            .chain(point.target.iter())
            .any(|v| !v.is_finite())
            || !inside(point.source, [0, 0, source.dimensions().0, source.dimensions().1])
            || !inside(point.target, [0, 0, target.dimensions().0, target.dimensions().1])
        {
            return Err(LocalRankError::Invalid);
        }
        for prior in &points[..i] {
            if cancel() {
                return Err(LocalRankError::Cancelled);
            }
            if distance(prior.source, point.source) <= p.minimum_point_separation
                || distance(prior.target, point.target) <= p.minimum_point_separation
            {
                return Err(LocalRankError::Invalid);
            }
        }
        if inside(point.source, source_region) && inside(point.target, target_region) {
            let forward = mesh
                .map_source(point.source, policy.maximum_triangles, &cancel)
                .map_err(error)?;
            let reverse = mesh
                .map_target(point.target, policy.maximum_triangles, &cancel)
                .map_err(error)?;
            if forward.is_some_and(|v| distance(v, point.target) <= p.target_tolerance)
                && reverse.is_some_and(|v| distance(v, point.source) <= p.source_tolerance)
            {
                witnesses += 1
            }
        }
    }
    if witnesses < p.minimum_witnesses {
        return Err(LocalRankError::InsufficientGeometry);
    }
    let compare = |a: &LinearRgbaView<'_>, b: &LinearRgbaView<'_>, direction, ar, br, context| {
        let grid = build_mesh_grid(
            mesh,
            context,
            direction,
            MeshGridPolicy {
                maximum_triangles: policy.maximum_triangles,
                maximum_sites: p.rank.maximum_sites,
                maximum_face_sites: policy.maximum_face_sites,
            },
            budget,
            &cancel,
        )
        .map_err(error)?;
        compare_mesh_rank_region(a, b, &grid, ar, br, p.rank, p.filter_radius, budget, &cancel)
            .map_err(LocalRankError::from)
    };
    let forward = compare(
        source,
        target,
        MeshDirection::TargetToSource,
        source_region,
        target_region,
        tr,
    )?;
    let reverse = compare(
        target,
        source,
        MeshDirection::SourceToTarget,
        target_region,
        source_region,
        sr,
    )?;
    if cancel() {
        return Err(LocalRankError::Cancelled);
    }
    let passes = |e: crate::rank_region::RankRegionEvidence| {
        e.valid_sites as f64 / e.sites as f64 >= p.minimum_coverage
            && e.agreeing_pairs as f64 / e.informative_pairs as f64 >= p.minimum_agreement
    };
    Ok(LocalRankEvidence {
        regional_witnesses: witnesses,
        forward,
        reverse,
        supported: passes(forward) && passes(reverse),
    })
}
