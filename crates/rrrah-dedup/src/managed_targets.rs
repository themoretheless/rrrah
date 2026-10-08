//! Reusable generation-marked target storage, with one counted ID per file.
use crate::local::LocalError;
use rrrah_core::{MemoryBudget, MutableBuffer};
#[derive(Debug)]
pub(crate) struct ManagedTargets {
    storage: MutableBuffer<u64>,
    files: usize,
    generation: u64,
    length: usize,
}
impl ManagedTargets {
    pub(crate) fn new(files: usize, budget: &MemoryBudget) -> Result<Self, LocalError> {
        let size = files.checked_mul(2).ok_or(LocalError::Budget)?;
        let storage = budget.try_buffer(size, 0u64).map_err(|_| LocalError::Budget)?;
        Ok(Self {
            storage,
            files,
            generation: 0,
            length: 0,
        })
    }
    pub(crate) fn begin(&mut self) -> Result<(), LocalError> {
        self.generation = self.generation.checked_add(1).ok_or(LocalError::Budget)?;
        self.length = 0;
        Ok(())
    }
    pub(crate) fn insert(&mut self, file: usize, id: u64) -> Result<(), LocalError> {
        if file >= self.files || self.generation == 0 {
            return Err(LocalError::Invalid);
        }
        if self.storage[file] != self.generation {
            self.storage[file] = self.generation;
            self.storage[self.files + self.length] = id;
            self.length += 1;
        }
        Ok(())
    }
    pub(crate) fn ordered(&mut self) -> &[u64] {
        self.storage[self.files..self.files + self.length].sort_unstable();
        &self.storage[self.files..self.files + self.length]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn targets_reuse_credit_and_preserve_unique_sorted_opaque_ids() {
        let budget = MemoryBudget::new(64);
        let mut targets = ManagedTargets::new(4, &budget).unwrap();
        assert_eq!(budget.used(), 64);
        assert!(matches!(targets.insert(0, 1), Err(LocalError::Invalid)));
        for _ in 0..32 {
            targets.begin().unwrap();
            for (i, id) in [(3, u64::MAX), (1, 7), (3, u64::MAX), (0, 0), (1, 7)] {
                targets.insert(i, id).unwrap();
            }
            assert_eq!(targets.ordered(), &[0, 7, u64::MAX]);
            assert_eq!(budget.peak(), 64);
            targets.begin().unwrap();
            targets.insert(2, 3).unwrap();
            assert_eq!(targets.ordered(), &[3]);
        }
        assert!(matches!(targets.insert(4, 9), Err(LocalError::Invalid)));
        drop(targets);
        assert_eq!(budget.used(), 0);
        let short = MemoryBudget::new(63);
        assert!(matches!(ManagedTargets::new(4, &short), Err(LocalError::Budget)));
        assert_eq!(short.used(), 0);
        assert_eq!(short.peak(), 0);
        let empty = MemoryBudget::new(0);
        let mut targets = ManagedTargets::new(0, &empty).unwrap();
        targets.begin().unwrap();
        assert!(targets.ordered().is_empty());
        assert_eq!(empty.used(), 0);
    }
}
