//! Duplicate search primitives. Perceptual matches are candidates, not proof
//! of content identity. See `docs/DUPLICATE_LIBRARY_CONTRACT.md` for coverage.

#[cfg(feature = "decode")]
pub mod animated;
pub mod affine_color;
pub mod cache;
#[cfg(feature = "decode")]
pub mod container;
pub mod crop;
#[cfg(feature = "decode")]
pub mod decode;
pub mod exact;
pub mod geometry;
pub mod gradient;
pub mod gradient_scale;
#[cfg(feature = "decode")]
pub mod gradient_index;
pub mod groups;
pub mod linear;
pub mod local;
#[cfg(feature = "decode")]
pub mod local_collection;
pub mod local_index;
#[cfg(feature = "decode")]
pub mod local_scan;
#[cfg(feature = "decode")]
pub mod managed_evidence;
pub mod ordinal;
#[cfg(feature = "decode")]
pub mod pages;
#[cfg(feature = "decode")]
pub mod pixel_index;
pub mod pixels;
#[cfg(feature = "decode")]
pub mod presentation_kind;
pub mod pyramid;
#[cfg(feature = "raster")]
pub mod region_grid;
#[cfg(feature = "decode")]
pub mod region_transform;
#[cfg(feature = "raster")]
pub mod raster;
#[cfg(feature = "decode")]
pub mod scan;
pub mod sequence;
#[cfg(feature = "decode")]
mod managed_metric;
#[cfg(feature = "decode")]
mod managed_visual;
#[cfg(feature = "decode")]
mod managed_descriptor_union;
#[cfg(feature = "decode")]
mod managed_targets;
#[cfg(feature = "decode")]
mod managed_pair_counts;
pub mod visual;
pub mod warp;

/// An indexed candidate and its exact Hamming distance from the query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    pub id: u64,
    pub distance: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("index search cancelled")]
pub struct SearchCancelled;

#[derive(Debug)]
struct Node {
    hash: u64,
    first_id: u64,
    other_ids: std::collections::HashSet<u64>,
    children: Vec<(u32, usize)>,
}

/// Fallible metric-index construction failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IndexError {
    #[error("metric index exceeds its entry budget or cannot allocate storage")]
    Budget,
    #[error("metric index operation cancelled")]
    Cancelled,
}

fn new_node(id: u64, hash: u64) -> Node {
    Node {
        hash,
        first_id: id,
        other_ids: std::collections::HashSet::new(),
        children: Vec::new(),
    }
}

/// BK-tree over 64-bit fingerprints, using exact metric distances for pruning.
///
/// Inspired by the search architecture of Czkawka and imagededup. This is an
/// independent implementation. Search is iterative, including degenerate trees;
/// identical fingerprints retain every id. Runtime depends on data and radius
/// and may approach a full scan. The index does not assign visual confidence.
#[derive(Debug, Default)]
pub struct HammingIndex {
    nodes: Vec<Node>,
    entries: usize,
}

impl HammingIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an application-assigned id. Reinserting the same id/hash is a no-op.
    /// The caller must remove/rebuild the index when an id's fingerprint changes.
    ///
    /// # Panics
    /// Panics on allocation failure. Use `try_insert_with_cancel` for fallible insertion.
    pub fn insert(&mut self, id: u64, hash: u64) {
        let _ = self.insert_with_cancel(id, hash, || false);
    }

    /// Insert with cancellation checkpoints during tree traversal and before
    /// collision-bucket admission. Cancellation before insertion leaves the index unchanged.
    ///
    /// # Errors
    /// Returns `SearchCancelled` when cancellation is observed.
    ///
    /// # Panics
    /// Panics on allocation failure. Use `try_insert_with_cancel` for fallible insertion.
    pub fn insert_with_cancel(
        &mut self,
        id: u64,
        hash: u64,
        cancel: impl Fn() -> bool,
    ) -> Result<(), SearchCancelled> {
        match self.try_insert_with_cancel(id, hash, usize::MAX, cancel) {
            Ok(()) => Ok(()),
            Err(IndexError::Cancelled) => Err(SearchCancelled),
            Err(IndexError::Budget) => panic!("metric-index allocation failed"),
        }
    }

    /// Insert under a total stored-ID limit, with fallible allocation and
    /// cancellation inside traversal. An existing ID/hash remains a no-op even
    /// at the limit. Failure leaves all searchable entries unchanged.
    ///
    /// # Errors
    /// Budget exhaustion, allocation failure or cancellation; never partial insertion.
    pub fn try_insert_with_cancel(
        &mut self,
        id: u64,
        hash: u64,
        max_entries: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<(), IndexError> {
        if cancel() {
            return Err(IndexError::Cancelled);
        }
        if self.nodes.is_empty() {
            self.admit_entry(max_entries)?;
            local::reserve_slot(&mut self.nodes, max_entries).map_err(|_| IndexError::Budget)?;
            let node = new_node(id, hash);
            if cancel() {
                return Err(IndexError::Cancelled);
            }
            self.nodes.push(node);
            self.entries += 1;
            return Ok(());
        }
        let mut cursor = 0;
        loop {
            if cancel() {
                return Err(IndexError::Cancelled);
            }
            let distance = (hash ^ self.nodes[cursor].hash).count_ones();
            if distance == 0 {
                if self.nodes[cursor].first_id == id || self.nodes[cursor].other_ids.contains(&id) {
                    return Ok(());
                }
                self.admit_entry(max_entries)?;
                self.nodes[cursor]
                    .other_ids
                    .try_reserve(1)
                    .map_err(|_| IndexError::Budget)?;
                if cancel() {
                    return Err(IndexError::Cancelled);
                }
                self.nodes[cursor].other_ids.insert(id);
                self.entries += 1;
                return Ok(());
            }
            match self.nodes[cursor]
                .children
                .binary_search_by_key(&distance, |&(d, _)| d)
            {
                Ok(position) => cursor = self.nodes[cursor].children[position].1,
                Err(position) => {
                    self.admit_entry(max_entries)?;
                    local::reserve_slot(&mut self.nodes[cursor].children, 64)
                        .map_err(|_| IndexError::Budget)?;
                    local::reserve_slot(&mut self.nodes, max_entries).map_err(|_| IndexError::Budget)?;
                    let node = new_node(id, hash);
                    if cancel() {
                        return Err(IndexError::Cancelled);
                    }
                    let next = self.nodes.len();
                    self.nodes.push(node);
                    self.nodes[cursor].children.insert(position, (distance, next));
                    self.entries += 1;
                    return Ok(());
                }
            }
        }
    }

    fn admit_entry(&self, maximum: usize) -> Result<(), IndexError> {
        if self.entries >= maximum || self.entries.checked_add(1).is_none() {
            return Err(IndexError::Budget);
        }
        Ok(())
    }

    /// Return all fingerprints within `radius` bits, in (distance, id) order.
    /// Radius above 64 includes every entry. No transitive grouping is implied.
    ///
    /// # Panics
    /// Panics on allocation failure. Use `try_search_with_cancel` for fallible search.
    pub fn search(&self, hash: u64, radius: u32) -> Vec<Candidate> {
        self.search_with_cancel(hash, radius, || false)
            .unwrap_or_default()
    }

    /// Same complete search with cancellation at every node and collision id.
    ///
    /// # Errors
    /// Returns cancellation instead of partial results.
    ///
    /// # Panics
    /// Panics on allocation failure. Use `try_search_with_cancel` for fallible search.
    pub fn search_with_cancel(
        &self,
        hash: u64,
        radius: u32,
        cancel: impl Fn() -> bool,
    ) -> Result<Vec<Candidate>, SearchCancelled> {
        match self.try_search_with_cancel(hash, radius, usize::MAX, cancel) {
            Ok(results) => Ok(results),
            Err(IndexError::Cancelled) => Err(SearchCancelled),
            Err(IndexError::Budget) => panic!("metric-index search allocation failed"),
        }
    }

    /// Complete search with fallible traversal/result storage and a result cap.
    ///
    /// # Errors
    /// Cancellation, allocation failure or result-budget exhaustion; no partial results.
    pub fn try_search_with_cancel(
        &self,
        hash: u64,
        radius: u32,
        max_results: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Vec<Candidate>, IndexError> {
        let mut remaining = usize::MAX;
        self.try_search_with_work_budget(hash, radius, max_results, &mut remaining, cancel)
            .and_then(|results| results.ok_or(IndexError::Budget))
    }

    // None means traversal work was exhausted, never a partial candidate list.
    pub(crate) fn try_search_with_work_budget(
        &self,
        hash: u64,
        radius: u32,
        max_results: usize,
        remaining: &mut usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Option<Vec<Candidate>>, IndexError> {
        let mut results = Vec::new();
        if cancel() {
            return Err(IndexError::Cancelled);
        }
        if self.nodes.is_empty() {
            return Ok(Some(results));
        }
        let radius = radius.min(64);
        let mut stack = Vec::new();
        local::reserve_slot(&mut stack, self.nodes.len()).map_err(|_| IndexError::Budget)?;
        stack.push(0);
        while let Some(cursor) = stack.pop() {
            if cancel() {
                return Err(IndexError::Cancelled);
            }
            if *remaining == 0 {
                return Ok(None);
            }
            *remaining -= 1;
            let node = &self.nodes[cursor];
            let distance = (hash ^ node.hash).count_ones();
            if distance <= radius {
                for id in std::iter::once(node.first_id).chain(node.other_ids.iter().copied()) {
                    if cancel() {
                        return Err(IndexError::Cancelled);
                    }
                    if *remaining == 0 {
                        return Ok(None);
                    }
                    *remaining -= 1;
                    local::reserve_slot(&mut results, max_results).map_err(|_| IndexError::Budget)?;
                    results.push(Candidate { id, distance });
                }
            }
            let lower = distance.saturating_sub(radius);
            let upper = distance.saturating_add(radius);
            for &(d, child) in &node.children {
                if cancel() {
                    return Err(IndexError::Cancelled);
                }
                if *remaining == 0 {
                    return Ok(None);
                }
                *remaining -= 1;
                if (lower..=upper).contains(&d) {
                    local::reserve_slot(&mut stack, self.nodes.len()).map_err(|_| IndexError::Budget)?;
                    stack.push(child);
                }
            }
        }
        results.sort_unstable_by_key(|candidate| (candidate.distance, candidate.id));
        if cancel() {
            return Err(IndexError::Cancelled);
        }
        Ok(Some(results))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_work_exhaustion_is_distinct_from_failure() {
        let mut index = HammingIndex::new();
        for id in 0..20 {
            index.insert(id, 0);
        }
        for budget in 0..21 {
            assert_eq!(
                index.try_search_with_work_budget(0, 0, 20, &mut { budget }, || false),
                Ok(None)
            );
        }
        assert_eq!(
            index
                .try_search_with_work_budget(0, 0, 20, &mut 21, || false)
                .unwrap()
                .unwrap()
                .len(),
            20
        );
        assert_eq!(
            index.try_search_with_work_budget(0, 0, 0, &mut 21, || false),
            Err(IndexError::Budget)
        );
        assert_eq!(
            index.try_search_with_work_budget(0, 0, 20, &mut 0, || true),
            Err(IndexError::Cancelled)
        );
    }

    #[test]
    fn collision_membership_keeps_arbitrary_order_and_pair_identity() {
        let mut index = HammingIndex::new();
        let count = 2048_u64;
        for ordinal in 0..count {
            let id = ordinal.wrapping_mul(997) % count;
            index
                .try_insert_with_cancel(id, 0, count as usize, || false)
                .unwrap();
        }
        let expected = (0..count)
            .map(|id| Candidate { id, distance: 0 })
            .collect::<Vec<_>>();
        assert_eq!(index.search(0, 0), expected);
        // Repeated IDs remain no-ops at capacity, while a new ID is refused.
        for id in (0..count).rev() {
            index
                .try_insert_with_cancel(id, 0, count as usize, || false)
                .unwrap();
        }
        assert_eq!(
            index.try_insert_with_cancel(count, 0, count as usize, || false),
            Err(IndexError::Budget)
        );
        assert_eq!(index.search(0, 0), expected);
        // Membership is per fingerprint: the same ID at another hash is retained.
        index
            .try_insert_with_cancel(0, u64::MAX, count as usize + 1, || false)
            .unwrap();
        assert_eq!(index.search(u64::MAX, 0), vec![Candidate { id: 0, distance: 0 }]);
        assert_eq!(index.search(0, 0), expected);
    }

    #[test]
    fn empty_and_duplicate_fingerprints() {
        let mut index = HammingIndex::new();
        assert!(index.search(0, 64).is_empty());
        index.insert(9, 0);
        index.insert(2, 0);
        index.insert(2, 0);
        index.insert(3, u64::MAX);
        assert_eq!(
            index.search(0, 0),
            vec![Candidate { id: 2, distance: 0 }, Candidate { id: 9, distance: 0 }]
        );
        assert_eq!(index.search(0, u32::MAX).len(), 3);
    }

    #[test]
    fn indexed_search_matches_independent_exhaustive_scan() {
        let mut values = vec![0, u64::MAX, 1, 2, 3, 0];
        let mut seed = 0x1234_5678_9abc_def0_u64;
        for _ in 0..600 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            values.push(seed);
        }
        let mut index = HammingIndex::new();
        for (id, &hash) in values.iter().enumerate() {
            index.insert(id as u64, hash);
        }
        for query in values.iter().step_by(17).copied().chain([0xaaaa_aaaa_aaaa_aaaa]) {
            for radius in [0, 1, 4, 8, 16, 32, 63, 64, u32::MAX] {
                let mut expected: Vec<_> = values
                    .iter()
                    .enumerate()
                    .filter_map(|(id, &value)| {
                        let distance = (query ^ value).count_ones();
                        (distance <= radius).then_some(Candidate {
                            id: id as u64,
                            distance,
                        })
                    })
                    .collect();
                expected.sort_unstable_by_key(|candidate| (candidate.distance, candidate.id));
                assert_eq!(index.search(query, radius), expected);
            }
        }
    }
}

#[cfg(feature = "raster")]
pub mod correspondence_union;
