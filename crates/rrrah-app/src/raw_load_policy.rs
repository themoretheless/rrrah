//! Bounded per-source/recipe foreground cost history; never uses extension guesses.
use rrrah_cache::CacheKey;
use std::{collections::HashMap, time::Duration};
const MAX_KEYS: usize = 64;
#[derive(Default)]
struct Cost {
    nanos: u64,
    samples: u64,
}
impl Cost {
    fn observe(&mut self, value: Duration) {
        let value = u64::try_from(value.as_nanos()).unwrap_or(u64::MAX);
        self.nanos = if self.samples == 0 {
            value
        } else {
            ((u128::from(self.nanos) * 3 + u128::from(value)) / 4) as u64
        };
        self.samples = self.samples.saturating_add(1);
    }
}
#[derive(Default)]
struct Entry {
    decode: Cost,
    restore: Cost,
    decode_first: bool,
    decode_deferred: bool,
    requests: u64,
    used: u64,
}
#[derive(Default)]
pub(super) struct RawLoadCosts {
    entries: HashMap<CacheKey, Entry>,
    clock: u64,
}
impl RawLoadCosts {
    fn entry(&mut self, key: CacheKey) -> &mut Entry {
        self.clock = self.clock.saturating_add(1);
        if !self.entries.contains_key(&key) && self.entries.len() == MAX_KEYS {
            let victim = *self.entries.iter().min_by_key(|(_, e)| e.used).unwrap().0;
            self.entries.remove(&victim);
        }
        let e = self.entries.entry(key).or_default();
        e.used = self.clock;
        e
    }
    pub fn decode_first(&mut self, key: CacheKey) -> bool {
        let e = self.entry(key);
        e.requests = e.requests.saturating_add(1);
        if e.decode.samples >= 3 && e.restore.samples >= 3 {
            if u128::from(e.decode.nanos) * 5 < u128::from(e.restore.nanos) * 4 {
                e.decode_first = true;
            } else if u128::from(e.restore.nanos) * 5 < u128::from(e.decode.nanos) * 4 {
                e.decode_first = false;
            }
        }
        // Occasionally measure the alternative after RAM misses; no duplicate decode.
        let preference = e.decode_first && !e.decode_deferred;
        let probe = e.requests.is_multiple_of(16);
        let selected = if probe { !preference } else { preference };
        log::debug!(
            "RAW auto key={} requests={} decode_samples={} decode_ns={} restore_samples={} restore_ns={} deferred={} probe={} decode_first={}",
            key.to_hex(),
            e.requests,
            e.decode.samples,
            e.decode.nanos,
            e.restore.samples,
            e.restore.nanos,
            e.decode_deferred,
            probe,
            selected
        );
        selected
    }
    pub fn observe_decode(&mut self, key: CacheKey, elapsed: Duration) {
        let entry = self.entry(key);
        entry.decode.observe(elapsed);
        entry.decode_deferred = false;
        log::debug!(
            "RAW auto observed decode key={} elapsed_ns={} samples={}",
            key.to_hex(),
            elapsed.as_nanos(),
            entry.decode.samples
        );
    }
    pub fn defer_decode(&mut self, key: CacheKey) {
        self.entry(key).decode_deferred = true;
        log::debug!(
            "RAW auto deferred decode key={} after memory refusal",
            key.to_hex()
        );
    }
    pub fn observe_restore(&mut self, key: CacheKey, elapsed: Duration) {
        let entry = self.entry(key);
        entry.restore.observe(elapsed);
        log::debug!(
            "RAW auto observed restore key={} elapsed_ns={} samples={}",
            key.to_hex(),
            elapsed.as_nanos(),
            entry.restore.samples
        );
    }
}
/// The same recovery path used by the foreground loader after decode admission fails.
pub(super) fn restore_after_pressure(
    cache: &rrrah_cache::DiskMosaicCache,
    mut ram: Option<&mut rrrah_cache::MosaicRamCache>,
    key: CacheKey,
    budget: Option<&rrrah_cache::MemoryBudget>,
    cancelled: &dyn Fn() -> bool,
) -> Option<rrrah_core::DecodedMosaic> {
    if cancelled() {
        return None;
    }
    if let Some(mosaic) = ram.as_mut().and_then(|ram| ram.get_with_cancel(&key, cancelled)) {
        return (!cancelled()).then_some(mosaic);
    }
    let budget = ram
        .as_ref()
        .and_then(|ram| ram.swap())
        .map(|swap| swap.restore_budget())
        .or(budget);
    cache
        .load_with_cancel(key, budget, cancelled)
        .ok()
        .flatten()
        .map(|hit| hit.mosaic)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(n: u8) -> CacheKey {
        CacheKey::for_mosaic(
            &rrrah_cache::SourceFingerprint {
                file_size: 1,
                modified_ns: 0,
                sampled_blake3: [n; 32],
            },
            0,
        )
    }
    #[test]
    #[ignore = "requires pinned Hasselblad X1D 3FR source"]
    fn real_3fr_decode_admission_refusal_recovers_disk_and_swap() {
        use rrrah_decode::RawDecoder;
        let path = std::path::PathBuf::from(std::env::var("RRRAH_AUTO_3FR_SOURCE").unwrap());
        let mut request = rrrah_decode::DecodeRequest::new(&path);
        let large = rrrah_cache::MemoryBudget::new(256 * 1024 * 1024);
        request.memory_budget = Some(large.clone());
        let source = rrrah_decode::NativeRawDecoder.decode(&request).unwrap().mosaic;
        let key = CacheKey::for_mosaic_recipe(
            &rrrah_cache::SourceFingerprint::from_path(&path).unwrap(),
            0,
            rrrah_decode::NativeRawDecoder.mosaic_recipe(&request).unwrap(),
        );
        let dir = tempfile::tempdir().unwrap();
        let cache = rrrah_cache::DiskMosaicCache::with_max_bytes(dir.path().join("disk"), 256 * 1024 * 1024);
        cache.store(key, &source).unwrap();
        let small = rrrah_cache::MemoryBudget::new(128 * 1024 * 1024);
        request.memory_budget = Some(small.clone());
        assert!(matches!(
            rrrah_decode::NativeRawDecoder.decode(&request),
            Err(rrrah_decode::DecodeError::Memory(_))
        ));
        assert_eq!(small.used(), 0);
        let restored = restore_after_pressure(&cache, None, key, Some(&small), &|| false).unwrap();
        assert_eq!(restored.metadata, source.metadata);
        assert_eq!(restored.pixels, source.pixels);
        assert!(restored.pixels.is_managed());
        drop(restored);
        assert_eq!(small.used(), 0);
        assert!(restore_after_pressure(&cache, None, key, Some(&small), &|| true).is_none());
        let empty = rrrah_cache::DiskMosaicCache::new(dir.path().join("missing"));
        let swap = rrrah_cache::MosaicSwapCache::new_with_budgets(
            dir.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits::bytes(256 * 1024 * 1024),
                queue_bytes: 128 * 1024 * 1024,
                queue_count: 1,
                restore_bytes: 128 * 1024 * 1024,
            },
            rrrah_cache::MemoryBudget::new(128 * 1024 * 1024),
            small.clone(),
        )
        .unwrap();
        assert_eq!(
            swap.try_enqueue(key, source.clone()),
            rrrah_cache::SpillAdmission::Queued
        );
        swap.wait_idle().unwrap();
        let mut ram = rrrah_cache::MosaicRamCache::new(0);
        ram.enable_swap(swap);
        let restored = restore_after_pressure(&empty, Some(&mut ram), key, Some(&small), &|| false).unwrap();
        assert_eq!(restored.metadata, source.metadata);
        assert_eq!(restored.pixels, source.pixels);
        drop(restored);
        assert_eq!(small.used(), 0);
        assert_eq!(ram.swap().unwrap().stats().reads, 1);
        drop(source);
        assert_eq!(large.used(), 0);
    }
    #[test]
    fn pressure_deferral_is_retryable_and_not_a_failed_cost_sample() {
        let mut costs = RawLoadCosts::default();
        for _ in 0..3 {
            costs.observe_decode(key(0), Duration::from_millis(10));
            costs.observe_restore(key(0), Duration::from_millis(100));
        }
        assert!(costs.decode_first(key(0)));
        costs.defer_decode(key(0));
        for _ in 0..14 {
            assert!(!costs.decode_first(key(0)));
        }
        assert!(costs.decode_first(key(0))); // Probe after resource conditions may have changed.
        assert!(!costs.decode_first(key(0)));
        costs.observe_decode(key(0), Duration::from_millis(10));
        assert!(costs.decode_first(key(0)));
        assert_eq!(costs.entries[&key(0)].decode.samples, 4);
    }
    #[test]
    fn confidence_hysteresis_and_probe_preserve_default() {
        let mut costs = RawLoadCosts::default();
        for _ in 0..2 {
            costs.observe_decode(key(0), Duration::from_millis(10));
            costs.observe_restore(key(0), Duration::from_millis(100));
        }
        assert!(!costs.decode_first(key(0)));
        costs.observe_decode(key(0), Duration::from_millis(10));
        costs.observe_restore(key(0), Duration::from_millis(100));
        assert!(costs.decode_first(key(0)));
        // A small fluctuation must not switch the learned preference.
        let entry = costs.entries.get_mut(&key(0)).unwrap();
        entry.decode.nanos = 90_000_000;
        entry.restore.nanos = 100_000_000;
        for _ in 0..13 {
            assert!(costs.decode_first(key(0)));
        }
        assert!(!costs.decode_first(key(0))); // Request 16 probes restoration.
        assert!(costs.decode_first(key(0))); // Probe does not alter preference.
    }
    #[test]
    fn measured_costs_hysteresis_and_bounded_identity() {
        let mut costs = RawLoadCosts::default();
        assert!(!costs.decode_first(key(0)));
        for _ in 0..3 {
            costs.observe_decode(key(0), Duration::from_millis(10));
            costs.observe_restore(key(0), Duration::from_millis(100));
        }
        assert!(costs.decode_first(key(0)));
        assert!(!costs.decode_first(key(1))); // Another source/recipe has no inherited decision.
        for _ in 0..12 {
            costs.observe_decode(key(0), Duration::from_millis(100));
            costs.observe_restore(key(0), Duration::from_millis(10));
        }
        assert!(!costs.decode_first(key(0)));
        for n in 2..100 {
            costs.observe_decode(key(n), Duration::MAX);
        }
        assert_eq!(costs.entries.len(), MAX_KEYS);
        assert!(!costs.entries.contains_key(&key(0)));
    }
}
