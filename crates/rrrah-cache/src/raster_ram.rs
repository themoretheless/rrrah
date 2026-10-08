//! Format-independent cache of decoded/prepared rasters.
//! Callers must include source identity, subimage and display transforms in K.
use rrrah_core::DecodedRaster;
use rrrah_memory::{CacheLease, CacheLimits, LeaseCache};
use std::hash::Hash;
#[derive(Debug)]
pub struct RasterRamCache<K> {
    entries: LeaseCache<K, DecodedRaster>,
    visible: Option<K>,
    swap: Option<crate::RasterSwapCache<K>>,
}
impl<K: Clone + Eq + Hash + Send + 'static> RasterRamCache<K> {
    pub fn new(limits: CacheLimits) -> Self {
        Self {
            entries: LeaseCache::new(limits),
            visible: None,
            swap: None,
        }
    }
    /// Transfers expired, unleased and unpinned entries to the bounded swap queue.
    /// Returns removed membership count; queued writes may still be refused or pending.
    pub fn spill_expired(&mut self) -> usize {
        let victims = self.entries.drain_expired();
        let count = victims.len();
        if let Some(swap) = &self.swap {
            for (key, frame) in victims {
                swap.enqueue(key, frame);
            }
        }
        count
    }

    pub fn get(&mut self, key: &K) -> Option<DecodedRaster> {
        self.get_with_cancel(key, || false)
    }
    /// Resident-only lease; protects the complete raster, including its ICC profile.
    pub fn get_lease(&mut self, key: &K) -> Option<CacheLease<DecodedRaster>> {
        self.spill_expired();
        self.entries.get(key)
    }
    /// Acquires the visible resident only when it shares the supplied frame's
    /// pixel allocation. Failed foreground admission must not lease the old frame.
    pub fn get_visible_lease_for(&mut self, raster: &DecodedRaster) -> Option<CacheLease<DecodedRaster>> {
        let key = self.visible.clone()?;
        let lease = self.get_lease(&key)?;
        let shared = match (lease.pixels(), raster.pixels()) {
            (rrrah_core::RasterPixels::Rgba8(a), rrrah_core::RasterPixels::Rgba8(b)) => a.ptr_eq(b),
            (rrrah_core::RasterPixels::Rgba16(a), rrrah_core::RasterPixels::Rgba16(b)) => a.ptr_eq(b),
            (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) => {
                a.ptr_eq(b)
            }
            _ => false,
        };
        (shared
            && lease.width() == raster.width()
            && lease.height() == raster.height()
            && lease.pixel_aspect() == raster.pixel_aspect()
            && lease.color_space() == raster.color_space())
        .then_some(lease)
    }
    /// Updates limits atomically while retaining visible-frame protection.
    /// Returned victims retain their memory credit until their last owner drops.
    pub fn set_limits(&mut self, limits: CacheLimits) -> Option<Vec<(K, DecodedRaster)>> {
        self.entries.set_limits(limits)
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

    pub fn enable_swap(&mut self, swap: crate::RasterSwapCache<K>) {
        self.swap = Some(swap);
    }
    pub fn swap(&self) -> Option<&crate::RasterSwapCache<K>> {
        self.swap.as_ref()
    }
    /// Loader-thread lookup; memory pressure releases only unpinned RAM entries.
    /// Pressure victims are not queued for spill, so they can release memory.
    pub fn get_with_cancel(&mut self, key: &K, cancelled: impl FnMut() -> bool) -> Option<DecodedRaster> {
        self.get_mode(key, true, cancelled)
    }
    /// Speculative restores do not transfer the visible pin to a neighbour.
    pub fn get_background_with_cancel(
        &mut self,
        key: &K,
        cancelled: impl FnMut() -> bool,
    ) -> Option<DecodedRaster> {
        self.get_mode(key, false, cancelled)
    }
    fn get_mode(
        &mut self,
        key: &K,
        visible: bool,
        mut cancelled: impl FnMut() -> bool,
    ) -> Option<DecodedRaster> {
        if cancelled() {
            return None;
        }
        self.spill_expired();
        if let Some(frame) = self.entries.get_cloned(key) {
            return (!cancelled()).then_some(frame);
        }
        loop {
            match self.swap.as_ref()?.try_get(key, &mut cancelled) {
                Ok(Some(frame)) => {
                    // A generation can change after stream verification but
                    // before RAM publication and visible-pin transfer.
                    if cancelled() {
                        return None;
                    }
                    if visible {
                        self.insert_visible(key.clone(), frame.clone());
                    } else {
                        self.insert(key.clone(), frame.clone());
                    }
                    return Some(frame);
                }
                Err(rrrah_memory::BufferError::Capacity { requested, limit, .. })
                    if visible && requested <= limit =>
                {
                    if cancelled() || !self.release_lru_unpinned() {
                        return None;
                    }
                }
                _ => return None,
            }
        }
    }
    /// Releases one unpinned LRU frame; external owners may retain its memory.
    pub fn release_lru_unpinned(&mut self) -> bool {
        if let Some(swap) = &self.swap {
            swap.discard_pending_writes();
        }
        self.entries.take_lru().is_some()
    }
    pub fn resident_bytes(&self) -> u64 {
        self.entries.resident_weight()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// Background admission preserves the current visible frame.
    pub fn insert(&mut self, key: K, raster: DecodedRaster) -> bool {
        self.spill_expired();
        let weight = raster.capacity_bytes();
        let visible = self.visible.as_ref() == Some(&key);
        let (admitted, evicted) = match self.entries.insert(key.clone(), raster, weight) {
            Ok(victims) => (true, victims),
            Err(_) => (false, Vec::new()),
        };
        if let Some(swap) = &self.swap {
            for (key, frame) in evicted {
                swap.enqueue(key, frame);
            }
        }
        if !admitted {
            return false;
        }
        if visible {
            self.entries.pin(&key);
        }
        true
    }
    pub fn mark_visible(&mut self, key: K) {
        if let Some(previous) = self.visible.take() {
            self.entries.unpin(&previous);
        }
        self.entries.pin(&key);
        self.visible = Some(key);
    }
    /// Allows transition at count one; failed admission restores the old pin.
    pub fn insert_visible(&mut self, key: K, raster: DecodedRaster) -> bool {
        let previous = self.visible.take();
        if let Some(old) = &previous {
            self.entries.unpin(old);
        }
        if self.insert(key.clone(), raster) {
            self.entries.pin(&key);
            self.visible = Some(key);
            true
        } else {
            if let Some(old) = &previous {
                self.entries.pin(old);
            }
            self.visible = previous;
            false
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use rrrah_core::{RasterColorSpace, RasterPixels};
    use std::sync::Arc;
    fn raster() -> DecodedRaster {
        DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![-0., 0.125, 128., 1.]).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
    }
    #[test]
    fn cancellation_after_ram_hit_does_not_return_a_stale_owner() {
        let root = rrrah_core::MemoryBudget::new(64);
        let mut cache = RasterRamCache::new(CacheLimits::bytes(64));
        assert!(cache.insert_visible(2u64, raster().try_manage_pixels(&root).unwrap()));
        let mut polls = 0;
        assert!(
            cache
                .get_with_cancel(&2, || {
                    polls += 1;
                    polls == 2
                })
                .is_none()
        );
        assert_eq!(polls, 2);
        assert_eq!(cache.visible, Some(2));
        assert_eq!(cache.len(), 1);
        assert_eq!(root.used(), 16);
        drop(cache);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn cancellation_after_swap_read_preserves_visible_pin_and_retry() {
        let parent = tempfile::tempdir().unwrap();
        let root = rrrah_core::MemoryBudget::new(128);
        let mut cache = RasterRamCache::new(CacheLimits::bytes(64));
        cache.enable_swap(
            crate::RasterSwapCache::new_with_budgets(
                parent.path(),
                crate::ImageSwapConfig {
                    limits: CacheLimits::bytes(4096),
                    queue_bytes: 64,
                    queue_count: 2,
                    restore_bytes: 64,
                },
                rrrah_core::MemoryBudget::new(64),
                root.clone(),
            )
            .unwrap(),
        );
        assert!(cache.insert_visible(2u64, raster().try_manage_pixels(&root).unwrap()));
        cache
            .swap()
            .unwrap()
            .enqueue(1, raster().try_manage_pixels(&root).unwrap());
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(root.used(), 16);
        let mut stream_polls = 0;
        drop(
            cache
                .swap()
                .unwrap()
                .try_get(&1, || {
                    stream_polls += 1;
                    false
                })
                .unwrap()
                .unwrap(),
        );
        assert_eq!(root.used(), 16);
        // RAM lookup adds an entry check and a final publication check to the
        // unchanged swap-read checkpoints. Cancel at that final boundary.
        let target = stream_polls + 2;
        let mut polls = 0;
        assert!(
            cache
                .get_with_cancel(&1, || {
                    polls += 1;
                    polls == target
                })
                .is_none()
        );
        assert_eq!(polls, target);
        assert_eq!(cache.visible, Some(2));
        assert_eq!(cache.len(), 1);
        assert_eq!(root.used(), 16);
        assert_eq!(cache.swap().unwrap().stats().errors, 0);
        let restored = cache.get_with_cancel(&1, || false).unwrap();
        assert_eq!(cache.visible, Some(1));
        let RasterPixels::Rgba32Float(values) = restored.pixels() else {
            panic!()
        };
        assert_eq!(
            values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            [-0.0f32, 0.125, 128.0, 1.0].map(f32::to_bits)
        );
        drop(restored);
        drop(cache);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn expired_hdr_raster_spills_before_new_admission() {
        let parent = tempfile::tempdir().unwrap();
        let mut cache = RasterRamCache::new(CacheLimits {
            max_bytes: 64,
            max_entries: Some(2),
            ttl: Some(std::time::Duration::ZERO),
        });
        cache.enable_swap(
            crate::RasterSwapCache::new_with_budgets(
                parent.path(),
                crate::ImageSwapConfig {
                    limits: CacheLimits::bytes(4096),
                    queue_bytes: 128,
                    queue_count: 2,
                    restore_bytes: 128,
                },
                rrrah_memory::MemoryBudget::new(128),
                rrrah_memory::MemoryBudget::new(128),
            )
            .unwrap(),
        );
        let original = raster();
        assert!(cache.insert(1, original.clone()));
        assert!(cache.insert(2, raster()));
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(cache.swap().unwrap().stats().writes, 1);
        assert!(cache.set_limits_and_spill(CacheLimits::bytes(64)));
        let restored = cache.get(&1).unwrap();
        let RasterPixels::Rgba32Float(values) = restored.pixels() else {
            panic!()
        };
        let RasterPixels::Rgba32Float(expected) = original.pixels() else {
            panic!()
        };
        assert_eq!(
            values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(restored.color_space(), original.color_space());
    }
    #[test]
    fn hdr_icc_leases_preserve_capacity_and_spill_after_last_release() {
        let parent = tempfile::tempdir().unwrap();
        let budget = rrrah_core::MemoryBudget::new(128);
        let profiled = || {
            DecodedRaster::new(
                1,
                1,
                RasterPixels::Rgba32Float(Arc::new(vec![-0.0, 2.0, -0.5, 1.0]).into()),
                RasterColorSpace::Icc(vec![7, 8, 9]),
            )
            .unwrap()
            .try_manage_pixels(&budget)
            .unwrap()
        };
        let mut cache = RasterRamCache::new(CacheLimits::bytes(38));
        cache.enable_swap(
            crate::RasterSwapCache::new_with_budgets(
                parent.path(),
                crate::ImageSwapConfig {
                    limits: CacheLimits::bytes(4096),
                    queue_bytes: 64,
                    queue_count: 2,
                    restore_bytes: 128,
                },
                budget.clone(),
                budget.clone(),
            )
            .unwrap(),
        );
        assert!(cache.insert_visible(1, profiled()));
        let lease = cache.get_lease(&1).unwrap();
        let second = lease.clone();
        assert!(cache.insert_visible(2, profiled()));
        assert_eq!(budget.used(), 38);
        assert!(!cache.set_limits_and_spill(CacheLimits::bytes(19)));
        assert!(!cache.insert(1, profiled()));
        assert!(!cache.release_lru_unpinned());
        drop(lease);
        assert!(!cache.set_limits_and_spill(CacheLimits::bytes(19)));
        let bits = match second.pixels() {
            RasterPixels::Rgba32Float(v) => v.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            _ => panic!(),
        };
        drop(second);
        assert!(cache.set_limits_and_spill(CacheLimits::bytes(19)));
        cache.swap().unwrap().wait_idle().unwrap();
        assert_eq!(budget.used(), 19);
        assert_eq!(cache.swap().unwrap().stats().writes, 1);
        let restored = cache.get_background_with_cancel(&1, || false).unwrap();
        assert_eq!(restored.color_space(), &RasterColorSpace::Icc(vec![7, 8, 9]));
        match restored.pixels() {
            RasterPixels::Rgba32Float(v) => {
                assert_eq!(v.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), bits)
            }
            _ => panic!(),
        }
        assert_eq!(cache.len(), 1);
        assert_eq!(budget.used(), 38);
        drop(restored);
        drop(cache);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn icc_weighted_admission_preserves_visible_and_external_owners() {
        for limits in [
            CacheLimits::bytes(19),
            CacheLimits {
                max_bytes: 38,
                max_entries: Some(1),
                ttl: None,
            },
        ] {
            let budget = rrrah_core::MemoryBudget::new(128);
            let profiled = |length| {
                DecodedRaster::new(
                    1,
                    1,
                    RasterPixels::Rgba32Float(Arc::new(vec![4.0, 2.0, -0.5, 1.0]).into()),
                    RasterColorSpace::Icc(vec![7; length]),
                )
                .unwrap()
                .try_manage_pixels(&budget)
                .unwrap()
            };
            let mut cache = RasterRamCache::new(limits);
            assert!(cache.insert_visible(1, profiled(3)));
            let held = cache.get(&1).unwrap();
            assert_eq!(cache.resident_bytes(), 19);
            assert_eq!(budget.used(), 19);
            assert!(!cache.insert_visible(2, profiled(40)));
            assert!(
                !cache.insert(3, profiled(3)),
                "failed transition must restore visible pin"
            );
            assert!(cache.get(&1).is_some());
            assert_eq!(budget.used(), 19);
            assert!(cache.insert_visible(2, profiled(3)));
            assert!(cache.get(&1).is_none());
            assert_eq!(cache.resident_bytes(), 19);
            assert_eq!(budget.used(), 38, "external owner keeps complete old allocation");
            drop(cache);
            assert_eq!(budget.used(), 19);
            drop(held);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn expired_icc_entry_releases_cache_owner_but_keeps_external_credit() {
        let budget = rrrah_core::MemoryBudget::new(19);
        let source = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![4.0, 2.0, -0.5, 1.0]).into()),
            RasterColorSpace::Icc(vec![1, 2, 3]),
        )
        .unwrap()
        .try_manage_pixels(&budget)
        .unwrap();
        let mut cache = RasterRamCache::new(CacheLimits {
            max_bytes: 19,
            max_entries: Some(1),
            ttl: Some(std::time::Duration::ZERO),
        });
        assert!(cache.insert(1, source.clone()));
        assert!(cache.get(&1).is_none());
        assert_eq!(cache.resident_bytes(), 0);
        assert_eq!(budget.used(), 19);
        drop(source);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn visible_transition_preserves_hdr_bits_and_external_owners() {
        let original = raster();
        let mut cache = RasterRamCache::new(CacheLimits {
            max_bytes: 16,
            max_entries: Some(1),
            ttl: None,
        });
        assert!(cache.insert_visible(1, original.clone()));
        let held = cache.get(&1).unwrap();
        assert!(!cache.insert(2, raster()));
        assert!(cache.insert_visible(2, raster()));
        assert!(cache.get(&1).is_none());
        let RasterPixels::Rgba32Float(values) = held.pixels() else {
            panic!()
        };
        let RasterPixels::Rgba32Float(expected) = original.pixels() else {
            panic!()
        };
        assert!(values.ptr_eq(expected));
        assert_eq!(
            values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(cache.resident_bytes(), 16);
    }
    #[test]
    fn failed_visible_admission_restores_pin_and_zero_limits_refuse_hits() {
        let mut cache = RasterRamCache::new(CacheLimits::bytes(16));
        assert!(cache.insert_visible(1, raster()));
        let large = DecodedRaster::new(
            2,
            1,
            RasterPixels::Rgba32Float(Arc::new(vec![1.; 8]).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        assert!(!cache.insert_visible(2, large.clone()));
        assert!(cache.get_visible_lease_for(&large).is_none());
        let current = cache.get(&1).unwrap();
        assert!(cache.get_visible_lease_for(&current).is_some());
        assert!(!cache.insert(3, raster()));
        assert!(cache.get(&1).is_some());
        let mut zero = RasterRamCache::new(CacheLimits {
            max_bytes: 16,
            max_entries: Some(0),
            ttl: None,
        });
        assert!(!zero.insert_visible(1, raster()));
        assert!(zero.is_empty());
        let mut expired = RasterRamCache::new(CacheLimits {
            max_bytes: 16,
            max_entries: None,
            ttl: Some(std::time::Duration::ZERO),
        });
        assert!(expired.insert_visible(1, raster()));
        assert!(expired.get(&1).is_none());
    }
}

#[cfg(test)]
mod managed_policy_tests {
    use super::*;
    use rrrah_core::{MemoryBudget, RasterColorSpace, RasterPixels};
    use std::{sync::Arc, time::Duration};
    fn managed(budget: &MemoryBudget) -> DecodedRaster {
        let mut pixels = Vec::with_capacity(16);
        pixels.extend([4.0f32, -0.0, 2.0, 1.0]);
        DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(pixels).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .try_manage_pixels(budget)
        .unwrap()
    }
    #[test]
    fn combined_limit_update_is_atomic_across_visible_pin_lease_and_ttl() {
        let budget = MemoryBudget::new(128);
        let mut cache = RasterRamCache::new(CacheLimits {
            max_bytes: 128,
            max_entries: Some(2),
            ttl: None,
        });
        assert!(cache.insert(1u8, managed(&budget)));
        let lease = cache.get_lease(&1).unwrap();
        assert!(cache.insert_visible(2, managed(&budget)));
        let tightened = CacheLimits {
            max_bytes: 64,
            max_entries: Some(1),
            ttl: Some(Duration::ZERO),
        };
        assert!(!cache.set_limits_and_spill(tightened));
        // Failed admission must not apply even the TTL part of the new policy.
        assert!(cache.get(&1).is_some());
        assert!(cache.get(&2).is_some());
        assert_eq!(
            (cache.len(), cache.resident_bytes(), budget.used()),
            (2, 128, 128)
        );
        drop(lease);
        assert!(cache.set_limits_and_spill(tightened));
        assert_eq!((cache.len(), cache.resident_bytes(), budget.used()), (1, 64, 64));
        assert!(cache.get(&1).is_none());
        // Existing entries keep their insertion deadlines when TTL is changed.
        assert!(cache.get(&2).is_some());
        assert!(cache.insert_visible(2, managed(&budget)));
        // Replacement uses the accepted new TTL and cannot be freshly looked up.
        assert!(cache.get(&2).is_none());
        assert_eq!(cache.spill_expired(), 0); // Visible allocation remains protected.
        cache.mark_visible(3);
        assert_eq!(cache.spill_expired(), 1);
        assert_eq!((cache.len(), cache.resident_bytes(), budget.used()), (0, 0, 0));
    }
    #[test]
    fn byte_and_count_eviction_preserve_external_managed_owners() {
        for limits in [
            CacheLimits {
                max_bytes: 64,
                max_entries: None,
                ttl: None,
            },
            CacheLimits {
                max_bytes: 128,
                max_entries: Some(1),
                ttl: None,
            },
        ] {
            let budget = MemoryBudget::new(128);
            let mut cache = RasterRamCache::new(limits);
            assert!(cache.insert(1, managed(&budget)));
            let held = cache.get(&1).unwrap();
            assert_eq!(held.pixel_capacity_bytes(), 64); // four initialized samples, sixteen allocated
            assert!(cache.insert(2, managed(&budget)));
            assert_eq!(cache.len(), 1);
            assert!(cache.get(&1).is_none());
            assert_eq!(cache.resident_bytes(), 64);
            assert_eq!(budget.used(), 128); // victim still held outside the cache
            let RasterPixels::Rgba32Float(values) = held.pixels() else {
                panic!()
            };
            assert_eq!(values[1].to_bits(), (-0.0f32).to_bits());
            drop(cache);
            assert_eq!(budget.used(), 64);
            drop(held);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn expired_visible_buffer_keeps_reservation_until_unpinned_and_last_owner_drops() {
        let budget = MemoryBudget::new(64);
        let frame = managed(&budget);
        let held = frame.clone();
        let mut cache = RasterRamCache::new(CacheLimits {
            max_bytes: 64,
            max_entries: Some(1),
            ttl: Some(Duration::ZERO),
        });
        assert!(cache.insert_visible(1, frame));
        assert!(cache.get(&1).is_none());
        assert!(!cache.release_lru_unpinned());
        assert_eq!(budget.used(), 64);
        assert_eq!(cache.len(), 1);
        cache.mark_visible(2);
        assert!(cache.release_lru_unpinned());
        assert!(cache.is_empty());
        assert_eq!(budget.used(), 64);
        drop(cache);
        assert_eq!(budget.used(), 64);
        drop(held);
        assert_eq!(budget.used(), 0);
    }
}

#[cfg(test)]
mod aspect_lease_tests {
    use super::*;
    #[test]
    fn visible_lease_requires_matching_geometry_even_for_shared_pixels() {
        let root = rrrah_core::MemoryBudget::new(4);
        let frame = DecodedRaster::new(
            1,
            1,
            rrrah_core::RasterPixels::Rgba8(std::sync::Arc::new(vec![1, 2, 3, 255]).into()),
            rrrah_core::RasterColorSpace::Srgb,
        )
        .unwrap()
        .with_pixel_aspect(Some(2.0))
        .unwrap()
        .try_manage_pixels(&root)
        .unwrap();
        let mut cache = RasterRamCache::new(CacheLimits::bytes(4));
        assert!(cache.insert_visible(1u8, frame.clone()));
        let altered = frame.clone().with_pixel_aspect(Some(1.0)).unwrap();
        assert!(cache.get_visible_lease_for(&altered).is_none());
        let lease = cache.get_visible_lease_for(&frame).unwrap();
        assert_eq!(lease.pixel_aspect(), Some(2.0));
        drop(altered);
        drop(frame);
        drop(cache);
        assert_eq!(root.used(), 4);
        drop(lease);
        assert_eq!(root.used(), 0);
    }
}
