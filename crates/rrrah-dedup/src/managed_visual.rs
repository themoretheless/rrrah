//! Budget-owned complete-descriptor retrieval with exact final comparison.
use crate::{
    managed_metric::ManagedMetricIndex,
    pixels::Fingerprint,
    visual::{VisualCandidate, VisualError},
};
use rrrah_core::{MemoryBudget, Reservation, SharedBuffer};

#[derive(Debug)]
pub(crate) struct ManagedVisualIndex {
    entries: Vec<(u64, Fingerprint)>,
    _credit: Reservation,
    owners: SharedBuffer<usize>,
    channels: [ManagedMetricIndex; 4],
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn build(
        entries: Vec<(u64, Fingerprint)>,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<ManagedVisualIndex, VisualError> {
        let credit = budget
            .try_reserve((entries.capacity() * std::mem::size_of::<(u64, Fingerprint)>()) as u64)
            .map_err(|_| VisualError::Budget)?;
        ManagedVisualIndex::new(entries, credit, budget, cancel)
    }
    #[test]
    fn managed_complete_recipe_matches_independent_comparison_and_cancellation() {
        let entries: Vec<_> = (0u64..24)
            .map(|i| {
                let base = i.wrapping_mul(0x9e3779b97f4a7c15);
                let mut variants = [[
                    base,
                    base.rotate_left(11),
                    base.rotate_left(29),
                    base.rotate_left(47),
                ]; 8];
                if i % 2 == 0 {
                    variants[1] = [base ^ 255, base ^ 65535, base.rotate_left(3), !base];
                }
                (
                    if i == 23 { u64::MAX } else { i },
                    Fingerprint {
                        variants,
                        mean_linear_rgb: [0.; 3],
                        luminance_stddev: 1.,
                    },
                )
            })
            .collect();
        let budget = MemoryBudget::new(1024 * 1024);
        let calls = Cell::new(0usize);
        let index = build(entries.clone(), &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        let construction_calls = calls.get();
        let retained = budget.used();
        assert!(retained > 0);
        for (_, query) in entries.iter().step_by(5) {
            for transforms in [false, true] {
                for radius in [0, 1, 4, 64, 128, 255, 256, u32::MAX] {
                    let mut expected: Vec<_> = entries
                        .iter()
                        .filter_map(|(id, other)| {
                            let evidence = query.compare(other, transforms);
                            (evidence.distance <= radius.min(256))
                                .then_some(VisualCandidate { id: *id, evidence })
                        })
                        .collect();
                    expected.sort_unstable_by_key(|v| (v.evidence.distance, v.id));
                    let actual = index
                        .search(query, radius, transforms, &budget, || false)
                        .unwrap();
                    assert_eq!(&*actual, expected.as_slice());
                    drop(actual);
                    assert_eq!(budget.used(), retained);
                }
            }
        }
        calls.set(0);
        drop(
            index
                .search(&entries[0].1, 64, true, &budget, || {
                    calls.set(calls.get() + 1);
                    false
                })
                .unwrap(),
        );
        let search_calls = calls.get();
        for stop in 1..=search_calls {
            calls.set(0);
            assert!(matches!(
                index.search(&entries[0].1, 64, true, &budget, || {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }),
                Err(VisualError::Cancelled)
            ));
            assert_eq!(budget.used(), retained);
        }
        drop(index);
        assert_eq!(budget.used(), 0);
        for stop in 1..=construction_calls {
            calls.set(0);
            assert!(matches!(
                build(entries.clone(), &budget, || {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }),
                Err(VisualError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
        let empty = MemoryBudget::new(0);
        let index = build(Vec::new(), &empty, || false).unwrap();
        assert!(
            index
                .search(&entries[0].1, 0, true, &empty, || false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(empty.used(), 0);
        let mut duplicates = entries.clone();
        duplicates[1].0 = duplicates[0].0;
        assert!(matches!(
            build(duplicates, &budget, || false),
            Err(VisualError::DuplicateId)
        ));
        assert_eq!(budget.used(), 0);
    }
}
#[derive(Debug)]
pub(crate) struct ManagedVisualCandidates {
    storage: SharedBuffer<VisualCandidate>,
    length: usize,
}
impl std::ops::Deref for ManagedVisualCandidates {
    type Target = [VisualCandidate];
    fn deref(&self) -> &[VisualCandidate] {
        &self.storage[..self.length]
    }
}
fn mapped(error: crate::IndexError) -> VisualError {
    match error {
        crate::IndexError::Budget => VisualError::Budget,
        crate::IndexError::Cancelled => VisualError::Cancelled,
    }
}
impl ManagedVisualIndex {
    /// Takes already admitted fingerprint storage; retains its credit with entries.
    pub(crate) fn new(
        mut entries: Vec<(u64, Fingerprint)>,
        mut credit: Reservation,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, VisualError> {
        if cancel() {
            return Err(VisualError::Cancelled);
        }
        let bytes = entries
            .capacity()
            .checked_mul(std::mem::size_of::<(u64, Fingerprint)>())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(VisualError::Budget)?;
        credit.ensure_bytes(bytes).map_err(|_| VisualError::Budget)?;
        entries.sort_unstable_by_key(|e| e.0);
        if entries.windows(2).any(|v| v[0].0 == v[1].0) {
            return Err(VisualError::DuplicateId);
        }
        let mut count = 0usize;
        for (_, fingerprint) in &entries {
            for (i, variant) in fingerprint.variants.iter().enumerate() {
                if cancel() {
                    return Err(VisualError::Cancelled);
                }
                if !fingerprint.variants[..i].contains(variant) {
                    count = count.checked_add(1).ok_or(VisualError::Budget)?;
                }
            }
        }
        u64::try_from(count).map_err(|_| VisualError::Budget)?;
        let mut owners = budget
            .try_buffer(count, 0usize)
            .map_err(|_| VisualError::Budget)?;
        let mut hashes = budget
            .try_buffer(count, (0u64, 0u64))
            .map_err(|_| VisualError::Budget)?;
        let mut channels: [Option<ManagedMetricIndex>; 4] = std::array::from_fn(|_| None);
        for channel in 0..4 {
            let mut slot = 0;
            for (owner, (_, fingerprint)) in entries.iter().enumerate() {
                for (i, variant) in fingerprint.variants.iter().enumerate() {
                    if cancel() {
                        return Err(VisualError::Cancelled);
                    }
                    if fingerprint.variants[..i].contains(variant) {
                        continue;
                    }
                    owners[slot] = owner;
                    hashes[slot] = (
                        u64::try_from(slot).map_err(|_| VisualError::Budget)?,
                        variant[channel],
                    );
                    slot += 1;
                }
            }
            channels[channel] = Some(ManagedMetricIndex::new(&hashes, budget, &cancel).map_err(mapped)?);
        }
        if cancel() {
            return Err(VisualError::Cancelled);
        }
        drop(hashes);
        Ok(Self {
            entries,
            _credit: credit,
            owners: owners.freeze(),
            channels: channels.map(|v| v.expect("all channels constructed")),
        })
    }
    pub(crate) fn search(
        &self,
        query: &Fingerprint,
        radius: u32,
        allow_transforms: bool,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<ManagedVisualCandidates, VisualError> {
        if cancel() {
            return Err(VisualError::Cancelled);
        }
        let radius = radius.min(256);
        let mut selected = budget
            .try_buffer(self.entries.len(), radius == 256)
            .map_err(|_| VisualError::Budget)?;
        if radius < 256 {
            let mut remaining = self.entries.len().saturating_mul(4);
            for (channel, hash) in self.channels.iter().zip(query.variants[0]) {
                let hits = channel
                    .search(
                        hash,
                        radius / 4,
                        self.owners.len(),
                        &mut remaining,
                        budget,
                        &cancel,
                    )
                    .map_err(mapped)?;
                let Some(hits) = hits else {
                    selected.fill(true);
                    break;
                };
                for hit in hits.iter() {
                    if cancel() {
                        return Err(VisualError::Cancelled);
                    }
                    let slot = usize::try_from(hit.id).map_err(|_| VisualError::Budget)?;
                    selected[self.owners[slot]] = true;
                }
            }
        }
        let count = selected.iter().filter(|v| **v).count();
        let placeholder = VisualCandidate {
            id: 0,
            evidence: crate::pixels::Comparison {
                distance: 0,
                right_transform: 0,
                informative: false,
            },
        };
        let mut storage = budget
            .try_buffer(count, placeholder)
            .map_err(|_| VisualError::Budget)?;
        let mut length = 0;
        for (i, (id, fingerprint)) in self.entries.iter().enumerate() {
            if cancel() {
                return Err(VisualError::Cancelled);
            }
            if !selected[i] {
                continue;
            }
            let evidence = query.compare(fingerprint, allow_transforms);
            if evidence.distance <= radius {
                storage[length] = VisualCandidate { id: *id, evidence };
                length += 1;
            }
        }
        storage[..length].sort_unstable_by_key(|v| (v.evidence.distance, v.id));
        if cancel() {
            return Err(VisualError::Cancelled);
        }
        Ok(ManagedVisualCandidates {
            storage: storage.freeze(),
            length,
        })
    }
}
