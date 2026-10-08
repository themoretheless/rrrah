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
        let order = reservation.try_adopt(order).map_err(|_| LocalError::Budget)?;
        Ok(Self {
            entries,
            order,
            axis,
            recipe,
        })
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
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if recipe != self.recipe || !valid(query) || !radius.is_finite() || !(0. ..=4.).contains(&radius) {
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
        if u64::try_from(end - start).map_err(|_| LocalError::Budget)? > max_comparisons {
            return Err(LocalError::Budget);
        }
        let mut result = Vec::new();
        for &i in &self.order[start..end] {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let distance = query
                .0
                .iter()
                .zip(self.entries[i].1.0)
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f64>();
            if distance <= radius {
                if result.len() >= max_results {
                    return Err(LocalError::Budget);
                }
                result.try_reserve_exact(1).map_err(|_| LocalError::Budget)?;
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
        Ok(result)
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
    if cancel() {
        return Err(LocalError::Cancelled);
    }
    if entries.len() != owners.len() || !radius.is_finite() || !(0. ..=4.).contains(&radius) {
        return Err(LocalError::Invalid);
    }
    let n = u64::try_from(entries.len()).map_err(|_| LocalError::Budget)?;
    if n.checked_mul(n).ok_or(LocalError::Budget)? > max_comparisons {
        return Err(LocalError::Budget);
    }
    let index = GradientDescriptorIndex::new(entries, recipe, entries.len(), budget, &cancel)?;
    let id_bytes = entries
        .len()
        .checked_mul(std::mem::size_of::<(u64, u64)>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let ids_credit = budget.try_reserve(id_bytes).map_err(|_| LocalError::Budget)?;
    let mut ids = Vec::new();
    ids.try_reserve_exact(entries.len())
        .map_err(|_| LocalError::Budget)?;
    ids.extend(entries.iter().zip(owners).map(|((id, _), owner)| (*id, *owner)));
    ids.sort_unstable_by_key(|entry| entry.0);
    let ids = ids_credit.try_adopt(ids).map_err(|_| LocalError::Budget)?;
    let mut pair_credit = budget.try_reserve(0).map_err(|_| LocalError::Budget)?;
    let mut hits = 0_usize;
    let mut pairs = Vec::new();
    for ((_, descriptor), &owner) in entries.iter().zip(owners) {
        let query_bytes = entries
            .len()
            .min(max_hits.saturating_sub(hits))
            .checked_mul(std::mem::size_of::<GradientCandidate>())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(LocalError::Budget)?;
        let _query_credit = budget.try_reserve(query_bytes).map_err(|_| LocalError::Budget)?;
        let results = index.search(
            descriptor,
            recipe,
            radius,
            n,
            max_hits.saturating_sub(hits),
            &cancel,
        )?;
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
                pair_credit.ensure_bytes(bytes).map_err(|_| LocalError::Budget)?;
                pairs.try_reserve_exact(1).map_err(|_| LocalError::Budget)?;
                let actual = pairs
                    .capacity()
                    .checked_mul(std::mem::size_of::<(u64, u64)>())
                    .and_then(|n| u64::try_from(n).ok())
                    .ok_or(LocalError::Budget)?;
                pair_credit.ensure_bytes(actual).map_err(|_| LocalError::Budget)?;
                pairs.insert(position, pair);
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
        _pair_reservation: pair_credit,
    })
}
