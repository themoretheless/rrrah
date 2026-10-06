//! Indexed candidate retrieval for the complete 256-bit perceptual recipe.
//! A pigeonhole union over four 64-bit metric indexes cannot omit an accepted
//! candidate: sum(distance) <= R implies at least one channel <= floor(R / 4).

use crate::{
    HammingIndex, IndexError,
    pixels::{Comparison, Fingerprint},
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VisualError {
    #[error("duplicate application id")]
    DuplicateId,
    #[error("index exceeds supplied entry budget")]
    Budget,
    #[error("operation cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualCandidate {
    pub id: u64,
    pub evidence: Comparison,
}

/// Immutable index; rebuild when fingerprints change. Every application id has
/// exactly one recipe. Internal slots avoid arithmetic on application ids.
#[derive(Debug)]
pub struct VisualIndex {
    entries: HashMap<u64, Fingerprint>,
    slots: Vec<u64>,
    channels: [HammingIndex; 4],
}

impl VisualIndex {
    /// Index all eight transform variants under a finite entry budget.
    ///
    /// # Errors
    /// Rejects duplicate ids, entry excess, internal slot overflow or cancellation.
    pub fn new(
        entries: impl IntoIterator<Item = (u64, Fingerprint)>,
        max_entries: usize,
        cancel: impl Fn() -> bool,
    ) -> Result<Self, VisualError> {
        let mut index = Self {
            entries: HashMap::new(),
            slots: Vec::new(),
            channels: std::array::from_fn(|_| HammingIndex::new()),
        };
        for (id, fingerprint) in entries {
            if cancel() {
                return Err(VisualError::Cancelled);
            }
            if index.entries.contains_key(&id) {
                return Err(VisualError::DuplicateId);
            }
            if index.entries.len() >= max_entries {
                return Err(VisualError::Budget);
            }
            index.entries.try_reserve(1).map_err(|_| VisualError::Budget)?;
            for variant in fingerprint.variants {
                let slot = u64::try_from(index.slots.len()).map_err(|_| VisualError::Budget)?;
                crate::local::reserve_slot(&mut index.slots, max_entries.saturating_mul(8))
                    .map_err(|_| VisualError::Budget)?;
                index.slots.push(id);
                for (channel, hash) in index.channels.iter_mut().zip(variant) {
                    channel
                        .try_insert_with_cancel(slot, hash, max_entries.saturating_mul(8), &cancel)
                        .map_err(|error| match error {
                            IndexError::Budget => VisualError::Budget,
                            IndexError::Cancelled => VisualError::Cancelled,
                        })?;
                }
            }
            index.entries.insert(id, fingerprint);
        }
        if cancel() {
            return Err(VisualError::Cancelled);
        }
        Ok(index)
    }

    /// Retrieve candidates with exact complete-recipe distance, deterministic
    /// (distance,id) order and explicit low-information evidence. No identity or
    /// transitive grouping is implied. Radius >=256 includes every entry.
    /// Cancellation is checked during metric traversal, collision buckets and
    /// candidate comparison; no partial list is returned.
    ///
    /// # Errors
    /// Returns cancellation or allocation failure without a partial candidate list.
    pub fn search(
        &self,
        query: &Fingerprint,
        radius: u32,
        allow_transforms: bool,
        cancel: impl Fn() -> bool,
    ) -> Result<Vec<VisualCandidate>, VisualError> {
        let radius = radius.min(256);
        if radius == 256 {
            return self.search_all(query, radius, allow_transforms, cancel);
        }
        // Bound total tree visits, child checks and collision-bucket emissions.
        // Exhaustion switches to complete comparison; allocation errors do not.
        let mut remaining = self.entries.len().saturating_mul(4);
        let mut ids = HashSet::new();
        for (channel, hash) in self.channels.iter().zip(query.variants[0]) {
            if cancel() {
                return Err(VisualError::Cancelled);
            }
            let hits = channel
                .try_search_with_work_budget(hash, radius / 4, self.slots.len(), &mut remaining, &cancel)
                .map_err(|error| match error {
                    IndexError::Budget => VisualError::Budget,
                    IndexError::Cancelled => VisualError::Cancelled,
                })?;
            let Some(hits) = hits else {
                return self.search_all(query, radius, allow_transforms, &cancel);
            };
            for hit in hits {
                if cancel() {
                    return Err(VisualError::Cancelled);
                }
                if let Ok(slot) = usize::try_from(hit.id)
                    && let Some(&id) = self.slots.get(slot)
                    && !ids.contains(&id)
                {
                    ids.try_reserve(1).map_err(|_| VisualError::Budget)?;
                    ids.insert(id);
                }
            }
        }
        let mut ordered = Vec::new();
        ordered
            .try_reserve_exact(ids.len())
            .map_err(|_| VisualError::Budget)?;
        ordered.extend(ids);
        ordered.sort_unstable();
        let mut matches = Vec::new();
        for id in ordered {
            if cancel() {
                return Err(VisualError::Cancelled);
            }
            if let Some(other) = self.entries.get(&id) {
                let evidence = query.compare(other, allow_transforms);
                if evidence.distance <= radius {
                    crate::local::reserve_slot(&mut matches, self.entries.len())
                        .map_err(|_| VisualError::Budget)?;
                    matches.push(VisualCandidate { id, evidence });
                }
            }
        }
        matches.sort_unstable_by_key(|candidate| (candidate.evidence.distance, candidate.id));
        if cancel() {
            return Err(VisualError::Cancelled);
        }
        Ok(matches)
    }
    fn search_all(
        &self,
        query: &Fingerprint,
        radius: u32,
        allow_transforms: bool,
        cancel: impl Fn() -> bool,
    ) -> Result<Vec<VisualCandidate>, VisualError> {
        let mut matches = Vec::new();
        for (&id, other) in &self.entries {
            if cancel() {
                return Err(VisualError::Cancelled);
            }
            let evidence = query.compare(other, allow_transforms);
            if evidence.distance <= radius {
                crate::local::reserve_slot(&mut matches, self.entries.len())
                    .map_err(|_| VisualError::Budget)?;
                matches.push(VisualCandidate { id, evidence });
            }
        }
        matches.sort_unstable_by_key(|candidate| (candidate.evidence.distance, candidate.id));
        if cancel() {
            return Err(VisualError::Cancelled);
        }
        Ok(matches)
    }
}
