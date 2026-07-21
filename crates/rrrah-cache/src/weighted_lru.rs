use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

#[derive(Debug)]
struct Entry<V> {
    value: V,
    weight: u64,
    last_used: u64,
}

/// A small deterministic byte-weighted LRU.
///
/// Production tile admission is intended to move to TinyLFU/2Q, but this type
/// already enforces the important invariant: capacity is measured in bytes,
/// not object count.
#[derive(Debug)]
pub struct WeightedLru<K, V> {
    entries: HashMap<K, Entry<V>>,
    capacity: u64,
    resident: u64,
    clock: u64,
    /// Entries protected from eviction while they are visible/being decoded.
    /// This is intentionally separate from recency so background prefetch
    /// cannot evict the current frame under memory pressure.
    protected: HashSet<K>,
}

impl<K, V> WeightedLru<K, V>
where
    K: Clone + Eq + Hash,
{
    pub fn new(capacity: u64) -> Self {
        Self {
            entries: HashMap::new(),
            capacity,
            resident: 0,
            clock: 0,
            protected: HashSet::new(),
        }
    }

    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    pub fn resident_weight(&self) -> u64 {
        self.resident
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Protect an already resident key from eviction. Returns whether the key
    /// exists; callers may pin the current frame before scheduling prefetch.
    pub fn pin(&mut self, key: &K) -> bool {
        if self.entries.contains_key(key) {
            self.protected.insert(key.clone());
            true
        } else {
            false
        }
    }

    pub fn unpin(&mut self, key: &K) {
        self.protected.remove(key);
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.clock = self.clock.wrapping_add(1);
        let entry = self.entries.get_mut(key)?;
        entry.last_used = self.clock;
        Some(&entry.value)
    }

    /// Inserts an entry. Returns `false` when a single entry exceeds the hard
    /// budget and therefore must not be admitted.
    pub fn insert(&mut self, key: K, value: V, weight: u64) -> bool {
        if weight > self.capacity {
            return false;
        }
        self.clock = self.clock.wrapping_add(1);
        if let Some(previous) = self.entries.remove(&key) {
            self.resident = self.resident.saturating_sub(previous.weight);
        }
        while self.resident.saturating_add(weight) > self.capacity {
            if self.pop_lru().is_none() {
                break;
            }
        }
        self.resident = self.resident.saturating_add(weight);
        self.entries.insert(
            key,
            Entry {
                value,
                weight,
                last_used: self.clock,
            },
        );
        true
    }

    /// Insert a speculative/background value. Pinned entries are never
    /// evicted; if all resident bytes are pinned, the speculative value is
    /// rejected instead of displacing the visible frame.
    pub fn insert_prefetch(&mut self, key: K, value: V, weight: u64) -> bool {
        if weight > self.capacity {
            return false;
        }
        self.clock = self.clock.wrapping_add(1);
        if let Some(previous) = self.entries.remove(&key) {
            self.resident = self.resident.saturating_sub(previous.weight);
        }
        while self.resident.saturating_add(weight) > self.capacity {
            if self.pop_lru_unprotected().is_none() {
                return false;
            }
        }
        self.resident += weight;
        self.entries.insert(
            key,
            Entry {
                value,
                weight,
                last_used: self.clock,
            },
        );
        true
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let entry = self.entries.remove(key)?;
        self.resident = self.resident.saturating_sub(entry.weight);
        Some(entry.value)
    }

    fn pop_lru(&mut self) -> Option<(K, V)> {
        let key = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(key, _)| key.clone())?;
        let entry = self.entries.remove(&key)?;
        self.resident = self.resident.saturating_sub(entry.weight);
        Some((key, entry.value))
    }

    fn pop_lru_unprotected(&mut self) -> Option<(K, V)> {
        let key = self
            .entries
            .iter()
            .filter(|(k, _)| !self.protected.contains(*k))
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(key, _)| key.clone())?;
        let entry = self.entries.remove(&key)?;
        self.resident = self.resident.saturating_sub(entry.weight);
        Some((key, entry.value))
    }
}

#[cfg(test)]
mod tests {
    use super::WeightedLru;

    #[test]
    fn evicts_by_bytes_and_recency() {
        let mut cache = WeightedLru::new(10);
        assert!(cache.insert("a", 1, 4));
        assert!(cache.insert("b", 2, 4));
        assert_eq!(cache.get(&"a"), Some(&1));
        assert!(cache.insert("c", 3, 4));
        assert!(cache.get(&"b").is_none());
        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.get(&"c"), Some(&3));
        assert_eq!(cache.resident_weight(), 8);
    }

    #[test]
    fn rejects_an_entry_larger_than_budget() {
        let mut cache = WeightedLru::new(3);
        assert!(!cache.insert("large", 1, 4));
        assert!(cache.is_empty());
    }

    #[test]
    fn prefetch_does_not_evict_pinned_visible_frame() {
        let mut cache = WeightedLru::new(10);
        assert!(cache.insert("current", 1, 6));
        assert!(cache.pin(&"current"));
        assert!(cache.insert_prefetch("next", 2, 4));
        assert!(cache.insert_prefetch("far", 3, 4));
        assert_eq!(cache.get(&"current"), Some(&1));
        assert!(cache.get(&"next").is_none());
    }
}
