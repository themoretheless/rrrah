//! Lazy flat pair-count storage; allocation admitted before first insertion.
use crate::local::LocalError;
use rrrah_core::{MemoryBudget, MutableBuffer};
use std::hash::{Hash, Hasher};
#[derive(Debug, Clone, Copy)]
struct Entry {
    left: u64,
    right: u64,
    count: usize,
    occupied: bool,
}
const EMPTY: Entry = Entry {
    left: 0,
    right: 0,
    count: 0,
    occupied: false,
};
#[derive(Debug)]
pub(crate) struct ManagedPairCounts {
    entries: Option<MutableBuffer<Entry>>,
    used: usize,
    limit: usize,
    budget: MemoryBudget,
}
impl ManagedPairCounts {
    pub(crate) fn new(limit: usize, budget: &MemoryBudget) -> Self {
        Self {
            entries: None,
            used: 0,
            limit,
            budget: budget.clone(),
        }
    }
    fn find_slot(
        entries: &[Entry],
        left: u64,
        right: u64,
        cancel: &impl Fn() -> bool,
    ) -> Result<usize, LocalError> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (left, right).hash(&mut hasher);
        let mut slot = (hasher.finish() as usize) & (entries.len() - 1);
        for _ in 0..entries.len() {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            let entry = entries[slot];
            if !entry.occupied || (entry.left == left && entry.right == right) {
                return Ok(slot);
            }
            slot = (slot + 1) & (entries.len() - 1);
        }
        Err(LocalError::Budget)
    }
    pub(crate) fn add(&mut self, left: u64, right: u64, cancel: impl Fn() -> bool) -> Result<(), LocalError> {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        let existing_slot = if let Some(entries) = self.entries.as_ref() {
            let slot = Self::find_slot(entries, left, right, &cancel)?;
            if entries[slot].occupied {
                let next = entries[slot].count.checked_add(1).ok_or(LocalError::Budget)?;
                self.entries.as_mut().expect("existing table")[slot].count = next;
                return Ok(());
            }
            Some(slot)
        } else {
            None
        };
        if self.used == self.limit {
            return Err(LocalError::Budget);
        }
        let capacity = self.entries.as_ref().map_or(0, |v| v.len());
        let value = Entry {
            left,
            right,
            count: 1,
            occupied: true,
        };
        if capacity == 0 || self.used >= capacity / 2 {
            let next_capacity = if capacity == 0 {
                2
            } else {
                capacity.checked_mul(2).ok_or(LocalError::Budget)?
            };
            // Both old and new allocations remain credited until the transfer commits.
            let mut next = self
                .budget
                .try_buffer(next_capacity, EMPTY)
                .map_err(|_| LocalError::Budget)?;
            if let Some(entries) = self.entries.as_ref() {
                for entry in entries.iter().copied() {
                    if cancel() {
                        return Err(LocalError::Cancelled);
                    }
                    if entry.occupied {
                        let slot = Self::find_slot(&next, entry.left, entry.right, &cancel)?;
                        next[slot] = entry;
                    }
                }
            }
            let slot = Self::find_slot(&next, left, right, &cancel)?;
            next[slot] = value;
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            self.entries = Some(next);
        } else {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            self.entries.as_mut().expect("existing table")[existing_slot.expect("vacant slot")] = value;
        }
        self.used += 1;
        Ok(())
    }
    pub(crate) fn iter(&self) -> PairCountIter<'_> {
        PairCountIter {
            entries: self.entries.as_ref().map_or(&[], |v| &v[..]),
            position: 0,
        }
    }
}
pub(crate) struct PairCountIter<'a> {
    entries: &'a [Entry],
    position: usize,
}
impl Iterator for PairCountIter<'_> {
    type Item = ((u64, u64), usize);
    fn next(&mut self) -> Option<Self::Item> {
        while self.position < self.entries.len() {
            let entry = self.entries[self.position];
            self.position += 1;
            if entry.occupied {
                return Some(((entry.left, entry.right), entry.count));
            }
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pair_table_matches_independent_counts_and_admits_before_allocating() {
        let budget = MemoryBudget::new(1024 * 1024);
        let mut counts = ManagedPairCounts::new(32, &budget);
        assert_eq!(budget.used(), 0);
        let mut expected = std::collections::BTreeMap::new();
        for repeat in 0..4 {
            for i in 0u64..32 {
                let pair = (i, if i == 31 { u64::MAX } else { i + 100 });
                counts.add(pair.0, pair.1, || false).unwrap();
                *expected.entry(pair).or_insert(0usize) += 1;
                if repeat == 0 {
                    assert!(budget.used() > 0);
                }
            }
        }
        assert_eq!(
            counts.iter().collect::<std::collections::BTreeMap<_, _>>(),
            expected
        );
        assert!(matches!(counts.add(999, 1000, || false), Err(LocalError::Budget)));
        counts.add(0, 100, || false).unwrap();
        assert_eq!(counts.iter().find(|v| v.0 == (0, 100)).unwrap().1, 5);
        assert!(matches!(counts.add(0, 100, || true), Err(LocalError::Cancelled)));
        drop(counts);
        assert_eq!(budget.used(), 0);
        let short = MemoryBudget::new((2 * std::mem::size_of::<Entry>()) as u64 - 1);
        let mut counts = ManagedPairCounts::new(1, &short);
        assert!(matches!(
            counts.add(0, u64::MAX, || false),
            Err(LocalError::Budget)
        ));
        assert_eq!(short.used(), 0);
        assert_eq!(short.peak(), 0);
        let empty = MemoryBudget::new(0);
        let mut counts = ManagedPairCounts::new(0, &empty);
        assert!(counts.iter().next().is_none());
        assert!(matches!(counts.add(0, 1, || false), Err(LocalError::Budget)));
        assert_eq!(empty.used(), 0);
    }
    #[test]
    fn collision_chain_cancellation_preserves_counts_at_every_checkpoint() {
        // Independent bucket selection forces all keys through the same chain.
        let keys: Vec<_> = (0..10000u64)
            .filter_map(|id| {
                let pair = (id, u64::MAX - id);
                let mut h = std::collections::hash_map::DefaultHasher::new();
                pair.hash(&mut h);
                (h.finish() & 15 == 0).then_some(pair)
            })
            .take(8)
            .collect();
        assert_eq!(keys.len(), 8);
        for checkpoint in 0..9 {
            let budget = MemoryBudget::new(4096);
            let mut table = ManagedPairCounts::new(8, &budget);
            for &(left, right) in &keys {
                table.add(left, right, || false).unwrap();
            }
            let before = table.iter().collect::<std::collections::BTreeMap<_, _>>();
            let calls = std::cell::Cell::new(0);
            let result = table.add(keys[7].0, keys[7].1, || {
                let current = calls.get();
                calls.set(current + 1);
                current == checkpoint
            });
            assert!(
                matches!(result, Err(LocalError::Cancelled)),
                "checkpoint {checkpoint}"
            );
            assert_eq!(table.iter().collect::<std::collections::BTreeMap<_, _>>(), before);
            table.add(keys[7].0, keys[7].1, || false).unwrap();
            assert_eq!(table.iter().find(|v| v.0 == keys[7]).unwrap().1, 2);
            drop(table);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn sparse_growth_admission_and_every_transfer_cancellation_are_atomic() {
        let entry_bytes = std::mem::size_of::<Entry>() as u64;
        let sparse = MemoryBudget::new(2 * entry_bytes);
        let mut table = ManagedPairCounts::new(usize::MAX, &sparse);
        table.add(0, u64::MAX, || false).unwrap();
        assert_eq!(sparse.used(), 2 * entry_bytes);
        assert!(matches!(table.add(1, 2, || false), Err(LocalError::Budget)));
        assert_eq!(table.iter().collect::<Vec<_>>(), vec![((0, u64::MAX), 1)]);
        table.add(0, u64::MAX, || false).unwrap();
        assert_eq!(table.iter().next().unwrap().1, 2);
        drop(table);
        assert_eq!(sparse.used(), 0);
        let run = |stop: Option<usize>| {
            let budget = MemoryBudget::new(1024);
            let mut table = ManagedPairCounts::new(usize::MAX, &budget);
            table.add(0, u64::MAX, || false).unwrap();
            let before = table.iter().collect::<Vec<_>>();
            let calls = std::cell::Cell::new(0usize);
            let result = table.add(1, 2, || {
                let n = calls.get();
                calls.set(n + 1);
                stop == Some(n)
            });
            if stop.is_some() {
                assert!(matches!(result, Err(LocalError::Cancelled)));
                assert_eq!(table.iter().collect::<Vec<_>>(), before);
                assert_eq!(budget.used(), 2 * entry_bytes);
            } else {
                result.unwrap();
                assert_eq!(table.used, 2);
                assert_eq!(budget.used(), 4 * entry_bytes);
                assert_eq!(budget.peak(), 6 * entry_bytes);
            }
            let checkpoints = calls.get();
            drop(table);
            assert_eq!(budget.used(), 0);
            checkpoints
        };
        let checkpoints = run(None);
        for stop in 0..checkpoints {
            run(Some(stop));
        }
    }
}
