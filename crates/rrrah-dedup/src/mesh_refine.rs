//! Bounded local source-translation proposals under a supplied mesh atlas.
//! Fixed 31-pixel windows; disjoint comparison centers have overlapping taps.
//! Geometry conformity and independent copy confirmation remain separate gates.
use crate::{
    linear::LinearRgbaView,
    mesh_grid::MeshGrid,
    rank_region::{RankRegionError, interpolated, luminance},
};
use rrrah_core::{MemoryBudget, SharedBuffer};
#[derive(Debug, Clone, Copy)]
pub struct MeshRefinePolicy {
    pub offset_radius: u32,
    pub offset_step: u32,
    pub minimum_contrast: f64,
    pub minimum_pairs: u64,
    pub minimum_training_fraction: f64,
    pub minimum_validation_fraction: f64,
    pub maximum_seeds: usize,
    pub maximum_offsets: u64,
    pub maximum_pixel_reads: u64,
}
#[derive(Debug, Clone, Copy)]
pub struct TranslationEvidence {
    pub initial_source: [f64; 2],
    pub source: [f64; 2],
    pub offset: [i32; 2],
    pub training_pairs: u64,
    pub training_agreeing: u64,
    pub validation_pairs: u64,
    pub validation_agreeing: u64,
    pub eligible_proposal: bool,
}
#[derive(Debug, Clone, Copy)]
pub enum SeedRefinement {
    Unavailable,
    Uninformative,
    Scored(TranslationEvidence),
}
#[derive(Clone, Copy, Default)]
struct Tap {
    coordinate: [f64; 2],
    target: f64,
    valid: bool,
}
#[derive(Clone, Copy, Default)]
struct Prefix {
    source: f64,
    target: f64,
    invalid: u64,
}
const WIDTH: usize = 31;
const STRIDE: usize = 32;
fn sample(sums: &[Prefix], x: i32, y: i32) -> Option<[f64; 2]> {
    // Fixed radius3 window, local center coordinates include the15-pixel pad.
    let x0 = (x - 3) as usize;
    let y0 = (y - 3) as usize;
    let x1 = (x + 4) as usize;
    let y1 = (y + 4) as usize;
    let a = sums[y1 * STRIDE + x1];
    let b = sums[y0 * STRIDE + x1];
    let c = sums[y1 * STRIDE + x0];
    let d = sums[y0 * STRIDE + x0];
    if i128::from(a.invalid) - i128::from(b.invalid) - i128::from(c.invalid) + i128::from(d.invalid) != 0 {
        return None;
    }
    Some([
        (a.source - b.source - c.source + d.source) / 49.,
        (a.target - b.target - c.target + d.target) / 49.,
    ])
}
fn counts(sums: &[Prefix], contrast: f64) -> Option<[u64; 4]> {
    let mut result = [0; 4];
    for y in 0..9 {
        for x in 0..9 {
            let cx = 7 + x * 2;
            let cy = 7 + y * 2;
            let center = sample(sums, cx, cy)?;
            let validation = (x + y) % 2 != 0;
            let index = if validation { 2 } else { 0 };
            for (dx, dy) in [(-4, 0), (4, 0), (0, -4), (0, 4)] {
                let p = sample(sums, cx + dx, cy + dy)?;
                let a = p[0] - center[0];
                let b = p[1] - center[1];
                // Target-only informative support is invariant across candidate offsets.
                // Weak source contrast is disagreement, never a dropped comparison.
                if b.abs() >= contrast {
                    result[index] += 1;
                    if a.abs() >= contrast && (a > 0.) == (b > 0.) {
                        result[index + 1] += 1;
                    }
                }
            }
        }
    }
    Some(result)
}
/// Output order equals seed order. Candidate support uses the worst displacement
/// bounds for every mapped tap, making all candidates share the same windows.
/// Source shifts are selected solely on training agreement, with stable first
/// candidate tie breaks in ascending(y,x). No topology/copy admission is inferred.
pub fn propose_mesh_translations(
    source: &LinearRgbaView<'_>,
    target: &LinearRgbaView<'_>,
    grid: &MeshGrid,
    seeds: &[[u32; 2]],
    policy: MeshRefinePolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<SharedBuffer<SeedRefinement>, RankRegionError> {
    if cancel() {
        return Err(RankRegionError::Cancelled);
    }
    if policy.offset_step == 0
        || policy.offset_radius % policy.offset_step != 0
        || policy.offset_radius > i32::MAX as u32
        || !policy.minimum_contrast.is_finite()
        || policy.minimum_contrast <= 0.
        || policy.minimum_pairs == 0
        || [
            policy.minimum_training_fraction,
            policy.minimum_validation_fraction,
        ]
        .iter()
        .any(|v| !v.is_finite() || !(0.0..=1.).contains(v))
    {
        return Err(RankRegionError::Invalid);
    }
    if seeds.len() > policy.maximum_seeds {
        return Err(RankRegionError::Budget);
    }
    let side = u64::from(policy.offset_radius)
        .checked_mul(2)
        .and_then(|v| v.checked_div(u64::from(policy.offset_step)))
        .and_then(|v| v.checked_add(1))
        .ok_or(RankRegionError::Budget)?;
    let offsets = side.checked_mul(side).ok_or(RankRegionError::Budget)?;
    if offsets > policy.maximum_offsets {
        return Err(RankRegionError::Budget);
    }
    let per_seed = offsets
        .checked_mul(4)
        .and_then(|v| v.checked_add(1))
        .and_then(|v| v.checked_mul((WIDTH * WIDTH) as u64))
        .ok_or(RankRegionError::Budget)?;
    let reads = per_seed
        .checked_mul(seeds.len() as u64)
        .ok_or(RankRegionError::Budget)?;
    if reads > policy.maximum_pixel_reads {
        return Err(RankRegionError::Budget);
    }
    let output_bytes = seeds
        .len()
        .checked_mul(std::mem::size_of::<SeedRefinement>())
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(RankRegionError::Budget)?;
    let scratch_bytes =
        (WIDTH * WIDTH * std::mem::size_of::<Tap>() + STRIDE * STRIDE * std::mem::size_of::<Prefix>()) as u64;
    let output_credit = budget
        .try_reserve(output_bytes)
        .map_err(|_| RankRegionError::Budget)?;
    let scratch_credit = budget
        .try_reserve(scratch_bytes)
        .map_err(|_| RankRegionError::Budget)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(seeds.len())
        .map_err(|_| RankRegionError::Budget)?;
    let mut taps = Vec::new();
    taps.try_reserve_exact(WIDTH * WIDTH)
        .map_err(|_| RankRegionError::Budget)?;
    let mut sums = Vec::new();
    sums.try_reserve_exact(STRIDE * STRIDE)
        .map_err(|_| RankRegionError::Budget)?;
    if output.capacity() > seeds.len() || taps.capacity() > WIDTH * WIDTH || sums.capacity() > STRIDE * STRIDE
    {
        return Err(RankRegionError::Budget);
    }
    for _ in 0..WIDTH * WIDTH {
        if cancel() {
            return Err(RankRegionError::Cancelled);
        }
        taps.push(Tap::default());
    }
    for _ in 0..STRIDE * STRIDE {
        if cancel() {
            return Err(RankRegionError::Cancelled);
        }
        sums.push(Prefix::default());
    }
    let (sw, sh) = source.dimensions();
    let (tw, th) = target.dimensions();
    let radius = f64::from(policy.offset_radius);
    for &[x, y] in seeds {
        if cancel() {
            return Err(RankRegionError::Cancelled);
        }
        if x < 15
            || y < 15
            || x.checked_add(15).is_none_or(|v| v >= tw)
            || y.checked_add(15).is_none_or(|v| v >= th)
        {
            output.push(SeedRefinement::Unavailable);
            continue;
        }
        let Some(initial) = grid.coordinate(x, y) else {
            output.push(SeedRefinement::Unavailable);
            continue;
        };
        for dy in 0..WIDTH {
            for dx in 0..WIDTH {
                if cancel() {
                    return Err(RankRegionError::Cancelled);
                }
                let px = x - 15 + dx as u32;
                let py = y - 15 + dy as u32;
                let coordinate = grid.coordinate(px, py);
                let valid = coordinate.is_some_and(|p| {
                    p[0] >= radius
                        && p[1] >= radius
                        && p[0] + radius + 1. < f64::from(sw)
                        && p[1] + radius + 1. < f64::from(sh)
                });
                taps[dy * WIDTH + dx] = Tap {
                    coordinate: coordinate.unwrap_or([0.; 2]),
                    target: if valid { luminance(target, px, py)? } else { 0. },
                    valid,
                };
            }
        }
        let mut best: Option<(u64, u64, [i32; 2], [u64; 4])> = None;
        let r = i64::from(policy.offset_radius);
        let step = i64::from(policy.offset_step);
        for iy in 0..side {
            let oy = -r + (iy as i64) * step;
            for ix in 0..side {
                let ox = -r + (ix as i64) * step;
                if cancel() {
                    return Err(RankRegionError::Cancelled);
                }
                for dy in 0..WIDTH {
                    let mut row = Prefix::default();
                    for dx in 0..WIDTH {
                        if cancel() {
                            return Err(RankRegionError::Cancelled);
                        }
                        let tap = taps[dy * WIDTH + dx];
                        if tap.valid {
                            let p = [tap.coordinate[0] + ox as f64, tap.coordinate[1] + oy as f64];
                            row.source += interpolated(source, p)?.ok_or(RankRegionError::Invalid)?;
                            row.target += tap.target;
                        } else {
                            row.invalid += 1;
                        }
                        let i = (dy + 1) * STRIDE + dx + 1;
                        let above = sums[i - STRIDE];
                        sums[i] = Prefix {
                            source: row.source + above.source,
                            target: row.target + above.target,
                            invalid: row.invalid + above.invalid,
                        };
                    }
                }
                let Some(c) = counts(&sums, policy.minimum_contrast) else {
                    continue;
                };
                if c[0] < policy.minimum_pairs || c[2] < policy.minimum_pairs {
                    continue;
                }
                if best.as_ref().is_none_or(|(n, k, _, _)| {
                    u128::from(c[1]) * u128::from(*n) > u128::from(*k) * u128::from(c[0])
                }) {
                    best = Some((c[0], c[1], [ox as i32, oy as i32], c));
                }
            }
        }
        output.push(if let Some((_, _, offset, c)) = best {
            SeedRefinement::Scored(TranslationEvidence {
                initial_source: initial,
                source: [
                    initial[0] + f64::from(offset[0]),
                    initial[1] + f64::from(offset[1]),
                ],
                offset,
                training_pairs: c[0],
                training_agreeing: c[1],
                validation_pairs: c[2],
                validation_agreeing: c[3],
                eligible_proposal: c[1] as f64 / c[0] as f64 >= policy.minimum_training_fraction
                    && c[3] as f64 / c[2] as f64 >= policy.minimum_validation_fraction,
            })
        } else {
            SeedRefinement::Uninformative
        });
    }
    if cancel() {
        return Err(RankRegionError::Cancelled);
    }
    drop(taps);
    drop(sums);
    drop(scratch_credit);
    output_credit
        .try_adopt(output)
        .map_err(|_| RankRegionError::Budget)
}

#[derive(Debug, Clone, Copy)]
pub struct LandmarkAdmissionPolicy {
    pub maximum_points: usize,
    pub maximum_proposals: usize,
    pub maximum_distance_tests: u64,
    pub minimum_separation: f64,
}
#[derive(Debug, Clone)]
pub struct AdmittedLandmarks {
    pub points: SharedBuffer<crate::geometry::Correspondence>,
    pub eligible_proposals: usize,
    pub duplicate_proposals: usize,
}
/// Preserve every original point and append eligible, spatially distinct source/
/// target proposals in input order. Duplicate proposals are counted explicitly.
/// Invalid original locations or malformed scored evidence refuse the operation.
/// This is uniqueness admission only: full mesh conformity remains mandatory.
pub fn admit_mesh_landmarks(
    original: &[crate::geometry::Correspondence],
    seeds: &[[u32; 2]],
    proposals: &[SeedRefinement],
    source_dimensions: (u32, u32),
    target_dimensions: (u32, u32),
    policy: LandmarkAdmissionPolicy,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<AdmittedLandmarks, RankRegionError> {
    if cancel() {
        return Err(RankRegionError::Cancelled);
    }
    if seeds.len() != proposals.len()
        || source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || target_dimensions.0 == 0
        || target_dimensions.1 == 0
        || !policy.minimum_separation.is_finite()
        || policy.minimum_separation <= 0.
    {
        return Err(RankRegionError::Invalid);
    }
    if proposals.len() > policy.maximum_proposals || original.len() > policy.maximum_points {
        return Err(RankRegionError::Budget);
    }
    let separation_squared = policy.minimum_separation * policy.minimum_separation;
    if !separation_squared.is_finite() {
        return Err(RankRegionError::Invalid);
    }
    let inside = |p: [f64; 2], size: (u32, u32)| {
        p.iter().all(|v| v.is_finite() && *v >= 0.) && p[0] < f64::from(size.0) && p[1] < f64::from(size.1)
    };
    let mut eligible = 0usize;
    for (seed, proposal) in seeds.iter().zip(proposals) {
        if cancel() {
            return Err(RankRegionError::Cancelled);
        }
        if let SeedRefinement::Scored(e) = proposal {
            if e.training_pairs == 0
                || e.validation_pairs == 0
                || e.training_agreeing > e.training_pairs
                || e.validation_agreeing > e.validation_pairs
                || !inside(e.initial_source, source_dimensions)
                || !inside(e.source, source_dimensions)
                || !inside(seed.map(f64::from), target_dimensions)
                || e.source
                    != [
                        e.initial_source[0] + f64::from(e.offset[0]),
                        e.initial_source[1] + f64::from(e.offset[1]),
                    ]
            {
                return Err(RankRegionError::Invalid);
            }
            if e.eligible_proposal {
                eligible = eligible.checked_add(1).ok_or(RankRegionError::Budget)?;
            }
        }
    }
    let capacity = original
        .len()
        .checked_add(eligible)
        .ok_or(RankRegionError::Budget)?;
    if capacity > policy.maximum_points {
        return Err(RankRegionError::Budget);
    }
    let n = u64::try_from(original.len()).map_err(|_| RankRegionError::Budget)?;
    let e = u64::try_from(eligible).map_err(|_| RankRegionError::Budget)?;
    let pairs = |v: u64| v.checked_mul(v.saturating_sub(1)).map(|v| v / 2);
    let work = pairs(n)
        .and_then(|v| n.checked_mul(e).and_then(|a| v.checked_add(a)))
        .and_then(|v| pairs(e).and_then(|a| v.checked_add(a)))
        .ok_or(RankRegionError::Budget)?;
    if work > policy.maximum_distance_tests {
        return Err(RankRegionError::Budget);
    }
    let conflict = |a: &crate::geometry::Correspondence, b: &crate::geometry::Correspondence| {
        [a.source, a.target]
            .iter()
            .zip([b.source, b.target])
            .any(|(a, b)| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) <= separation_squared)
    };
    for (i, p) in original.iter().enumerate() {
        if cancel() {
            return Err(RankRegionError::Cancelled);
        }
        if !inside(p.source, source_dimensions) || !inside(p.target, target_dimensions) {
            return Err(RankRegionError::Invalid);
        }
        for prior in &original[..i] {
            if cancel() {
                return Err(RankRegionError::Cancelled);
            }
            if conflict(p, prior) {
                return Err(RankRegionError::Invalid);
            }
        }
    }
    let bytes = capacity
        .checked_mul(std::mem::size_of::<crate::geometry::Correspondence>())
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(RankRegionError::Budget)?;
    let credit = budget.try_reserve(bytes).map_err(|_| RankRegionError::Budget)?;
    let mut points = Vec::new();
    points
        .try_reserve_exact(capacity)
        .map_err(|_| RankRegionError::Budget)?;
    if points.capacity() > capacity {
        return Err(RankRegionError::Budget);
    }
    for p in original {
        if cancel() {
            return Err(RankRegionError::Cancelled);
        }
        points.push(*p);
    }
    let mut duplicates = 0;
    for (seed, proposal) in seeds.iter().zip(proposals) {
        if cancel() {
            return Err(RankRegionError::Cancelled);
        }
        let SeedRefinement::Scored(e) = proposal else {
            continue;
        };
        if !e.eligible_proposal {
            continue;
        }
        let p = crate::geometry::Correspondence {
            source: e.source,
            target: seed.map(f64::from),
        };
        let mut duplicate = false;
        for prior in &points {
            if cancel() {
                return Err(RankRegionError::Cancelled);
            }
            if conflict(&p, prior) {
                duplicate = true;
                break;
            }
        }
        if duplicate {
            duplicates += 1;
        } else {
            points.push(p);
        }
    }
    if cancel() {
        return Err(RankRegionError::Cancelled);
    }
    let points = credit.try_adopt(points).map_err(|_| RankRegionError::Budget)?;
    Ok(AdmittedLandmarks {
        points,
        eligible_proposals: eligible,
        duplicate_proposals: duplicates,
    })
}
