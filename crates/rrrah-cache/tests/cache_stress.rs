//! Deterministic stress and adversarial tests for the cache boundary.
//!
//! These tests intentionally use a small, reproducible model rather than a
//! wall-clock benchmark. The cache is on the RAW hot path, so a race or a
//! byte-accounting regression must fail the fast unit suite before a noisy
//! throughput benchmark is trusted.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use rrrah_cache::{CacheKey, SourceFingerprint, WeightedLru};

#[derive(Debug, Clone, Copy)]
struct ModelEntry {
    value: u64,
    weight: u64,
    last_used: u64,
}

#[derive(Debug, Default, Clone, Copy)]
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        // A full-period 64-bit LCG. The fixed seed makes this a regression
        // test, not an unrepeatable fuzz run.
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
}

fn model_insert(
    entries: &mut HashMap<u16, ModelEntry>,
    resident: &mut u64,
    clock: &mut u64,
    key: u16,
    value: u64,
    weight: u64,
    capacity: u64,
) -> bool {
    if weight > capacity {
        return false;
    }
    *clock = clock.wrapping_add(1);
    if let Some(previous) = entries.remove(&key) {
        *resident -= previous.weight;
    }
    while resident.saturating_add(weight) > capacity {
        let victim = entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(key, _)| *key)
            .expect("a resident entry must exist while over capacity");
        let removed = entries.remove(&victim).expect("model victim exists");
        *resident -= removed.weight;
    }
    *resident += weight;
    entries.insert(
        key,
        ModelEntry {
            value,
            weight,
            last_used: *clock,
        },
    );
    true
}

#[test]
fn weighted_lru_has_bounded_resident_bytes_under_deterministic_stress() {
    const CAPACITY: u64 = 256;
    const KEY_COUNT: u16 = 64;
    const OPERATIONS: usize = 50_000;

    let mut cache = WeightedLru::new(CAPACITY);
    let mut model = HashMap::<u16, ModelEntry>::new();
    let mut model_resident = 0_u64;
    let mut model_clock = 0_u64;
    let mut rng = Lcg::new(0x5eed_cafe_f00d_baad);

    for step in 0..OPERATIONS {
        let key = u16::try_from(rng.next() % u64::from(KEY_COUNT)).expect("bounded key");
        match rng.next() % 3 {
            0 => {
                let value = (step as u64) ^ rng.next();
                // Include zero-weight entries and values close to the budget;
                // both have historically exposed accounting mistakes.
                let weight = rng.next() % (CAPACITY + 1);
                let expected = model_insert(
                    &mut model,
                    &mut model_resident,
                    &mut model_clock,
                    key,
                    value,
                    weight,
                    CAPACITY,
                );
                assert_eq!(cache.insert(key, value, weight), expected, "step {step}");
            }
            1 => {
                model_clock = model_clock.wrapping_add(1);
                let expected = model.get_mut(&key).map(|entry| {
                    entry.last_used = model_clock;
                    entry.value
                });
                assert_eq!(cache.get(&key).copied(), expected, "step {step}");
            }
            _ => {
                let expected = model.remove(&key).map(|entry| {
                    model_resident -= entry.weight;
                    entry.value
                });
                assert_eq!(cache.remove(&key), expected, "step {step}");
            }
        }

        assert!(cache.resident_weight() <= CAPACITY, "step {step}");
        assert_eq!(cache.resident_weight(), model_resident, "bytes at step {step}");
        assert_eq!(cache.len(), model.len(), "entries at step {step}");
    }

    // Compare every key once at the end. `get` is mirrored in the model
    // because it updates recency and must not alter accounting.
    for key in 0..KEY_COUNT {
        model_clock = model_clock.wrapping_add(1);
        let expected = model.get_mut(&key).map(|entry| {
            entry.last_used = model_clock;
            entry.value
        });
        assert_eq!(cache.get(&key).copied(), expected, "final key {key}");
    }
    assert_eq!(cache.resident_weight(), model_resident);
    assert_eq!(cache.len(), model.len());
}

#[test]
fn weighted_lru_replacement_never_underflows_byte_accounting() {
    let mut cache = WeightedLru::new(10);
    assert!(cache.insert("same", 1_u8, 10));
    assert_eq!(cache.resident_weight(), 10);
    assert!(cache.insert("same", 2_u8, 1));
    assert_eq!(cache.resident_weight(), 1);
    assert_eq!(cache.remove(&"same"), Some(2));
    assert_eq!(cache.resident_weight(), 0);
    assert!(cache.remove(&"same").is_none());
    assert_eq!(cache.resident_weight(), 0);

    // A rejected oversized replacement must leave the existing entry intact.
    assert!(cache.insert("same", 3_u8, 4));
    assert!(!cache.insert("same", 4_u8, 11));
    assert_eq!(cache.get(&"same"), Some(&3));
    assert_eq!(cache.resident_weight(), 4);
}

#[test]
fn cache_key_domain_is_separated_for_sources_and_image_indices() {
    let source = SourceFingerprint {
        file_size: 123_456,
        modified_ns: 987_654,
        sampled_blake3: [0xA5; 32],
    };
    let mut keys = HashSet::new();
    for image_index in 0..1024 {
        assert!(keys.insert(CacheKey::for_mosaic(&source, image_index)));
    }

    let mut changed_source = source.clone();
    changed_source.file_size += 1;
    assert_ne!(
        CacheKey::for_mosaic(&source, 0),
        CacheKey::for_mosaic(&changed_source, 0)
    );
    changed_source = source.clone();
    changed_source.modified_ns += 1;
    assert_ne!(
        CacheKey::for_mosaic(&source, 0),
        CacheKey::for_mosaic(&changed_source, 0)
    );
    changed_source = source.clone();
    changed_source.sampled_blake3[0] ^= 1;
    assert_ne!(
        CacheKey::for_mosaic(&source, 0),
        CacheKey::for_mosaic(&changed_source, 0)
    );
}

/// A tiny test-only publish gate. The production scheduler is not present in
/// this workspace yet; this oracle documents the required atomic rule while
/// testing stale-generation behaviour without sleeping or thread scheduling.
#[derive(Debug, Default)]
struct PublishGate {
    state: Mutex<(u64, Vec<u64>)>,
}

impl PublishGate {
    fn generation(&self) -> u64 {
        self.state.lock().expect("gate lock poisoned").0
    }

    fn advance(&self) -> u64 {
        let mut state = self.state.lock().expect("gate lock poisoned");
        state.0 = state.0.checked_add(1).expect("test generation overflow");
        state.0
    }

    fn publish(&self, work_generation: u64, tile: u64) -> bool {
        let mut state = self.state.lock().expect("gate lock poisoned");
        if state.0 != work_generation {
            return false;
        }
        state.1.push(tile);
        true
    }

    fn published(&self) -> Vec<u64> {
        self.state.lock().expect("gate lock poisoned").1.clone()
    }
}

#[test]
fn stale_generation_is_dropped_before_publish() {
    let gate = Arc::new(PublishGate::default());
    let old_generation = gate.advance();
    let new_generation = gate.advance();
    assert_eq!(gate.generation(), new_generation);
    assert!(!gate.publish(old_generation, 11));
    assert!(gate.publish(new_generation, 22));
    assert_eq!(gate.published(), vec![22]);
}

/// Reservation ledger oracle used until the real scheduler lands. It models
/// checked byte admission and idempotent release; the test deliberately calls
/// release twice to ensure a future implementation does not hide underflow
/// with saturating subtraction.
#[derive(Debug)]
struct ReservationLedger {
    capacity: u64,
    resident: AtomicU64,
    next_id: AtomicU64,
    released: Mutex<HashSet<u64>>,
}

impl ReservationLedger {
    fn new(capacity: u64) -> Self {
        Self {
            capacity,
            resident: AtomicU64::new(0),
            next_id: AtomicU64::new(1),
            released: Mutex::new(HashSet::new()),
        }
    }

    fn reserve(&self, bytes: u64) -> Option<u64> {
        let current = self.resident.load(Ordering::Relaxed);
        let next = current.checked_add(bytes)?;
        if next > self.capacity {
            return None;
        }
        self.resident
            .compare_exchange(current, next, Ordering::SeqCst, Ordering::Relaxed)
            .ok()?;
        Some(self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    fn release(&self, id: u64, bytes: u64) -> bool {
        let mut released = self.released.lock().expect("ledger lock poisoned");
        if !released.insert(id) {
            return false;
        }
        let current = self.resident.load(Ordering::Relaxed);
        let Some(next) = current.checked_sub(bytes) else {
            panic!("reservation accounting underflow");
        };
        self.resident.store(next, Ordering::SeqCst);
        true
    }
}

#[test]
fn reservation_accounting_rejects_overflow_and_double_release() {
    let ledger = ReservationLedger::new(100);
    let first = ledger.reserve(60).expect("first reservation");
    assert!(ledger.reserve(41).is_none());
    assert!(ledger.release(first, 60));
    assert!(!ledger.release(first, 60));
    assert_eq!(ledger.resident.load(Ordering::SeqCst), 0);

    // `u64::MAX` must be rejected by checked arithmetic even with a large
    // logical capacity; no wrapped reservation may enter the ledger.
    let large = ReservationLedger::new(u64::MAX);
    let one = large.reserve(u64::MAX - 1).expect("large reservation");
    assert!(large.reserve(2).is_none());
    assert!(large.release(one, u64::MAX - 1));
    assert_eq!(large.resident.load(Ordering::SeqCst), 0);
}
