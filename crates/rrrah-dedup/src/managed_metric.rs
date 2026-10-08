//! Flat, budget-owned BK-tree storage for internal unique descriptor slots.
use crate::{Candidate, IndexError};
use rrrah_core::{MemoryBudget, SharedBuffer};

const NONE: usize = usize::MAX;
#[derive(Debug, Clone, Copy)]
struct Node {
    hash: u64,
    id: u64,
    collision: usize,
    child: usize,
    sibling: usize,
    distance: u32,
}
const EMPTY: Node = Node {
    hash: 0,
    id: 0,
    collision: NONE,
    child: NONE,
    sibling: NONE,
    distance: 0,
};

/// Internal IDs are unique slots supplied by the complete-descriptor index.
/// Each entry occupies one flat node, including equal-hash collision members.
#[derive(Debug)]
pub(crate) struct ManagedMetricIndex {
    nodes: SharedBuffer<Node>,
}
#[derive(Debug, Clone)]
pub(crate) struct ManagedCandidates {
    storage: SharedBuffer<Candidate>,
    length: usize,
}
impl std::ops::Deref for ManagedCandidates {
    type Target = [Candidate];
    fn deref(&self) -> &[Candidate] {
        &self.storage[..self.length]
    }
}
impl ManagedMetricIndex {
    pub(crate) fn new(
        entries: &[(u64, u64)],
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, IndexError> {
        if cancel() {
            return Err(IndexError::Cancelled);
        }
        let mut nodes = budget
            .try_buffer(entries.len(), EMPTY)
            .map_err(|_| IndexError::Budget)?;
        for (next, &(id, hash)) in entries.iter().enumerate() {
            if cancel() {
                return Err(IndexError::Cancelled);
            }
            let mut fresh = Node { id, hash, ..EMPTY };
            if next != 0 {
                let mut cursor = 0;
                loop {
                    if cancel() {
                        return Err(IndexError::Cancelled);
                    }
                    let distance = (hash ^ nodes[cursor].hash).count_ones();
                    if distance == 0 {
                        fresh.collision = nodes[cursor].collision;
                        nodes[cursor].collision = next;
                        break;
                    }
                    let mut child = nodes[cursor].child;
                    while child != NONE && nodes[child].distance != distance {
                        if cancel() {
                            return Err(IndexError::Cancelled);
                        }
                        child = nodes[child].sibling;
                    }
                    if child != NONE {
                        cursor = child;
                    } else {
                        fresh.distance = distance;
                        fresh.sibling = nodes[cursor].child;
                        nodes[cursor].child = next;
                        break;
                    }
                }
            }
            nodes[next] = fresh;
        }
        if cancel() {
            return Err(IndexError::Cancelled);
        }
        Ok(Self {
            nodes: nodes.freeze(),
        })
    }

    /// Same inclusive Hamming metric and deterministic output as the legacy tree.
    /// Returned results keep their credit until their last owner is released.
    pub(crate) fn search(
        &self,
        hash: u64,
        radius: u32,
        max_results: usize,
        remaining: &mut usize,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<Option<ManagedCandidates>, IndexError> {
        if cancel() {
            return Err(IndexError::Cancelled);
        }
        let mut stack = budget
            .try_buffer(self.nodes.len(), 0usize)
            .map_err(|_| IndexError::Budget)?;
        let capacity = self.nodes.len().min(max_results);
        let mut result = budget
            .try_buffer(capacity, Candidate { id: 0, distance: 0 })
            .map_err(|_| IndexError::Budget)?;
        let mut used = 0;
        let mut depth = usize::from(!self.nodes.is_empty());
        let radius = radius.min(64);
        while depth != 0 {
            if cancel() {
                return Err(IndexError::Cancelled);
            }
            if *remaining == 0 {
                return Ok(None);
            }
            *remaining -= 1;
            depth -= 1;
            let cursor = stack[depth];
            let node = self.nodes[cursor];
            let distance = (hash ^ node.hash).count_ones();
            if distance <= radius {
                let mut member = cursor;
                while member != NONE {
                    if cancel() {
                        return Err(IndexError::Cancelled);
                    }
                    if *remaining == 0 {
                        return Ok(None);
                    }
                    *remaining -= 1;
                    if used == capacity {
                        return Err(IndexError::Budget);
                    }
                    result[used] = Candidate {
                        id: self.nodes[member].id,
                        distance,
                    };
                    used += 1;
                    member = self.nodes[member].collision;
                }
            }
            let lower = distance.saturating_sub(radius);
            let upper = distance.saturating_add(radius);
            let mut child = node.child;
            while child != NONE {
                if cancel() {
                    return Err(IndexError::Cancelled);
                }
                if *remaining == 0 {
                    return Ok(None);
                }
                *remaining -= 1;
                let edge = self.nodes[child];
                if (lower..=upper).contains(&edge.distance) {
                    stack[depth] = child;
                    depth += 1;
                }
                child = edge.sibling;
            }
        }
        result[..used].sort_unstable_by_key(|v| (v.distance, v.id));
        if cancel() {
            return Err(IndexError::Cancelled);
        }
        // Retain admitted capacity without allocating a second result array.
        let result = result.freeze();
        Ok(Some(ManagedCandidates {
            storage: result,
            length: used,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_tree_equals_exhaustive_and_retains_results_credit() {
        let entries: Vec<_> = (0u64..96)
            .map(|id| {
                (
                    id,
                    if id % 3 == 0 {
                        0
                    } else {
                        id.wrapping_mul(0x9e3779b97f4a7c15)
                    },
                )
            })
            .collect();
        let budget = MemoryBudget::new(1024 * 1024);
        let index = ManagedMetricIndex::new(&entries, &budget, || false).unwrap();
        let retained = budget.used();
        assert!(retained > 0);
        for query in [0, u64::MAX, 0x12345678] {
            for radius in [0, 1, 7, 31, 63, 64, u32::MAX] {
                let mut expected: Vec<_> = entries
                    .iter()
                    .filter_map(|&(id, hash)| {
                        let distance = (hash ^ query).count_ones();
                        (distance <= radius.min(64)).then_some(Candidate { id, distance })
                    })
                    .collect();
                expected.sort_unstable_by_key(|v| (v.distance, v.id));
                let actual = index
                    .search(query, radius, entries.len(), &mut usize::MAX, &budget, || false)
                    .unwrap()
                    .unwrap();
                assert_eq!(&*actual, expected.as_slice());
                let shared = actual.clone();
                drop(actual);
                assert!(budget.used() > retained);
                drop(shared);
                assert_eq!(budget.used(), retained);
            }
        }
        assert!(
            index
                .search(0, 64, 96, &mut 0, &budget, || false)
                .unwrap()
                .is_none()
        );
        assert_eq!(budget.used(), retained);
        assert!(matches!(
            index.search(0, 64, 0, &mut usize::MAX, &budget, || false),
            Err(IndexError::Budget)
        ));
        assert_eq!(budget.used(), retained);
        assert!(matches!(
            index.search(0, 64, 96, &mut usize::MAX, &budget, || true),
            Err(IndexError::Cancelled)
        ));
        assert_eq!(budget.used(), retained);
        drop(index);
        assert_eq!(budget.used(), 0);
        let short = MemoryBudget::new((entries.len() * std::mem::size_of::<Node>()) as u64 - 1);
        assert!(matches!(
            ManagedMetricIndex::new(&entries, &short, || false),
            Err(IndexError::Budget)
        ));
        assert_eq!(short.used(), 0);
        assert_eq!(short.peak(), 0);
        let empty = MemoryBudget::new(0);
        let index = ManagedMetricIndex::new(&[], &empty, || false).unwrap();
        let result = index.search(0, 64, 0, &mut 0, &empty, || false).unwrap().unwrap();
        assert!(result.is_empty());
        assert_eq!(empty.used(), 0);
    }
}
