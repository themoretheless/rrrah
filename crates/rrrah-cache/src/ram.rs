//! In-RAM decoded-mosaic LRU sitting in front of the disk cache.
//!
//! The cache is deliberately single-owner: the foreground loader thread owns
//! it outright, so there is no locking anywhere on the load path. The pixel
//! payload is shared with the renderer through `Arc`, making hits an O(1)
//! clone. The currently displayed frame is pinned so background admission can
//! never evict the visible image under memory pressure.

use rrrah_core::DecodedMosaic;

use crate::CacheKey;
use rrrah_memory::{CacheLease, LeaseCache};

/// Default in-RAM budget: ~2 GiB of decoded mosaic pixels.
pub const DEFAULT_RAM_CACHE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Byte-weighted LRU over decoded mosaics keyed by the same [`CacheKey`] as
/// the disk cache.
#[derive(Debug)]
pub struct MosaicRamCache {
    inner: LeaseCache<CacheKey, DecodedMosaic>,
    /// Key of the frame currently on screen, pinned against eviction.
    visible: Option<CacheKey>,
    swap: Option<crate::MosaicSwapCache>,
}

impl MosaicRamCache {
    pub fn new(capacity_bytes: u64) -> Self {
        Self::with_limits(rrrah_memory::CacheLimits::bytes(capacity_bytes))
    }

    pub fn with_limits(limits: rrrah_memory::CacheLimits) -> Self {
        Self {
            inner: LeaseCache::new(limits),
            visible: None,
            swap: None,
        }
    }

    /// Transfers expired, unleased and unpinned entries to the bounded swap queue.
    /// Returns removed membership count; queued writes may still be refused or pending.
    pub fn spill_expired(&mut self) -> usize {
        let victims = self.inner.drain_expired();
        let count = victims.len();
        if let Some(swap) = &self.swap {
            for (key, frame) in victims {
                swap.enqueue(key, frame);
            }
        }
        count
    }

    pub fn capacity(&self) -> u64 {
        self.inner.capacity()
    }

    /// Updates limits without evicting the visible frame. Returned victims may
    /// be spilled or dropped by the caller; failure leaves the cache unchanged.
    pub fn set_limits(
        &mut self,
        limits: rrrah_memory::CacheLimits,
    ) -> Option<Vec<(CacheKey, DecodedMosaic)>> {
        self.inner.set_limits(limits)
    }

    /// Applies limits and offers victims to the bounded swap queue.
    /// Success means policy acceptance, not persistence; refused writes drop.
    pub fn set_limits_and_spill(&mut self, limits: rrrah_memory::CacheLimits) -> bool {
        let Some(victims) = self.set_limits(limits) else {
            return false;
        };
        if let Some(swap) = &self.swap {
            for (key, frame) in victims {
                swap.enqueue(key, frame);
            }
        }
        true
    }

    pub fn resident_weight(&self) -> u64 {
        self.inner.resident_weight()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Look up a mosaic, promoting it in recency. Returns a cheap `Arc` clone
    /// of the resident entry.
    pub fn get(&mut self, key: &CacheKey) -> Option<DecodedMosaic> {
        self.get_with_cancel(key, || false)
    }

    /// RAM-only lookup for callers selecting native decode instead of disk I/O.
    pub fn get_resident(&mut self, key: &CacheKey) -> Option<DecodedMosaic> {
        self.spill_expired();
        self.inner.get_cloned(key)
    }

    /// Resident-only consumer lease; protects replacement and background eviction
    /// until the last lease drops, independently of the visible-frame pin.
    pub fn get_lease(&mut self, key: &CacheKey) -> Option<CacheLease<DecodedMosaic>> {
        self.spill_expired();
        self.inner.get(key)
    }

    pub fn enable_swap(&mut self, swap: crate::MosaicSwapCache) {
        self.swap = Some(swap);
    }

    pub fn get_with_cancel(
        &mut self,
        key: &CacheKey,
        cancelled: impl FnMut() -> bool,
    ) -> Option<DecodedMosaic> {
        self.get_mode(key, true, cancelled)
    }
    /// Speculative restore preserves the visible pin and does not evict RAM
    /// entries to satisfy temporary allocation pressure.
    pub fn get_background_with_cancel(
        &mut self,
        key: &CacheKey,
        cancelled: impl FnMut() -> bool,
    ) -> Option<DecodedMosaic> {
        self.get_mode(key, false, cancelled)
    }
    fn get_mode(
        &mut self,
        key: &CacheKey,
        visible: bool,
        mut cancelled: impl FnMut() -> bool,
    ) -> Option<DecodedMosaic> {
        if cancelled() {
            return None;
        }
        self.spill_expired();
        if let Some(mosaic) = self.inner.get_cloned(key) {
            return Some(mosaic);
        }
        loop {
            match self.swap.as_ref()?.try_get(key, &mut cancelled) {
                Ok(Some(mosaic)) => {
                    if visible {
                        self.insert_visible(*key, mosaic.clone());
                    } else {
                        self.insert(*key, mosaic.clone());
                    }
                    return Some(mosaic);
                }
                Err(rrrah_memory::BufferError::Capacity { requested, limit, .. })
                    if visible && requested <= limit =>
                {
                    if !self.release_lru_unpinned() {
                        return None;
                    }
                }
                _ => return None,
            }
        }
    }

    pub fn swap(&self) -> Option<&crate::MosaicSwapCache> {
        self.swap.as_ref()
    }

    /// Admit a mosaic. Eviction never displaces the pinned visible frame; if
    /// every resident byte is pinned the new entry is rejected instead.
    /// Returns whether the entry was admitted.
    pub fn insert(&mut self, key: CacheKey, mosaic: DecodedMosaic) -> bool {
        self.spill_expired();
        let weight = mosaic.pixels.capacity_bytes();
        // Lease-aware admission preserves explicit visible pins and consumer leases.
        let (admitted, evicted) = match self.inner.insert(key, mosaic, weight) {
            Ok(victims) => (true, victims),
            Err(_) => (false, Vec::new()),
        };
        if let Some(swap) = &self.swap {
            for (key, mosaic) in evicted {
                swap.enqueue(key, mosaic);
            }
        }
        if admitted && self.visible == Some(key) {
            self.inner.pin(&key);
        }
        admitted
    }

    /// Foreground replacement may release the previous visible pin. Failed
    /// admission restores its pin and preserves the existing cache contents.
    pub fn insert_visible(&mut self, key: CacheKey, mosaic: DecodedMosaic) -> bool {
        let previous = self.visible;
        self.mark_visible(&key);
        if self.insert(key, mosaic) {
            true
        } else {
            self.inner.unpin(&key);
            self.visible = None;
            if let Some(previous) = previous {
                self.mark_visible(&previous);
            }
            false
        }
    }

    /// Pin `key` as the currently displayed frame and unpin the previous one.
    /// A key that is not (or no longer) resident simply records the intent; a
    /// later successful `insert` of the same key pins it.
    pub fn mark_visible(&mut self, key: &CacheKey) {
        if self.visible == Some(*key) {
            self.inner.pin(key);
            return;
        }
        if let Some(previous) = self.visible.take() {
            self.inner.unpin(&previous);
        }
        self.visible = Some(*key);
        self.inner.pin(key);
    }

    /// Drops one unpinned LRU entry for allocation admission pressure.
    /// Does not enqueue a spill, which would retain the very pixels being released.
    /// External owners may keep its allocation alive after this returns true.
    pub fn release_lru_unpinned(&mut self) -> bool {
        if let Some(swap) = &self.swap {
            swap.discard_pending_writes();
        }
        self.inner.take_lru().is_some()
    }

    pub fn visible(&self) -> Option<CacheKey> {
        self.visible
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rrrah_core::{
        CfaColor, CfaPattern, DecodedMosaic, LevelGrid, Orientation, Photometric, RawMetadata, WhiteLevel,
    };

    use super::*;

    fn key(byte: u8) -> CacheKey {
        CacheKey::from_bytes_for_test(byte)
    }

    fn mosaic(pixels: usize) -> DecodedMosaic {
        DecodedMosaic::new(
            RawMetadata {
                make: "Test".into(),
                model: "Ram".into(),
                width: pixels as u32,
                height: 1,
                components_per_pixel: 1,
                bits_per_sample: 14,
                photometric: Photometric::Cfa,
                cfa: Some(CfaPattern {
                    width: 2,
                    height: 2,
                    cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
                }),
                black_level: LevelGrid {
                    width: 1,
                    height: 1,
                    components: 1,
                    values: vec![0.0],
                },
                white_level: WhiteLevel(vec![16_383.0]),
                white_balance: [1.0, 1.0, 1.0, 1.0],
                xyz_to_camera: [[0.0; 3]; 4],
                active_area: None,
                crop_area: None,
                orientation: Orientation::Normal,
            },
            Arc::new(vec![42_u16; pixels]),
        )
        .unwrap()
    }

    #[test]
    fn hit_returns_shared_pixels_and_promotes_recency() {
        // Capacity fits two 4-byte mosaics; a third must evict the LRU.
        let mut cache = MosaicRamCache::new(8);
        assert!(cache.insert(key(1), mosaic(2)));
        assert!(cache.insert(key(2), mosaic(2)));

        let hit = cache.get(&key(1)).expect("resident entry must hit");
        assert!(hit.pixels.ptr_eq(&cache.get(&key(1)).unwrap().pixels));

        assert!(cache.insert(key(3), mosaic(2)));
        assert!(cache.get(&key(2)).is_none(), "un-promoted entry evicted");
        assert!(cache.get(&key(1)).is_some(), "promoted entry survived");
        assert!(cache.get(&key(3)).is_some());
    }

    #[test]
    fn pin_prevents_eviction_of_visible_frame() {
        let mut cache = MosaicRamCache::new(8);
        assert!(cache.insert(key(1), mosaic(2)));
        cache.mark_visible(&key(1));
        assert!(cache.insert(key(2), mosaic(2)));

        // Admitting a third frame must evict the unpinned entry, never the
        // pinned visible one.
        assert!(cache.insert(key(3), mosaic(2)));
        assert!(cache.get(&key(1)).is_some(), "visible frame survived");
        assert!(cache.get(&key(2)).is_none(), "unpinned LRU evicted");
        assert!(cache.get(&key(3)).is_some());

        // Moving the visible pin releases the old frame for eviction.
        cache.mark_visible(&key(3));
        assert!(cache.insert(key(4), mosaic(2)));
        assert!(cache.get(&key(1)).is_none(), "previous frame unpinned");
        assert!(cache.get(&key(3)).is_some());
        assert!(cache.get(&key(4)).is_some());
    }

    #[test]
    fn admission_is_rejected_when_all_resident_bytes_are_pinned() {
        let mut cache = MosaicRamCache::new(4);
        assert!(cache.insert(key(1), mosaic(2)));
        cache.mark_visible(&key(1));
        assert!(
            !cache.insert(key(2), mosaic(2)),
            "must not displace the visible frame"
        );
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn oversized_entry_is_rejected() {
        let mut cache = MosaicRamCache::new(2);
        assert!(!cache.insert(key(1), mosaic(2)));
        assert!(cache.is_empty());
    }

    #[test]
    fn visible_intent_before_admission_pins_the_later_value() {
        let mut cache = MosaicRamCache::new(4);
        cache.mark_visible(&key(1));
        assert!(cache.insert(key(1), mosaic(2)));
        assert!(!cache.insert(key(2), mosaic(2)));
        assert!(cache.get(&key(1)).is_some());
        cache.mark_visible(&key(2));
        assert!(cache.insert(key(2), mosaic(2)));
        assert!(cache.get(&key(1)).is_none());
        assert!(!cache.insert(key(3), mosaic(2)));
    }

    #[test]
    fn raw_adapter_enforces_count_and_expired_pins() {
        let mut cache = MosaicRamCache::with_limits(crate::CacheLimits {
            max_bytes: 8,
            max_entries: Some(1),
            ttl: None,
        });
        assert!(cache.insert(key(1), mosaic(2)));
        cache.mark_visible(&key(1));
        assert!(!cache.insert(key(2), mosaic(2)));
        cache.mark_visible(&key(2));
        assert!(cache.insert(key(2), mosaic(2)));
        assert!(cache.get(&key(1)).is_none());
        assert!(cache.get(&key(2)).is_some());
        let mut cache = MosaicRamCache::with_limits(crate::CacheLimits {
            max_bytes: 8,
            max_entries: Some(1),
            ttl: Some(std::time::Duration::ZERO),
        });
        assert!(cache.insert(key(1), mosaic(2)));
        cache.mark_visible(&key(1));
        assert!(cache.get(&key(1)).is_none());
        assert_eq!(cache.resident_weight(), 4);
        assert!(!cache.insert(key(2), mosaic(2)));
        cache.mark_visible(&key(2));
        assert!(cache.insert(key(2), mosaic(2)));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn consumer_lease_survives_navigation_and_blocks_spill_until_last_release() {
        let (_parent, mut cache) = swap_cache(64, 2);
        cache.set_limits_and_spill(crate::CacheLimits::bytes(8));
        assert!(cache.insert_visible(key(1), mosaic(2)));
        let lease = cache.get_lease(&key(1)).unwrap();
        let second = lease.clone();
        assert!(cache.insert_visible(key(2), mosaic(2)));
        assert!(!cache.set_limits_and_spill(crate::CacheLimits::bytes(4)));
        assert!(!cache.insert(key(1), mosaic(2)));
        assert!(!cache.insert(key(3), mosaic(2)));
        assert!(!cache.release_lru_unpinned());
        drop(lease);
        assert!(!cache.set_limits_and_spill(crate::CacheLimits::bytes(4)));
        let original = second.pixels.clone();
        drop(second);
        assert!(cache.set_limits_and_spill(crate::CacheLimits::bytes(4)));
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(cache.swap().unwrap().stats().writes, 1);
        assert_eq!(cache.visible(), Some(key(2)));
        assert_eq!(
            cache
                .get_background_with_cancel(&key(1), || false)
                .unwrap()
                .pixels,
            original
        );
        assert_eq!(cache.len(), 1);
    }
    #[test]
    fn foreground_count_one_replaces_current_and_failure_restores_its_pin() {
        let mut cache = MosaicRamCache::with_limits(crate::CacheLimits {
            max_bytes: 4,
            max_entries: Some(1),
            ttl: None,
        });
        assert!(cache.insert_visible(key(1), mosaic(2)));
        assert!(cache.insert_visible(key(2), mosaic(2)));
        assert_eq!(cache.visible(), Some(key(2)));
        assert!(cache.get(&key(1)).is_none());
        assert!(cache.get(&key(2)).is_some());
        assert!(!cache.insert_visible(key(3), mosaic(3)));
        assert_eq!(cache.visible(), Some(key(2)));
        assert!(cache.get(&key(2)).is_some());
        assert!(!cache.insert(key(4), mosaic(2)));
    }
    #[test]
    fn expired_raw_is_offered_to_swap_before_new_admission() {
        let (_parent, mut cache) = swap_cache(64, 2);
        let limits = crate::CacheLimits {
            max_bytes: 64,
            max_entries: Some(2),
            ttl: Some(std::time::Duration::ZERO),
        };
        assert!(cache.set_limits_and_spill(limits));
        let original = mosaic(2);
        assert!(cache.insert(key(1), original.clone()));
        // Admission triggers TTL spill, even though the byte/count caps have headroom.
        assert!(cache.insert(key(2), mosaic(2)));
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(cache.swap().unwrap().stats().writes, 1);
        assert!(cache.set_limits_and_spill(crate::CacheLimits::bytes(64)));
        let restored = cache.get(&key(1)).unwrap();
        assert_eq!(restored.pixels, original.pixels);
        assert_eq!(restored.metadata, original.metadata);
    }
    fn swap_cache(queue_bytes: u64, count: usize) -> (tempfile::TempDir, MosaicRamCache) {
        let parent = tempfile::tempdir().unwrap();
        let swap = crate::MosaicSwapCache::new(
            parent.path(),
            crate::MosaicSwapConfig {
                limits: crate::CacheLimits {
                    max_bytes: 4096,
                    max_entries: Some(count),
                    ttl: None,
                },
                queue_bytes,
                queue_count: 4,
                restore_bytes: 8,
            },
        )
        .unwrap();
        let mut cache = MosaicRamCache::with_limits(crate::CacheLimits {
            max_bytes: 4,
            max_entries: Some(1),
            ttl: None,
        });
        cache.enable_swap(swap);
        (parent, cache)
    }
    #[test]
    fn shrinking_limits_spills_only_background_raw_and_restores_exact_samples() {
        let (_parent, mut cache) = swap_cache(64, 2);
        assert!(cache.set_limits_and_spill(crate::CacheLimits::bytes(8)));
        let background = mosaic(2);
        assert!(cache.insert_visible(key(1), mosaic(2)));
        assert!(cache.insert(key(2), background.clone()));
        assert!(!cache.set_limits_and_spill(crate::CacheLimits::bytes(3)));
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.swap().unwrap().stats().writes, 0);
        assert!(cache.set_limits_and_spill(crate::CacheLimits {
            max_bytes: 4,
            max_entries: Some(1),
            ttl: None,
        }));
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(cache.visible(), Some(key(1)));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.swap().unwrap().stats().writes, 1);
        let restored = cache.get_background_with_cancel(&key(2), || false).unwrap();
        assert_eq!(restored.pixels, background.pixels);
        assert_eq!(restored.metadata, background.metadata);
        assert_eq!(cache.visible(), Some(key(1)));
        assert_eq!(cache.len(), 1);
    }
    #[test]
    fn background_raw_restore_preserves_visible_pin_and_refuses_pressure_eviction() {
        let (_parent, mut cache) = swap_cache(64, 2);
        let original = mosaic(2);
        assert!(cache.insert_visible(key(1), original.clone()));
        assert!(cache.insert_visible(key(2), mosaic(2)));
        cache.swap().unwrap().wait_idle().unwrap();
        let restored = cache.get_background_with_cancel(&key(1), || false).unwrap();
        assert_eq!(restored.pixels, original.pixels);
        assert_eq!(cache.visible(), Some(key(2)));
        assert!(cache.inner.get(&key(2)).is_some());
        assert!(cache.inner.get(&key(1)).is_none()); // count-one visible pin refuses speculative admission
        assert!(!cache.release_lru_unpinned());
        drop(restored);
        cache
            .swap
            .as_mut()
            .unwrap()
            .set_restore_budget(rrrah_memory::MemoryBudget::new(0));
        assert!(cache.get_background_with_cancel(&key(1), || false).is_none());
        assert_eq!(cache.visible(), Some(key(2)));
        assert_eq!(cache.len(), 1);
    }
    #[test]
    fn evicted_raw_is_restored_exactly_and_readmitted_without_a_decoder() {
        let (_parent, mut cache) = swap_cache(64, 2);
        let mut original = mosaic(2);
        original.metadata.model = "original".into();
        assert!(cache.insert_visible(key(1), original.clone()));
        assert!(cache.insert_visible(key(2), mosaic(2)));
        cache.swap().unwrap().wait_idle().unwrap();
        assert!(cache.inner.get(&key(1)).is_none());
        assert_eq!(cache.swap().unwrap().stats().writes, 1);
        assert!(cache.get_resident(&key(1)).is_none());
        assert_eq!(
            cache.swap().unwrap().stats().reads,
            0,
            "RAM-only lookup must not restore swap"
        );
        assert!(cache.get_resident(&key(2)).is_some());
        assert_eq!(cache.swap().unwrap().stats().reads, 0);
        let restored = cache.get_with_cancel(&key(1), || false).unwrap();
        assert_eq!(restored.metadata, original.metadata);
        assert_eq!(restored.pixels, original.pixels);
        assert!(cache.inner.get(&key(1)).is_some());
        cache.swap().unwrap().wait_idle().unwrap();
        let stats = cache.swap().unwrap().stats();
        assert_eq!(stats.reads, 1);
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.queued_bytes, 0);
    }
    #[test]
    fn swap_queue_limits_cancelled_restore_and_disk_count_are_bounded() {
        let (_parent, mut cache) = swap_cache(0, 1);
        assert!(cache.insert_visible(key(1), mosaic(2)));
        assert!(cache.insert_visible(key(2), mosaic(2)));
        cache.swap().unwrap().wait_idle().unwrap();
        assert!(cache.get(&key(1)).is_none());
        assert_eq!(cache.swap().unwrap().stats().dropped, 1);
        let (_parent, mut cache) = swap_cache(64, 1);
        assert!(cache.insert_visible(key(1), mosaic(2)));
        assert!(cache.insert_visible(key(2), mosaic(2)));
        cache.swap().unwrap().wait_idle().unwrap();
        assert!(cache.get_with_cancel(&key(1), || true).is_none());
        assert!(cache.get(&key(1)).is_some());
        cache.swap().unwrap().wait_idle().unwrap();
        assert!(cache.get(&key(2)).is_some());
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(cache.swap().unwrap().stats().errors, 0);
    }
    #[test]
    fn restored_external_owner_holds_budget_and_pressure_preserves_swap_for_retry() {
        let (_parent, mut cache) = swap_cache(64, 2);
        let budget = rrrah_memory::MemoryBudget::new(4);
        let swap = cache.swap.as_mut().unwrap();
        swap.set_restore_budget(budget.clone());
        swap.enqueue(key(1), mosaic(2));
        swap.wait_idle().unwrap();
        swap.set_restore_budget(rrrah_memory::MemoryBudget::new(0));
        assert!(swap.get(&key(1), || false).is_none());
        assert_eq!(swap.stats().errors, 0);
        swap.set_restore_budget(budget.clone());
        let first = swap.get(&key(1), || false).unwrap();
        assert!(first.pixels.is_managed());
        let owner = first.clone();
        drop(first);
        assert_eq!(budget.used(), 4);
        assert!(swap.get(&key(1), || false).is_none());
        assert_eq!(swap.stats().errors, 0);
        drop(owner);
        assert_eq!(budget.used(), 0);
        let retry = swap.get(&key(1), || false).unwrap();
        assert_eq!(retry.pixels[0], 42);
        drop(retry);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn spare_pixel_capacity_is_charged_by_ram_and_swap_queue() {
        let mut oversized = mosaic(2);
        let mut pixels = Vec::with_capacity(64);
        pixels.extend_from_slice(&[42_u16, 42]);
        oversized.pixels = Arc::new(pixels).into();
        assert_eq!(oversized.byte_len(), 4);
        assert!(oversized.pixels.capacity_bytes() >= 128);
        let mut ram = MosaicRamCache::new(4);
        assert!(!ram.insert_visible(key(1), oversized.clone()));
        assert!(ram.is_empty());
        let (_parent, cache) = swap_cache(8, 2);
        let swap = cache.swap().unwrap();
        swap.enqueue(key(1), oversized);
        swap.wait_idle().unwrap();
        assert_eq!(swap.stats().dropped, 1);
        assert_eq!(swap.stats().queued_bytes, 0);
        assert_eq!(swap.stats().writes, 0);
        assert_eq!(swap.stats().errors, 0);
    }
    #[test]
    fn allocation_pressure_releases_only_unpinned_entries_and_respects_external_owners() {
        let budget = rrrah_memory::MemoryBudget::new(24);
        let mut cache = MosaicRamCache::new(24);
        let visible = mosaic(4).try_manage_pixels(&budget).unwrap();
        cache.insert_visible(key(1), visible);
        let background = mosaic(4).try_manage_pixels(&budget).unwrap();
        let external = background.clone();
        cache.insert(key(2), background);
        cache.insert(key(3), mosaic(4).try_manage_pixels(&budget).unwrap());
        assert_eq!(budget.used(), 24);
        assert!(cache.release_lru_unpinned());
        assert_eq!(budget.used(), 24);
        assert!(cache.release_lru_unpinned());
        assert_eq!(budget.used(), 16);
        assert!(!cache.release_lru_unpinned());
        assert_eq!(cache.visible(), Some(key(1)));
        assert!(cache.get(&key(1)).is_some());
        drop(external);
        assert_eq!(budget.used(), 8);
        drop(cache);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn swap_restore_pressure_releases_unpinned_ram_and_preserves_impossible_admission() {
        let (_parent, mut cache) = swap_cache(64, 2);
        let budget = rrrah_memory::MemoryBudget::new(8);
        cache.swap.as_mut().unwrap().set_restore_budget(budget.clone());
        cache.swap().unwrap().enqueue(key(1), mosaic(4));
        cache.swap().unwrap().wait_idle().unwrap();
        cache.inner = LeaseCache::new(crate::CacheLimits::bytes(16));
        assert!(cache.insert(key(2), mosaic(4).try_manage_pixels(&budget).unwrap()));
        assert_eq!(budget.used(), 8);
        let restored = cache.get(&key(1)).unwrap();
        assert!(cache.inner.get(&key(2)).is_none());
        assert_eq!(cache.swap().unwrap().stats().errors, 0);
        assert_eq!(budget.used(), 8);
        drop(restored);
        // A zero local limit cannot be helped by eviction: keep the visible RAM entry.
        cache
            .swap
            .as_mut()
            .unwrap()
            .set_restore_budget(rrrah_memory::MemoryBudget::new(0));
        cache.swap().unwrap().enqueue(key(3), mosaic(4));
        cache.swap().unwrap().wait_idle().unwrap();
        let resident = cache.resident_weight();
        assert!(cache.get(&key(3)).is_none());
        assert_eq!(cache.resident_weight(), resident);
        assert_eq!(cache.swap().unwrap().stats().errors, 0);
        cache
            .swap
            .as_mut()
            .unwrap()
            .set_restore_budget(rrrah_memory::MemoryBudget::new(8));
        // Aggregate pressure permits evicting the unpinned old-generation frame.
        assert!(cache.get(&key(3)).is_none()); // visible frame remains protected
        let visible = cache.visible.take().unwrap();
        cache.inner.unpin(&visible);
        assert!(cache.get(&key(3)).is_some());
        assert!(cache.inner.get(&key(1)).is_none());
        assert_eq!(budget.used(), 0);
    }
}
