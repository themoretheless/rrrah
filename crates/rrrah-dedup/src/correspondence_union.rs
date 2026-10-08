//! Explicit candidate union; final geometry and original-pixel confirmation remain required.
use crate::{geometry::Correspondence, local::LocalError};
use rrrah_core::{MemoryBudget, SharedBuffer};

/// Merge recipe-qualified correspondences in canonical coordinate order and
/// retain independent source AND target locations outside the inclusive radius.
/// Exact duplicates never count as additional geometric support. Input lists
/// and sort control stack are caller-owned/outside the managed vector ledger.
/// `max_points` bounds input records, including duplicates; `max_comparisons`
/// admits the conservative n*(n-1)/2 distinct-location comparisons before allocation.
/// Returned buffer credit follows its last owner, including unused capacity.
///
/// # Errors
/// Nonfinite points/radius, overflowing or insufficient work/memory admission,
/// or cancellation without retained partial evidence. Sort is checked before/after.
pub fn union_correspondences_distinct_managed(
    recipes: &[&[Correspondence]],
    radius: f64,
    max_points: usize,
    max_comparisons: u64,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<SharedBuffer<Correspondence>, LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let radius_squared = radius * radius;
    if !radius.is_finite() || radius < 0. || !radius_squared.is_finite() {
        return Err(LocalError::Invalid);
    }
    let mut count = 0usize;
    for recipe in recipes {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        count = count.checked_add(recipe.len()).ok_or(LocalError::Budget)?;
        if count > max_points {
            return Err(LocalError::Budget);
        }
        for point in *recipe {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            if !point.source.into_iter().chain(point.target).all(f64::is_finite) {
                return Err(LocalError::Invalid);
            }
        }
    }
    let n = u64::try_from(count).map_err(|_| LocalError::Budget)?;
    let work = n.checked_mul(n.saturating_sub(1)).ok_or(LocalError::Budget)? / 2;
    if work > max_comparisons {
        return Err(LocalError::Budget);
    }
    let bytes = count
        .checked_mul(std::mem::size_of::<Correspondence>())
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(LocalError::Budget)?;
    let credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
    let mut points = Vec::new();
    points.try_reserve_exact(count).map_err(|_| LocalError::Budget)?;
    for recipe in recipes {
        for point in *recipe {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let canonical = |v: f64| if v == 0. { 0. } else { v };
            points.push(Correspondence {
                source: point.source.map(canonical),
                target: point.target.map(canonical),
            });
        }
    }
    points.sort_unstable_by(|a, b| {
        a.source[0]
            .total_cmp(&b.source[0])
            .then(a.source[1].total_cmp(&b.source[1]))
            .then(a.target[0].total_cmp(&b.target[0]))
            .then(a.target[1].total_cmp(&b.target[1]))
    });
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    let mut retained = 0;
    for index in 0..points.len() {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let point = points[index];
        let mut independent = true;
        for prior in &points[..retained] {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let distance = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
            if distance(point.source, prior.source) <= radius_squared
                || distance(point.target, prior.target) <= radius_squared
            {
                independent = false;
                break;
            }
        }
        if independent {
            points[retained] = point;
            retained += 1;
        }
    }
    points.truncate(retained);
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    credit.try_adopt(points).map_err(|_| LocalError::Budget)
}
