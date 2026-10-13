//! Exact squared-distance descriptor retrieval; candidates need independent matching/geometry.
use crate::{
    gradient::{GradientCellRecipe, GradientDescriptor},
    local::LocalError,
};
use rrrah_core::{MemoryBudget, SharedBuffer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientCandidate {
    pub id: u64,
    pub squared_distance: f64,
}

/// Borrows immutable caller-owned descriptors; only the ordering buffer is reserved here.
/// A sorted coordinate provides a necessary lower bound, followed by full128-bin distance.
/// No approximate projection, ambiguity rejection or duplicate identity is implied.
pub struct GradientDescriptorIndex<'a> {
    entries: &'a [(u64, GradientDescriptor)],
    order: SharedBuffer<usize>,
    axis: usize,
    recipe: GradientCellRecipe,
}
fn valid(d: &GradientDescriptor) -> bool {
    d.0.iter().all(|v| v.is_finite() && *v >= 0.)
        && (d.0.iter().map(|v| v * v).sum::<f64>() - 1.).abs() <= 1e-6
}
impl<'a> GradientDescriptorIndex<'a> {
    /// Build a bounded ordering. Duplicate IDs, nonunit descriptors and cancellation refuse atomically.
    pub fn new(
        entries: &'a [(u64, GradientDescriptor)],
        recipe: GradientCellRecipe,
        max_entries: usize,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, LocalError> {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if entries.len() > max_entries {
            return Err(LocalError::Budget);
        }
        let mut low = [f64::INFINITY; 128];
        let mut high = [f64::NEG_INFINITY; 128];
        for (_, d) in entries {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            if !valid(d) {
                return Err(LocalError::Invalid);
            }
            for (dim, value) in d.0.iter().copied().enumerate() {
                low[dim] = low[dim].min(value);
                high[dim] = high[dim].max(value);
            }
        }
        let bytes = entries
            .len()
            .checked_mul(std::mem::size_of::<usize>())
            .and_then(|v| u64::try_from(v).ok())
            .ok_or(LocalError::Budget)?;
        let reservation = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
        let mut order = Vec::new();
        order
            .try_reserve_exact(entries.len())
            .map_err(|_| LocalError::Budget)?;
        order.extend(0..entries.len());
        order.sort_unstable_by_key(|&i| entries[i].0);
        if order.windows(2).any(|w| entries[w[0]].0 == entries[w[1]].0) {
            return Err(LocalError::Invalid);
        }
        let mut axis = 0;
        let mut widest = -1.;
        for dim in 0..128 {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let span = high[dim] - low[dim];
            if span > widest {
                widest = span;
                axis = dim;
            }
        }
        order.sort_unstable_by(|&a, &b| {
            entries[a].1.0[axis]
                .total_cmp(&entries[b].1.0[axis])
                .then(entries[a].0.cmp(&entries[b].0))
        });
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let order = reservation
            .try_adopt(order)
            .map_err(|_| LocalError::Budget)?;
        Ok(Self {
            entries,
            order,
            axis,
            recipe,
        })
    }
    /// Search a horizontally reflected source descriptor in the ordinary index.
    /// Coordinates remain caller-owned; these are proposals requiring pixel confirmation.
    /// Reflection admission is explicit and no second descriptor bank is allocated.
    pub fn search_reflected(
        &self,
        query: &GradientDescriptor,
        recipe: GradientCellRecipe,
        radius: f64,
        max_comparisons: u64,
        max_results: usize,
        maximum_reflection_bins: u64,
        cancel: impl Fn() -> bool,
    ) -> Result<Vec<GradientCandidate>, LocalError> {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if maximum_reflection_bins < 128 {
            return Err(LocalError::Budget);
        }
        let reflected = crate::gradient::reflect_gradient_descriptor(query, &cancel)?;
        self.search(
            &reflected,
            recipe,
            radius,
            max_comparisons,
            max_results,
            cancel,
        )
    }
    /// Exact inclusive radius retrieval, ordered by(distance,id). Work counts full descriptor comparisons.
    /// Result storage is fallibly allocated and bounded by max_results; source descriptors remain borrowed.
    pub fn search(
        &self,
        query: &GradientDescriptor,
        recipe: GradientCellRecipe,
        radius: f64,
        max_comparisons: u64,
        max_results: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Vec<GradientCandidate>, LocalError> {
        self.search_with_comparisons(query, recipe, radius, max_comparisons, max_results, cancel)
            .map(|(hits, _)| hits)
    }
    fn search_with_comparisons(
        &self,
        query: &GradientDescriptor,
        recipe: GradientCellRecipe,
        radius: f64,
        max_comparisons: u64,
        max_results: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<(Vec<GradientCandidate>, u64), LocalError> {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if recipe != self.recipe
            || !valid(query)
            || !radius.is_finite()
            || !(0. ..=4.).contains(&radius)
        {
            return Err(LocalError::Invalid);
        }
        let q = query.0[self.axis];
        let lower = |i: usize| {
            let v = self.entries[i].1.0[self.axis];
            let d = v - q;
            (v, d * d)
        };
        let start = self.order.partition_point(|&i| {
            let (v, d) = lower(i);
            v < q && d > radius
        });
        let end = self.order.partition_point(|&i| {
            let (v, d) = lower(i);
            v <= q || d <= radius
        });
        let comparisons = u64::try_from(end - start).map_err(|_| LocalError::Budget)?;
        if comparisons > max_comparisons {
            return Err(LocalError::Budget);
        }
        let mut result = Vec::new();
        for &i in &self.order[start..end] {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let mut distance = 0.;
            for (a, b) in query.0.iter().zip(self.entries[i].1.0.iter()) {
                let delta = a - b;
                distance += delta * delta;
                // Valid descriptors are finite; every term is nonnegative.
                // Retained distances still accumulate all bins in the same
                // order. Prepaid comparison caps include rejected candidates.
                if distance > radius {
                    break;
                }
            }
            if distance <= radius {
                if result.len() >= max_results {
                    return Err(LocalError::Budget);
                }
                if result.len() == result.capacity() {
                    let limit = max_results.min(end - start);
                    let remaining = limit - result.len();
                    let growth = result.capacity().max(1).min(remaining);
                    result
                        .try_reserve_exact(growth)
                        .map_err(|_| LocalError::Budget)?;
                }
                result.push(GradientCandidate {
                    id: self.entries[i].0,
                    squared_distance: distance,
                });
            }
        }
        result.sort_unstable_by(|a, b| {
            a.squared_distance
                .total_cmp(&b.squared_distance)
                .then(a.id.cmp(&b.id))
        });
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        Ok((result, comparisons))
    }
}

/// Conservative cross-file proposals for one explicit descriptor recipe.
/// `owners[i]` identifies the file of `entries[i]`; entry IDs remain unique.
/// Every within-radius descriptor hit participates, including ambiguous matches.
/// Mutual matching and geometric/pixel decisions run only during fresh file confirmation.
/// The declared comparison budget conservatively admits N squared; hit/pair caps
/// refuse atomically. Returned storage is bounded/fallible, not a whole-RSS claim.
pub fn gradient_file_pairs(
    entries: &[(u64, GradientDescriptor)],
    owners: &[u64],
    recipe: GradientCellRecipe,
    radius: f64,
    max_comparisons: u64,
    max_hits: usize,
    max_pairs: usize,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<Vec<(u64, u64)>, LocalError> {
    Ok(gradient_file_pair_report(
        entries,
        owners,
        recipe,
        radius,
        max_comparisons,
        max_hits,
        max_pairs,
        budget,
        cancel,
    )?
    .pairs)
}
#[derive(Debug)]
pub struct GradientFilePairReport {
    pub pairs: Vec<(u64, u64)>,
    // Retains pair-list credit while the report owns its proposal storage.
    _pair_reservation: rrrah_core::Reservation,
    /// Includes same-file/self descriptor hits admitted by the declared hit cap.
    pub descriptor_hits: usize,
    /// Actual coordinate-window candidates evaluated, including rejected distances.
    /// Index construction, coordinate lookup and reflection have separate bounds.
    pub descriptor_comparisons: u64,
}
pub fn gradient_file_pair_report(
    entries: &[(u64, GradientDescriptor)],
    owners: &[u64],
    recipe: GradientCellRecipe,
    radius: f64,
    max_comparisons: u64,
    max_hits: usize,
    max_pairs: usize,
    budget: &MemoryBudget,
    cancel: impl Fn() -> bool,
) -> Result<GradientFilePairReport, LocalError> {
    gradient_file_pair_report_selected(
        entries,
        owners,
        recipe,
        radius,
        max_comparisons,
        max_hits,
        max_pairs,
        budget,
        None,
        true,
        cancel,
    )
}

/// Ordinary and reflected-source proposals share one index and cumulative hit/pair limits.
/// Both query domains are prepaid before allocation; all hits require pixel confirmation.
pub fn gradient_file_pair_report_with_reflection(
    entries: &[(u64, GradientDescriptor)],
    owners: &[u64],
    recipe: GradientCellRecipe,
    radius: f64,
    max_comparisons: u64,
    max_hits: usize,
    max_pairs: usize,
    budget: &MemoryBudget,
    maximum_reflection_bins: u64,
    cancel: impl Fn() -> bool,
) -> Result<GradientFilePairReport, LocalError> {
    gradient_file_pair_report_selected(
        entries,
        owners,
        recipe,
        radius,
        max_comparisons,
        max_hits,
        max_pairs,
        budget,
        Some(maximum_reflection_bins),
        true,
        cancel,
    )
}

/// Exact descriptor-radius proposals with a cumulative actual-comparison cap.
/// Unlike the legacy worst-case preflight, a sparse coordinate window can admit
/// fewer than n*n comparisons. Every attempted descriptor distance consumes one
/// unit, including early rejected distances; self/same-file hits still count.
/// `reflection_bins` is None for ordinary search, or the prepaid transform-bin
/// cap for both ordinary and reflected queries. Index/scratch/pair memory, hit
/// and pair caps remain independent. Sources remain immutable borrowed data.
/// These proposals require fresh geometry/pixel confirmation, not file identity.
/// # Errors
/// Invalid data, cancellation or any cumulative limit refuses the whole report;
/// no partial pairs publish. Actual work refusal can occur after allocation or
/// earlier successful queries; all managed reservations are released.
#[allow(clippy::too_many_arguments)]
pub fn gradient_file_pair_report_with_actual_work(
    entries: &[(u64, GradientDescriptor)],
    owners: &[u64],
    recipe: GradientCellRecipe,
    radius: f64,
    max_comparisons: u64,
    max_hits: usize,
    max_pairs: usize,
    budget: &MemoryBudget,
    reflection_bins: Option<u64>,
    cancel: impl Fn() -> bool,
) -> Result<GradientFilePairReport, LocalError> {
    gradient_file_pair_report_selected(
        entries,
        owners,
        recipe,
        radius,
        max_comparisons,
        max_hits,
        max_pairs,
        budget,
        reflection_bins,
        false,
        cancel,
    )
}

fn gradient_file_pair_report_selected(
    entries: &[(u64, GradientDescriptor)],
    owners: &[u64],
    recipe: GradientCellRecipe,
    radius: f64,
    max_comparisons: u64,
    max_hits: usize,
    max_pairs: usize,
    budget: &MemoryBudget,
    reflection_bins: Option<u64>,
    prepay_worst_case: bool,
    cancel: impl Fn() -> bool,
) -> Result<GradientFilePairReport, LocalError> {
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    if entries.len() != owners.len() || !radius.is_finite() || !(0. ..=4.).contains(&radius) {
        return Err(LocalError::Invalid);
    }
    let n = u64::try_from(entries.len()).map_err(|_| LocalError::Budget)?;
    let phases = if reflection_bins.is_some() { 2 } else { 1 };
    // Every ordinary query must visit and admit its own descriptor.
    // Reject impossible budgets before allocating the input-sized index.
    if !prepay_worst_case && (n > max_comparisons || entries.len() > max_hits) {
        return Err(LocalError::Budget);
    }
    if prepay_worst_case
        && n.checked_mul(n)
            .and_then(|v| v.checked_mul(phases))
            .ok_or(LocalError::Budget)?
            > max_comparisons
    {
        return Err(LocalError::Budget);
    }
    if reflection_bins
        .is_some_and(|limit| n.checked_mul(128).is_none_or(|required| required > limit))
    {
        return Err(LocalError::Budget);
    }
    let index = GradientDescriptorIndex::new(entries, recipe, entries.len(), budget, &cancel)?;
    let id_bytes = entries
        .len()
        .checked_mul(std::mem::size_of::<(u64, u64)>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let ids_credit = budget
        .try_reserve(id_bytes)
        .map_err(|_| LocalError::Budget)?;
    let mut ids = Vec::new();
    ids.try_reserve_exact(entries.len())
        .map_err(|_| LocalError::Budget)?;
    ids.extend(
        entries
            .iter()
            .zip(owners)
            .map(|((id, _), owner)| (*id, *owner)),
    );
    ids.sort_unstable_by_key(|entry| entry.0);
    let ids = ids_credit.try_adopt(ids).map_err(|_| LocalError::Budget)?;
    let mut pair_credit = budget.try_reserve(0).map_err(|_| LocalError::Budget)?;
    let mut hits = 0_usize;
    let mut comparisons = 0_u64;
    let mut pairs = Vec::new();
    for ((_, descriptor), &owner) in entries.iter().zip(owners) {
        for phase in 0..phases {
            let query_bytes = entries
                .len()
                .min(max_hits.saturating_sub(hits))
                .checked_mul(std::mem::size_of::<GradientCandidate>())
                .and_then(|n| u64::try_from(n).ok())
                .ok_or(LocalError::Budget)?;
            let _query_credit = budget
                .try_reserve(query_bytes)
                .map_err(|_| LocalError::Budget)?;
            let reflected;
            let query = if phase == 1 {
                reflected = crate::gradient::reflect_gradient_descriptor(descriptor, &cancel)?;
                &reflected
            } else {
                descriptor
            };
            let remaining = if prepay_worst_case {
                n
            } else {
                max_comparisons.saturating_sub(comparisons)
            };
            let (results, attempted) = index.search_with_comparisons(
                query,
                recipe,
                radius,
                remaining,
                max_hits.saturating_sub(hits),
                &cancel,
            )?;
            comparisons = comparisons
                .checked_add(attempted)
                .ok_or(LocalError::Budget)?;
            hits = hits.checked_add(results.len()).ok_or(LocalError::Budget)?;
            for result in results {
                if cancel() {
                    return Err(LocalError::Cancelled);
                }
                let i = ids
                    .binary_search_by_key(&result.id, |v| v.0)
                    .map_err(|_| LocalError::Invalid)?;
                let other = ids[i].1;
                if owner != other {
                    let pair = (owner.min(other), owner.max(other));
                    // Descriptor hits may repeat the same file pair millions of times.
                    // Keep only unique proposals; the independent hit count stays intact.
                    let position = match pairs.binary_search(&pair) {
                        Ok(_) => continue,
                        Err(position) => position,
                    };
                    let required = pairs.len().checked_add(1).ok_or(LocalError::Budget)?;
                    if required > max_pairs {
                        return Err(LocalError::Budget);
                    }
                    let bytes = required
                        .checked_mul(std::mem::size_of::<(u64, u64)>())
                        .and_then(|n| u64::try_from(n).ok())
                        .ok_or(LocalError::Budget)?;
                    pair_credit
                        .ensure_bytes(bytes)
                        .map_err(|_| LocalError::Budget)?;
                    pairs.try_reserve_exact(1).map_err(|_| LocalError::Budget)?;
                    let actual = pairs
                        .capacity()
                        .checked_mul(std::mem::size_of::<(u64, u64)>())
                        .and_then(|n| u64::try_from(n).ok())
                        .ok_or(LocalError::Budget)?;
                    pair_credit
                        .ensure_bytes(actual)
                        .map_err(|_| LocalError::Budget)?;
                    pairs.insert(position, pair);
                }
            }
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    if pairs.len() > max_pairs {
        return Err(LocalError::Budget);
    }
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    Ok(GradientFilePairReport {
        pairs,
        descriptor_hits: hits,
        descriptor_comparisons: comparisons,
        _pair_reservation: pair_credit,
    })
}

#[cfg(test)]
mod row_major_tests {
    use super::*;
    fn reference<'a>(
        entries: &'a [(u64, GradientDescriptor)],
        recipe: GradientCellRecipe,
        max_entries: usize,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<GradientDescriptorIndex<'a>, LocalError> {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if entries.len() > max_entries {
            return Err(LocalError::Budget);
        }
        for (_, d) in entries {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            if !valid(d) {
                return Err(LocalError::Invalid);
            }
        }
        let bytes = entries
            .len()
            .checked_mul(std::mem::size_of::<usize>())
            .and_then(|v| u64::try_from(v).ok())
            .ok_or(LocalError::Budget)?;
        let reservation = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
        let mut order = Vec::new();
        order
            .try_reserve_exact(entries.len())
            .map_err(|_| LocalError::Budget)?;
        order.extend(0..entries.len());
        order.sort_unstable_by_key(|&i| entries[i].0);
        if order.windows(2).any(|w| entries[w[0]].0 == entries[w[1]].0) {
            return Err(LocalError::Invalid);
        }
        let mut axis = 0;
        let mut widest = -1.;
        for dim in 0..128 {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
            for (_, d) in entries {
                low = low.min(d.0[dim]);
                high = high.max(d.0[dim]);
            }
            if high - low > widest {
                widest = high - low;
                axis = dim;
            }
        }
        order.sort_unstable_by(|&a, &b| {
            entries[a].1.0[axis]
                .total_cmp(&entries[b].1.0[axis])
                .then(entries[a].0.cmp(&entries[b].0))
        });
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let order = reservation
            .try_adopt(order)
            .map_err(|_| LocalError::Budget)?;
        Ok(GradientDescriptorIndex {
            entries,
            order,
            axis,
            recipe,
        })
    }
    fn bank(n: usize, family: usize) -> Vec<(u64, GradientDescriptor)> {
        let mut seed = 173u64;
        (0..n)
            .map(|i| {
                let mut values = [0.; 128];
                for (dim, v) in values.iter_mut().enumerate() {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    *v = match family {
                        1 => 1.,
                        2 => f64::from(dim == i % 128),
                        _ => ((seed >> 32) as u32 as f64 + 1.),
                    };
                }
                let norm = values.iter().map(|v| v * v).sum::<f64>().sqrt();
                for v in &mut values {
                    *v /= norm;
                }
                ((n - i) as u64, GradientDescriptor(values))
            })
            .collect()
    }
    #[test]
    fn row_major_preserves_axis_order_search_credit_and_every_cancellation_checkpoint() {
        use std::cell::Cell;
        for family in 0..3 {
            for n in [0, 1, 17, 64, 1024] {
                let entries = bank(n, family);
                for recipe in [GradientCellRecipe::Fixed, GradientCellRecipe::Interpolated] {
                    let ba = MemoryBudget::new(1_000_000);
                    let bb = MemoryBudget::new(1_000_000);
                    let ca = Cell::new(0);
                    let cb = Cell::new(0);
                    let a = reference(&entries, recipe, n, &ba, || {
                        ca.set(ca.get() + 1);
                        false
                    })
                    .unwrap();
                    let b = GradientDescriptorIndex::new(&entries, recipe, n, &bb, || {
                        cb.set(cb.get() + 1);
                        false
                    })
                    .unwrap();
                    assert_eq!(a.axis, b.axis);
                    assert_eq!(&a.order[..], &b.order[..]);
                    assert_eq!(ca.get(), cb.get());
                    assert_eq!(ba.peak(), bb.peak());
                    for (_, query) in entries.iter().take(4) {
                        for radius in [0., 0.001, 0.1, 4.] {
                            assert_eq!(
                                a.search(query, recipe, radius, n as u64, n, || false)
                                    .unwrap(),
                                b.search(query, recipe, radius, n as u64, n, || false)
                                    .unwrap()
                            );
                        }
                    }
                    drop(a);
                    drop(b);
                    assert_eq!(ba.used(), 0);
                    assert_eq!(bb.used(), 0);
                }
            }
        }
        let entries = bank(64, 0);
        let recipe = GradientCellRecipe::Fixed;
        let count = Cell::new(0);
        let budget = MemoryBudget::new(4096);
        drop(
            reference(&entries, recipe, 64, &budget, || {
                count.set(count.get() + 1);
                false
            })
            .unwrap(),
        );
        for stop in 1..=count.get() {
            for baseline in [true, false] {
                let seen = Cell::new(0);
                let budget = MemoryBudget::new(4096);
                let cancel = || {
                    seen.set(seen.get() + 1);
                    seen.get() == stop
                };
                let result = if baseline {
                    reference(&entries, recipe, 64, &budget, cancel)
                } else {
                    GradientDescriptorIndex::new(&entries, recipe, 64, &budget, cancel)
                };
                assert!(matches!(result, Err(LocalError::Cancelled)));
                assert_eq!(budget.used(), 0);
            }
        }
    }
    #[test]
    #[ignore = "explicit alternating index construction measurement"]
    fn row_major_index_build_latency_probe() {
        use std::{
            sync::atomic::{AtomicBool, Ordering},
            time::Instant,
        };
        for (name, n, family) in [
            ("dense512", 512, 0),
            ("dense8192", 8192, 0),
            ("dense32768", 32768, 0),
            ("flat8192", 8192, 1),
            ("sparse8192", 8192, 2),
        ] {
            let entries = bank(n, family);
            let stopped = AtomicBool::new(false);
            for round in 0..8 {
                for baseline in if round % 2 == 0 {
                    [true, false, false, true]
                } else {
                    [false, true, true, false]
                } {
                    let budget = MemoryBudget::new(1_000_000);
                    let cancel = || std::hint::black_box(&stopped).load(Ordering::Acquire);
                    let start = Instant::now();
                    let index = if baseline {
                        reference(&entries, GradientCellRecipe::Fixed, n, &budget, cancel)
                    } else {
                        GradientDescriptorIndex::new(
                            &entries,
                            GradientCellRecipe::Fixed,
                            n,
                            &budget,
                            cancel,
                        )
                    }
                    .unwrap();
                    let elapsed = start.elapsed().as_nanos();
                    println!(
                        "ROW_MAJOR_INDEX_BENCH control={} baseline={} round={} elapsed_ns={} peak={} axis={}",
                        name,
                        baseline,
                        round,
                        elapsed,
                        budget.peak(),
                        index.axis
                    );
                    std::hint::black_box(&index);
                    drop(index);
                    assert_eq!(budget.used(), 0);
                }
            }
        }
    }
}
