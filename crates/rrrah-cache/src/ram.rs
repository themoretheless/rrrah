//! In-RAM decoded-mosaic LRU sitting in front of the disk cache.
//!
//! The cache is deliberately single-owner: the foreground loader thread owns
//! it outright, so there is no locking anywhere on the load path. The pixel
//! payload is shared with the renderer through `Arc`, making hits an O(1)
//! clone. The currently displayed frame is pinned so background admission can
//! never evict the visible image under memory pressure.

use rrrah_core::DecodedMosaic;

use crate::{CacheKey, weighted_lru::WeightedLru};

/// Default in-RAM budget: ~2 GiB of decoded mosaic pixels.
pub const DEFAULT_RAM_CACHE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Byte-weighted LRU over decoded mosaics keyed by the same [`CacheKey`] as
/// the disk cache.
#[derive(Debug)]
pub struct MosaicRamCache {
    inner: WeightedLru<CacheKey, DecodedMosaic>,
    /// Key of the frame currently on screen, pinned against eviction.
    visible: Option<CacheKey>,
}

impl MosaicRamCache {
    pub fn new(capacity_bytes: u64) -> Self {
        Self {
            inner: WeightedLru::new(capacity_bytes),
            visible: None,
        }
    }

    pub fn capacity(&self) -> u64 {
        self.inner.capacity()
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
        self.inner.get(key).cloned()
    }

    /// Admit a mosaic. Eviction never displaces the pinned visible frame; if
    /// every resident byte is pinned the new entry is rejected instead.
    /// Returns whether the entry was admitted.
    pub fn insert(&mut self, key: CacheKey, mosaic: DecodedMosaic) -> bool {
        let weight = u64::try_from(mosaic.byte_len()).unwrap_or(u64::MAX);
        // `insert_prefetch` is the pin-aware admission path: `WeightedLru`'s
        // plain `insert` ignores protection when evicting.
        self.inner.insert_prefetch(key, mosaic, weight)
    }

    /// Pin `key` as the currently displayed frame and unpin the previous one.
    /// A key that is not (or no longer) resident simply records the intent; a
    /// later `insert` of the same key re-pins it via `pin_if_visible`.
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
        assert!(Arc::ptr_eq(&hit.pixels, &cache.get(&key(1)).unwrap().pixels));

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
}
