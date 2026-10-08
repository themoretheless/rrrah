//! Explicit local geometric/ordinal support; never whole-file identity.
//! Descriptor provenance is the caller's responsibility. This policy is opt-in
//! and requires calibration against unrelated images before copy admission.
use crate::{
    geometry::{Correspondence, ProjectiveTransform},
    linear::LinearRgbaView,
    rank_region::{RankRegionError, RankRegionEvidence, RankRegionPolicy, compare_filtered_rank_region},
};

#[derive(Debug, Clone, Copy)]
pub struct LocalRankPolicy {
    pub rank: RankRegionPolicy,
    pub filter_radius: u32,
    pub minimum_witnesses: usize,
    pub maximum_points: usize,
    /// Includes pairwise distinctness checks, admitted before examining points.
    pub maximum_point_checks: u64,
    pub minimum_point_separation: f64,
    pub target_tolerance: f64,
    pub source_tolerance: f64,
    pub minimum_coverage: f64,
    pub minimum_agreement: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalRankEvidence {
    pub regional_witnesses: usize,
    pub forward: RankRegionEvidence,
    pub reverse: RankRegionEvidence,
    /// Local support only; does not classify either complete image as a copy.
    pub supported: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LocalRankError {
    #[error("invalid local rank geometry or policy")]
    Invalid,
    #[error("local rank aggregate work budget exceeded")]
    Budget,
    #[error("local rank cancelled")]
    Cancelled,
    #[error("insufficient regional geometric witnesses")]
    InsufficientGeometry,
    #[error(transparent)]
    Rank(#[from] RankRegionError),
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
/// Require original one-to-one points, full-domain eligible supplied geometry,
/// regional witnesses in both rectangles, forward AND inverse residuals, and
/// bidirectional rank support. All point pairs are checked for duplicate/near
/// source or target positions, including points outside the chosen rectangles.
/// Site/read budgets are aggregate across both pixel directions; geometric
/// checks have a separate prepaid limit. No allocation or partial success.
#[allow(clippy::too_many_arguments)]
pub fn compare_local_rank_region(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    model: ProjectiveTransform,
    points: &[Correspondence],
    source_region: [u32; 4],
    target_region: [u32; 4],
    policy: LocalRankPolicy,
    cancel: impl Fn() -> bool,
) -> Result<LocalRankEvidence, LocalRankError> {
    compare_local_rank_region_impl(
        source,
        target,
        model,
        points,
        source_region,
        target_region,
        policy,
        None,
        cancel,
    )
}
/// Managed cached equivalent for opaque display-range participating pixels.
/// Site/read limits charge both complete expanded contexts, not selected centers.
/// Context expansion is rank radius + filter radius. Pixels outside selected
/// centers in that context can conservatively refuse unsupported alpha/HDR.
/// Scratch and atlas credits are released before returning evidence or refusal.
#[allow(clippy::too_many_arguments)]
pub fn compare_local_rank_region_cached(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    model: ProjectiveTransform,
    points: &[Correspondence],
    source_region: [u32; 4],
    target_region: [u32; 4],
    policy: LocalRankPolicy,
    budget: &rrrah_core::MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<LocalRankEvidence, LocalRankError> {
    compare_local_rank_region_impl(
        source,
        target,
        model,
        points,
        source_region,
        target_region,
        policy,
        Some(budget),
        cancel,
    )
}
#[allow(clippy::too_many_arguments)]
fn compare_local_rank_region_impl(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    model: ProjectiveTransform,
    points: &[Correspondence],
    source_region: [u32; 4],
    target_region: [u32; 4],
    policy: LocalRankPolicy,
    cache: Option<&rrrah_core::MemoryBudget>,
    cancel: impl Fn() -> bool,
) -> Result<LocalRankEvidence, LocalRankError> {
    if cancel() {
        return Err(LocalRankError::Cancelled);
    }
    let finite_positive = |x: f64| x.is_finite() && x > 0.;
    let fraction = |x: f64| x.is_finite() && x > 0. && x <= 1.;
    let rectangle = |r: [u32; 4], d: (u32, u32)| {
        r[2] > 0
            && r[3] > 0
            && r[0].checked_add(r[2]).is_some_and(|v| v <= d.0)
            && r[1].checked_add(r[3]).is_some_and(|v| v <= d.1)
    };
    if policy.minimum_witnesses < 4
        || policy.maximum_points < policy.minimum_witnesses
        || !finite_positive(policy.minimum_point_separation)
        || !finite_positive(policy.target_tolerance)
        || !finite_positive(policy.source_tolerance)
        || !fraction(policy.minimum_coverage)
        || !fraction(policy.minimum_agreement)
        || !rectangle(source_region, source.dimensions())
        || !rectangle(target_region, target.dimensions())
        || policy.rank.radius == 0
        || !fraction(policy.rank.minimum_contrast)
        || policy.rank.minimum_pairs == 0
    {
        return Err(LocalRankError::Invalid);
    }
    if points.len() > policy.maximum_points {
        return Err(LocalRankError::Budget);
    }
    let n = u64::try_from(points.len()).map_err(|_| LocalRankError::Budget)?;
    let checks = n
        .checked_mul(n.saturating_sub(1))
        .and_then(|v| v.checked_div(2))
        .and_then(|v| v.checked_add(n))
        .ok_or(LocalRankError::Budget)?;
    let context = |r: [u32; 4], d: (u32, u32)| -> Result<[u32; 4], LocalRankError> {
        let pad = policy
            .rank
            .radius
            .checked_add(policy.filter_radius)
            .ok_or(LocalRankError::Budget)?;
        let x = r[0].saturating_sub(pad);
        let y = r[1].saturating_sub(pad);
        let right = (r[0] + r[2]).saturating_add(pad).min(d.0);
        let bottom = (r[1] + r[3]).saturating_add(pad).min(d.1);
        Ok([x, y, right - x, bottom - y])
    };
    let (sr, tr) = if cache.is_some() {
        (
            context(source_region, source.dimensions())?,
            context(target_region, target.dimensions())?,
        )
    } else {
        (source_region, target_region)
    };
    let sites = (u64::from(sr[2]) * u64::from(sr[3]))
        .checked_add(u64::from(tr[2]) * u64::from(tr[3]))
        .ok_or(LocalRankError::Budget)?;
    let reads = if cache.is_some() {
        sites.checked_mul(5).ok_or(LocalRankError::Budget)?
    } else {
        let window = u64::from(policy.filter_radius)
            .checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .ok_or(LocalRankError::Budget)?;
        sites
            .checked_mul(45)
            .and_then(|v| v.checked_mul(window))
            .and_then(|v| v.checked_mul(window))
            .ok_or(LocalRankError::Budget)?
    };
    if checks > policy.maximum_point_checks
        || sites > policy.rank.maximum_sites
        || reads > policy.rank.maximum_pixel_reads
    {
        return Err(LocalRankError::Budget);
    }
    if !crate::geometry::projective_domain_eligible(model, source.dimensions(), target.dimensions()) {
        return Err(LocalRankError::Invalid);
    }
    let inverse = model.inverse().map_err(|_| LocalRankError::Invalid)?;
    let mut witnesses = 0;
    for (i, p) in points.iter().enumerate() {
        if cancel() {
            return Err(LocalRankError::Cancelled);
        }
        if !inside(p.source, [0, 0, source.dimensions().0, source.dimensions().1])
            || !inside(p.target, [0, 0, target.dimensions().0, target.dimensions().1])
            || p.source.iter().chain(p.target.iter()).any(|v| !v.is_finite())
        {
            return Err(LocalRankError::Invalid);
        }
        for q in &points[..i] {
            if cancel() {
                return Err(LocalRankError::Cancelled);
            }
            if distance(p.source, q.source) <= policy.minimum_point_separation
                || distance(p.target, q.target) <= policy.minimum_point_separation
            {
                return Err(LocalRankError::Invalid);
            }
        }
        if inside(p.source, source_region)
            && inside(p.target, target_region)
            && model
                .apply(p.source)
                .is_some_and(|v| distance(v, p.target) <= policy.target_tolerance)
            && inverse
                .apply(p.target)
                .is_some_and(|v| distance(v, p.source) <= policy.source_tolerance)
        {
            witnesses += 1
        }
    }
    if witnesses < policy.minimum_witnesses {
        return Err(LocalRankError::InsufficientGeometry);
    }
    let (forward, reverse) = if let Some(budget) = cache {
        use crate::{
            mesh_grid::{ProjectiveGridPolicy, build_projective_grid},
            mesh_rank::compare_mesh_rank_region,
            piecewise_warp::PiecewiseError,
        };
        let convert = |e| match e {
            PiecewiseError::Invalid => LocalRankError::Invalid,
            PiecewiseError::Budget => LocalRankError::Budget,
            PiecewiseError::Cancelled => LocalRankError::Cancelled,
        };
        let compare = |a: &LinearRgbaView<'_>, b: &LinearRgbaView<'_>, m, ar, br, region| {
            let grid = build_projective_grid(
                m,
                a.dimensions(),
                b.dimensions(),
                region,
                ProjectiveGridPolicy {
                    maximum_sites: policy.rank.maximum_sites,
                },
                budget,
                &cancel,
            )
            .map_err(convert)?;
            compare_mesh_rank_region(
                a,
                b,
                &grid,
                ar,
                br,
                policy.rank,
                policy.filter_radius,
                budget,
                &cancel,
            )
            .map_err(LocalRankError::from)
        };
        (
            compare(source, target, model, source_region, target_region, tr)?,
            compare(target, source, inverse, target_region, source_region, sr)?,
        )
    } else {
        let forward = compare_filtered_rank_region(
            source,
            target,
            model,
            source_region,
            target_region,
            policy.rank,
            policy.filter_radius,
            &cancel,
        )?;
        let reverse = compare_filtered_rank_region(
            target,
            source,
            inverse,
            target_region,
            source_region,
            policy.rank,
            policy.filter_radius,
            &cancel,
        )?;
        (forward, reverse)
    };
    if cancel() {
        return Err(LocalRankError::Cancelled);
    }
    let passes = |e: RankRegionEvidence| {
        e.valid_sites as f64 / e.sites as f64 >= policy.minimum_coverage
            && e.agreeing_pairs as f64 / e.informative_pairs as f64 >= policy.minimum_agreement
    };
    Ok(LocalRankEvidence {
        regional_witnesses: witnesses,
        forward,
        reverse,
        supported: passes(forward) && passes(reverse),
    })
}
