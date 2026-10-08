//! Whole-atlas ordinal evidence with managed summed windows; no copy decision.
use crate::{
    linear::LinearRgbaView,
    mesh_grid::MeshGrid,
    rank_region::{RankRegionError, RankRegionEvidence, RankRegionPolicy, interpolated, luminance},
};
use rrrah_core::MemoryBudget;
#[derive(Clone, Copy, Default)]
struct Sum {
    source: f64,
    target: f64,
    invalid: u64,
}
/// Compare integer grid centers and eight neighbors after square-window means.
/// A window with any missing mapped sample is excluded. Work includes at most
/// five image reads per atlas site, with bounded constant-time window queries.
/// Prefix sums change floating-point accumulation order from direct windows.
pub fn compare_mesh_rank(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    grid: &MeshGrid,
    policy: RankRegionPolicy,
    filter_radius: u32,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<RankRegionEvidence, RankRegionError> {
    let (sw, sh) = source.dimensions();
    compare_mesh_rank_region(
        source,
        target,
        grid,
        [0, 0, sw, sh],
        grid.region(),
        policy,
        filter_radius,
        budget,
        cancel,
    )
}

/// Compare selected target centers, clipping mapped centers to a source region.
/// Neighbor/window samples still use the supplied atlas context outside those
/// regions. Missing context excludes a sample. All participating atlas pixels
/// are validated, so unsupported pixels outside selected centers can refuse the
/// operation. Sites count selected centers; read/work/memory admission charges
/// the whole atlas. This produces ordinal evidence only, never a copy verdict.
pub fn compare_mesh_rank_region(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    grid: &MeshGrid,
    source_region: [u32; 4],
    target_region: [u32; 4],
    policy: RankRegionPolicy,
    filter_radius: u32,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<RankRegionEvidence, RankRegionError> {
    compare_region(
        source,
        target,
        grid,
        source_region,
        target_region,
        policy,
        filter_radius,
        budget,
        true,
        cancel,
    )
}

pub(crate) fn compare_mesh_rank_region_counts(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    grid: &MeshGrid,
    source_region: [u32; 4],
    target_region: [u32; 4],
    policy: RankRegionPolicy,
    filter_radius: u32,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<RankRegionEvidence, RankRegionError> {
    compare_region(
        source,
        target,
        grid,
        source_region,
        target_region,
        policy,
        filter_radius,
        budget,
        false,
        cancel,
    )
}

fn compare_region(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    grid: &MeshGrid,
    source_region: [u32; 4],
    target_region: [u32; 4],
    policy: RankRegionPolicy,
    filter_radius: u32,
    budget: &MemoryBudget,
    require_information: bool,
    cancel: impl Fn() -> bool,
) -> Result<RankRegionEvidence, RankRegionError> {
    if cancel() {
        return Err(RankRegionError::Cancelled);
    }
    let [ox, oy, w, h] = grid.region();
    let (tw, th) = target.dimensions();
    let valid_rectangle = |r: [u32; 4], size: (u32, u32)| {
        r[2] > 0
            && r[3] > 0
            && r[0].checked_add(r[2]).is_some_and(|v| v <= size.0)
            && r[1].checked_add(r[3]).is_some_and(|v| v <= size.1)
    };
    if !valid_rectangle(source_region, source.dimensions())
        || !valid_rectangle(target_region, (tw, th))
        || target_region[0] < ox
        || target_region[1] < oy
        || target_region[0]
            .checked_add(target_region[2])
            .zip(ox.checked_add(w))
            .is_none_or(|(a, b)| a > b)
        || target_region[1]
            .checked_add(target_region[3])
            .zip(oy.checked_add(h))
            .is_none_or(|(a, b)| a > b)
        || ox.checked_add(w).is_none_or(|v| v > tw)
        || oy.checked_add(h).is_none_or(|v| v > th)
        || policy.radius == 0
        || !policy.minimum_contrast.is_finite()
        || policy.minimum_contrast <= 0.
        || policy.minimum_pairs == 0
    {
        return Err(RankRegionError::Invalid);
    }
    let sites = u64::from(w) * u64::from(h);
    let reads = sites.checked_mul(5).ok_or(RankRegionError::Budget)?;
    if sites > policy.maximum_sites || reads > policy.maximum_pixel_reads {
        return Err(RankRegionError::Budget);
    }
    let side = filter_radius
        .checked_mul(2)
        .and_then(|v| v.checked_add(1))
        .ok_or(RankRegionError::Budget)?;
    let stride = u64::from(w) + 1;
    let entries = stride
        .checked_mul(u64::from(h) + 1)
        .ok_or(RankRegionError::Budget)?;
    let bytes = entries
        .checked_mul(std::mem::size_of::<Sum>() as u64)
        .ok_or(RankRegionError::Budget)?;
    let count = usize::try_from(entries).map_err(|_| RankRegionError::Budget)?;
    let stride = usize::try_from(stride).map_err(|_| RankRegionError::Budget)?;
    let credit = budget.try_reserve(bytes).map_err(|_| RankRegionError::Budget)?;
    let mut sums = Vec::new();
    sums.try_reserve_exact(count)
        .map_err(|_| RankRegionError::Budget)?;
    if sums.capacity() > count {
        return Err(RankRegionError::Budget);
    }
    for _ in 0..count {
        if cancel() {
            return Err(RankRegionError::Cancelled);
        }
        sums.push(Sum::default());
    }
    for y in 0..h {
        let mut row = Sum::default();
        for x in 0..w {
            if cancel() {
                return Err(RankRegionError::Cancelled);
            }
            if let Some(p) = grid.coordinate(ox + x, oy + y) {
                if let Some(v) = interpolated(source, p)? {
                    row.source += v;
                    row.target += luminance(target, ox + x, oy + y)?;
                } else {
                    row.invalid += 1;
                }
            } else {
                row.invalid += 1;
            }
            let i = (y as usize + 1) * stride + x as usize + 1;
            let above = sums[i - stride];
            sums[i] = Sum {
                source: row.source + above.source,
                target: row.target + above.target,
                invalid: row.invalid + above.invalid,
            };
        }
    }
    let sample = |x: i64, y: i64| -> Option<[f64; 2]> {
        let r = i64::from(filter_radius);
        let x0 = x - r;
        let y0 = y - r;
        let x1 = x + r + 1;
        let y1 = y + r + 1;
        if x0 < 0 || y0 < 0 || x1 > i64::from(w) || y1 > i64::from(h) {
            return None;
        }
        let a = sums[y1 as usize * stride + x1 as usize];
        let b = sums[y0 as usize * stride + x1 as usize];
        let c = sums[y1 as usize * stride + x0 as usize];
        let d = sums[y0 as usize * stride + x0 as usize];
        // Signed arithmetic avoids unsigned intermediate subtraction underflow.
        if i128::from(a.invalid) - i128::from(b.invalid) - i128::from(c.invalid) + i128::from(d.invalid) != 0
        {
            return None;
        }
        let divisor = f64::from(side) * f64::from(side);
        Some([
            (a.source - b.source - c.source + d.source) / divisor,
            (a.target - b.target - c.target + d.target) / divisor,
        ])
    };
    let radius = i64::from(policy.radius);
    let [sx, sy, sw, sh] = source_region;
    let [cx, cy, cw, ch] = target_region;
    let mut evidence = RankRegionEvidence {
        sites: u64::from(cw) * u64::from(ch),
        valid_sites: 0,
        informative_pairs: 0,
        agreeing_pairs: 0,
        pixel_reads: reads,
    };
    for y in cy - oy..cy - oy + ch {
        for x in cx - ox..cx - ox + cw {
            if cancel() {
                return Err(RankRegionError::Cancelled);
            }
            let Some(p) = grid.coordinate(ox + x, oy + y) else {
                continue;
            };
            if p[0] < f64::from(sx)
                || p[1] < f64::from(sy)
                || p[0] >= f64::from(sx + sw)
                || p[1] >= f64::from(sy + sh)
            {
                continue;
            }
            let Some(center) = sample(i64::from(x), i64::from(y)) else {
                continue;
            };
            let mut neighbors = [[0.; 2]; 8];
            let mut valid = true;
            for (i, (dx, dy)) in [
                (-radius, -radius),
                (0, -radius),
                (radius, -radius),
                (-radius, 0),
                (radius, 0),
                (-radius, radius),
                (0, radius),
                (radius, radius),
            ]
            .into_iter()
            .enumerate()
            {
                if cancel() {
                    return Err(RankRegionError::Cancelled);
                }
                if let Some(p) = sample(i64::from(x) + dx, i64::from(y) + dy) {
                    neighbors[i] = p;
                } else {
                    valid = false;
                    break;
                }
            }
            if !valid {
                continue;
            }
            evidence.valid_sites += 1;
            for p in neighbors {
                let a = p[0] - center[0];
                let b = p[1] - center[1];
                if a.abs() >= policy.minimum_contrast && b.abs() >= policy.minimum_contrast {
                    evidence.informative_pairs += 1;
                    if (a > 0.) == (b > 0.) {
                        evidence.agreeing_pairs += 1;
                    }
                }
            }
        }
    }
    if cancel() {
        return Err(RankRegionError::Cancelled);
    }
    drop(sums);
    drop(credit);
    if require_information && evidence.informative_pairs < policy.minimum_pairs {
        return Err(RankRegionError::Uninformative);
    }
    Ok(evidence)
}
