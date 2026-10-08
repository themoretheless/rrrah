//! Pre-scoring geometric anchor windows; finite local support, never file identity.
use crate::{
    geometry::{Correspondence, ProjectiveTransform},
    linear::LinearRgbaView,
    local_rank::{LocalRankError, LocalRankPolicy},
    mesh_grid::{ProjectiveGridPolicy, build_projective_grid},
    mesh_rank::compare_mesh_rank_region_counts,
    piecewise_warp::PiecewiseError,
    rank_region::RankRegionEvidence,
};
use rrrah_core::MemoryBudget;
#[derive(Debug, Clone, Copy)]
pub struct AnchorRankPolicy {
    pub local: LocalRankPolicy,
    pub window_radius: u32,
    /// Prepaid point aliases and both greedy window-overlap selections.
    pub maximum_selection_checks: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorRankEvidence {
    pub forward_anchors: usize,
    pub reverse_anchors: usize,
    pub forward: RankRegionEvidence,
    pub reverse: RankRegionEvidence,
    pub supported: bool,
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn window(p: [f64; 2], d: (u32, u32), radius: u32, size: u32) -> Option<[u32; 4]> {
    let x = (p[0].floor() as u32).checked_sub(radius)?;
    let y = (p[1].floor() as u32).checked_sub(radius)?;
    (x.checked_add(size)? <= d.0 && y.checked_add(size)? <= d.1).then_some([x, y, size, size])
}
fn overlaps(a: [u32; 4], b: [u32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}
fn error(e: PiecewiseError) -> LocalRankError {
    match e {
        PiecewiseError::Invalid => LocalRankError::Invalid,
        PiecewiseError::Budget => LocalRankError::Budget,
        PiecewiseError::Cancelled => LocalRankError::Cancelled,
    }
}
pub(crate) fn validate_anchor_rank_policy(policy: AnchorRankPolicy) -> Result<(), LocalRankError> {
    let p = policy.local;
    let fraction = |v: f64| v.is_finite() && v > 0. && v <= 1.;
    let positive = |v: f64| v.is_finite() && v > 0.;
    if p.minimum_witnesses < 4
        || p.maximum_points < p.minimum_witnesses
        || !fraction(p.minimum_coverage)
        || !fraction(p.minimum_agreement)
        || !fraction(p.rank.minimum_contrast)
        || !positive(p.minimum_point_separation)
        || !positive(p.source_tolerance)
        || !positive(p.target_tolerance)
        || p.rank.radius == 0
        || p.rank.minimum_pairs == 0
    {
        return Err(LocalRankError::Invalid);
    }
    Ok(())
}
/// Original caller-provided descriptor-qualified points are all retained and
/// checked for aliases. Model/full-image domains and both residuals qualify
/// anchors. Greedy disjoint integer center windows use caller order before any
/// pixel reads; neighbor/filter contexts can overlap, so this is not holdout.
/// Minimum pairs apply to the aggregate, including zero-information windows.
/// Both worst-case expanded contexts and selection work are prepaid. Temporary
/// selection/atlas/prefix storage is managed and freed before every return.
#[allow(clippy::too_many_arguments)]
pub fn compare_anchor_rank(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    model: ProjectiveTransform,
    points: &[Correspondence],
    policy: AnchorRankPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AnchorRankEvidence, LocalRankError> {
    if cancel() {
        return Err(LocalRankError::Cancelled);
    }
    validate_anchor_rank_policy(policy)?;
    let p = policy.local;
    if !crate::geometry::projective_domain_eligible(model, source.dimensions(), target.dimensions()) {
        return Err(LocalRankError::Invalid);
    }
    let n = u64::try_from(points.len()).map_err(|_| LocalRankError::Budget)?;
    let pairs = n
        .checked_mul(n.saturating_sub(1))
        .and_then(|v| v.checked_div(2))
        .ok_or(LocalRankError::Budget)?;
    let point_checks = pairs.checked_add(n).ok_or(LocalRankError::Budget)?;
    let selection = pairs
        .checked_mul(3)
        .and_then(|v| v.checked_add(n))
        .ok_or(LocalRankError::Budget)?;
    let size = policy
        .window_radius
        .checked_mul(2)
        .and_then(|v| v.checked_add(1))
        .ok_or(LocalRankError::Budget)?;
    let pad = p
        .rank
        .radius
        .checked_add(p.filter_radius)
        .ok_or(LocalRankError::Budget)?;
    let _maximum_comparisons = n
        .checked_mul(2)
        .and_then(|v| v.checked_mul(u64::from(size)))
        .and_then(|v| v.checked_mul(u64::from(size)))
        .and_then(|v| v.checked_mul(8))
        .ok_or(LocalRankError::Budget)?;
    let extent = u64::from(size)
        .checked_add(u64::from(pad) * 2)
        .ok_or(LocalRankError::Budget)?;
    let sites = n
        .checked_mul(2)
        .and_then(|v| v.checked_mul(extent))
        .and_then(|v| v.checked_mul(extent))
        .ok_or(LocalRankError::Budget)?;
    let reads = sites.checked_mul(5).ok_or(LocalRankError::Budget)?;
    if points.len() > p.maximum_points
        || point_checks > p.maximum_point_checks
        || selection > policy.maximum_selection_checks
        || sites > p.rank.maximum_sites
        || reads > p.rank.maximum_pixel_reads
    {
        return Err(LocalRankError::Budget);
    }
    let inverse = model.inverse().map_err(|_| LocalRankError::Invalid)?;
    let bytes = n.checked_mul(16).ok_or(LocalRankError::Budget)?;
    let ac = budget.try_reserve(bytes).map_err(|_| LocalRankError::Budget)?;
    let bc = budget.try_reserve(bytes).map_err(|_| LocalRankError::Budget)?;
    let mut aw = Vec::new();
    let mut bw = Vec::new();
    aw.try_reserve_exact(points.len())
        .map_err(|_| LocalRankError::Budget)?;
    bw.try_reserve_exact(points.len())
        .map_err(|_| LocalRankError::Budget)?;
    for (i, point) in points.iter().enumerate() {
        if cancel() {
            return Err(LocalRankError::Cancelled);
        }
        for (xy, d) in [
            (point.source, source.dimensions()),
            (point.target, target.dimensions()),
        ] {
            if xy.iter().any(|v| !v.is_finite() || *v < 0.)
                || xy[0] >= f64::from(d.0)
                || xy[1] >= f64::from(d.1)
            {
                return Err(LocalRankError::Invalid);
            }
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
        if !model
            .apply(point.source)
            .is_some_and(|v| distance(v, point.target) <= p.target_tolerance)
            || !inverse
                .apply(point.target)
                .is_some_and(|v| distance(v, point.source) <= p.source_tolerance)
        {
            continue;
        }
        for (xy, d, windows) in [
            (point.source, source.dimensions(), &mut aw),
            (point.target, target.dimensions(), &mut bw),
        ] {
            if let Some(rect) = window(xy, d, policy.window_radius, size) {
                let mut conflict = false;
                for prior in windows.iter() {
                    if cancel() {
                        return Err(LocalRankError::Cancelled);
                    }
                    if overlaps(rect, *prior) {
                        conflict = true;
                        break;
                    }
                }
                if !conflict {
                    windows.push(rect)
                }
            }
        }
    }
    if aw.len() < p.minimum_witnesses || bw.len() < p.minimum_witnesses {
        return Err(LocalRankError::InsufficientGeometry);
    }
    let aw = ac.try_adopt(aw).map_err(|_| LocalRankError::Budget)?;
    let bw = bc.try_adopt(bw).map_err(|_| LocalRankError::Budget)?;
    let compare = |a: &LinearRgbaView<'_>,
                   b: &LinearRgbaView<'_>,
                   m,
                   windows: &[[u32; 4]]|
     -> Result<RankRegionEvidence, LocalRankError> {
        let mut total = RankRegionEvidence {
            sites: 0,
            valid_sites: 0,
            informative_pairs: 0,
            agreeing_pairs: 0,
            pixel_reads: 0,
        };
        for &rect in windows {
            if cancel() {
                return Err(LocalRankError::Cancelled);
            }
            let x = rect[0].saturating_sub(pad);
            let y = rect[1].saturating_sub(pad);
            let right = (rect[0] + rect[2]).saturating_add(pad).min(b.dimensions().0);
            let bottom = (rect[1] + rect[3]).saturating_add(pad).min(b.dimensions().1);
            let grid = build_projective_grid(
                m,
                a.dimensions(),
                b.dimensions(),
                [x, y, right - x, bottom - y],
                ProjectiveGridPolicy {
                    maximum_sites: p.rank.maximum_sites,
                },
                budget,
                &cancel,
            )
            .map_err(error)?;
            let e = compare_mesh_rank_region_counts(
                a,
                b,
                &grid,
                [0, 0, a.dimensions().0, a.dimensions().1],
                rect,
                p.rank,
                p.filter_radius,
                budget,
                &cancel,
            )?;
            total.sites += e.sites;
            total.valid_sites += e.valid_sites;
            total.informative_pairs += e.informative_pairs;
            total.agreeing_pairs += e.agreeing_pairs;
            total.pixel_reads += e.pixel_reads;
        }
        Ok(total)
    };
    let forward = compare(source, target, model, &bw)?;
    let reverse = compare(target, source, inverse, &aw)?;
    if cancel() {
        return Err(LocalRankError::Cancelled);
    }
    let passes = |e: RankRegionEvidence| {
        e.informative_pairs >= p.rank.minimum_pairs
            && e.valid_sites as f64 / e.sites as f64 >= p.minimum_coverage
            && e.agreeing_pairs as f64 / e.informative_pairs as f64 >= p.minimum_agreement
    };
    Ok(AnchorRankEvidence {
        forward_anchors: bw.len(),
        reverse_anchors: aw.len(),
        forward,
        reverse,
        supported: passes(forward) && passes(reverse),
    })
}
