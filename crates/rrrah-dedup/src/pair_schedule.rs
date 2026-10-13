//! Prioritize descriptor proposals while retaining exhaustive file-pair coverage.
use crate::local::LocalError;
use rrrah_core::{MemoryBudget, Reservation, SharedBuffer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledPair {
    pub left: u64,
    pub right: u64,
    /// A proposal is a search hint, never positive or negative image evidence.
    pub proposed: bool,
}

/// Owns bounded file IDs and unique canonical proposals, not an n² pair table.
/// Include every file ID, including files with no extracted descriptors.
#[derive(Debug)]
pub struct CompletePairSchedule {
    ids: SharedBuffer<u64>,
    proposals: SharedBuffer<(u64, u64)>,
    total: u64,
}
fn copy_slots<T: Copy>(values: &[T], budget: &MemoryBudget) -> Result<(Vec<T>, Reservation), LocalError> {
    let bytes = values
        .len()
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    let mut credit = budget.try_reserve(bytes).map_err(|_| LocalError::Budget)?;
    let mut copy = Vec::new();
    copy.try_reserve_exact(values.len())
        .map_err(|_| LocalError::Budget)?;
    let actual = copy
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(LocalError::Budget)?;
    credit.ensure_bytes(actual).map_err(|_| LocalError::Budget)?;
    copy.extend_from_slice(values);
    Ok((copy, credit))
}
impl CompletePairSchedule {
    /// Proposals may be unordered and repeated, but each pair must have left < right
    /// and reference two declared file IDs. Duplicate file IDs refuse.
    /// `max_pairs` admits the complete unordered file-pair count, not only hints.
    /// `max_proposals` bounds the supplied list before deduplication.
    /// # Errors
    /// Invalid IDs/pairs, cancellation, allocation or any declared budget excess.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        file_ids: &[u64],
        proposals: &[(u64, u64)],
        max_files: usize,
        max_pairs: u64,
        max_proposals: usize,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, LocalError> {
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        if file_ids.len() > max_files || proposals.len() > max_proposals {
            return Err(LocalError::Budget);
        }
        let n = u64::try_from(file_ids.len()).map_err(|_| LocalError::Budget)?;
        let (a, b) = if n % 2 == 0 {
            (n / 2, n.saturating_sub(1))
        } else {
            (n, n.saturating_sub(1) / 2)
        };
        let total = a.checked_mul(b).ok_or(LocalError::Budget)?;
        if total > max_pairs {
            return Err(LocalError::Budget);
        }
        let (mut ids, ids_credit) = copy_slots(file_ids, budget)?;
        ids.sort_unstable();
        for (i, id) in ids.iter().enumerate() {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            if i > 0 && ids[i - 1] == *id {
                return Err(LocalError::Invalid);
            }
        }
        let (mut hints, hints_credit) = copy_slots(proposals, budget)?;
        for &(left, right) in &hints {
            if cancel() {
                return Err(LocalError::Cancelled);
            }
            if left >= right || ids.binary_search(&left).is_err() || ids.binary_search(&right).is_err() {
                return Err(LocalError::Invalid);
            }
        }
        hints.sort_unstable();
        hints.dedup();
        let ids = ids_credit.try_adopt(ids).map_err(|_| LocalError::Budget)?;
        let proposals = hints_credit.try_adopt(hints).map_err(|_| LocalError::Budget)?;
        if cancel() {
            return Err(LocalError::Cancelled);
        }
        Ok(Self {
            ids,
            proposals,
            total,
        })
    }
    pub fn file_ids(&self) -> &[u64] {
        &self.ids
    }
    pub fn total_pairs(&self) -> u64 {
        self.total
    }
    pub fn proposed_pairs(&self) -> usize {
        self.proposals.len()
    }
    /// Unique proposals first, then every remaining unordered pair in ID order.
    /// Drain successfully to establish complete enumeration. Cancellation yields
    /// one error and ends iteration; callers must discard incomplete aggregates.
    /// This iterator does not confirm images or roll back callback side effects.
    pub fn pairs<F: Fn() -> bool>(&self, cancel: F) -> CompletePairIter<'_, F> {
        CompletePairIter {
            schedule: self,
            cancel,
            hint: 0,
            left: 0,
            right: 1,
            done: false,
        }
    }
}

#[derive(Debug)]
pub struct CompletePairIter<'a, F> {
    schedule: &'a CompletePairSchedule,
    cancel: F,
    hint: usize,
    left: usize,
    right: usize,
    done: bool,
}
impl<F: Fn() -> bool> Iterator for CompletePairIter<'_, F> {
    type Item = Result<ScheduledPair, LocalError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if (self.cancel)() {
            self.done = true;
            return Some(Err(LocalError::Cancelled));
        }
        if self.hint < self.schedule.proposals.len() {
            let (left, right) = self.schedule.proposals[self.hint];
            self.hint += 1;
            return Some(Ok(ScheduledPair {
                left,
                right,
                proposed: true,
            }));
        }
        while self.left < self.schedule.ids.len().saturating_sub(1) {
            if (self.cancel)() {
                self.done = true;
                return Some(Err(LocalError::Cancelled));
            }
            if self.right >= self.schedule.ids.len() {
                self.left += 1;
                self.right = self.left + 1;
                continue;
            }
            let pair = (self.schedule.ids[self.left], self.schedule.ids[self.right]);
            self.right += 1;
            if self.schedule.proposals.binary_search(&pair).is_ok() {
                continue;
            }
            return Some(Ok(ScheduledPair {
                left: pair.0,
                right: pair.1,
                proposed: false,
            }));
        }
        self.done = true;
        if (self.cancel)() {
            Some(Err(LocalError::Cancelled))
        } else {
            None
        }
    }
}
impl<F: Fn() -> bool> std::iter::FusedIterator for CompletePairIter<'_, F> {}
