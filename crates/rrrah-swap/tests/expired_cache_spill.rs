use rrrah_memory::{CacheLimits, LeaseCache, MemoryBudget};
use rrrah_swap::{SwapLimits, SwapStore};
use std::time::Duration;

#[test]
fn expired_values_transfer_to_streamed_swap_without_copy_or_early_budget_release() {
    const SIZE: usize = 96 * 1024;
    let budget = MemoryBudget::new(SIZE as u64);
    let source = budget.try_buffer(SIZE, 73u8).unwrap().freeze();
    let address = source.as_ptr();
    let mut cache = LeaseCache::new(CacheLimits {
        max_bytes: SIZE as u64,
        max_entries: Some(1),
        ttl: Some(Duration::from_secs(1)),
    });
    cache.insert("visible", source, SIZE as u64).unwrap();
    let lease = cache.get(&"visible").unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    assert!(cache.get(&"visible").is_none());
    assert!(cache.drain_expired().is_empty());
    assert_eq!(lease.as_ptr(), address);
    drop(lease);
    let mut victims = cache.drain_expired();
    assert_eq!(victims.len(), 1);
    let (key, pixels) = victims.pop().unwrap();
    assert_eq!(key, "visible");
    assert_eq!(pixels.as_ptr(), address);
    assert_eq!(cache.resident_weight(), 0);
    assert!(cache.is_empty());
    assert_eq!(budget.used(), SIZE as u64);
    let directory = tempfile::tempdir().unwrap();
    let store = SwapStore::new(
        directory.path(),
        SwapLimits {
            max_bytes: SIZE as u64,
            max_objects: Some(1),
        },
    )
    .unwrap();
    let handle = store
        .write_chunks(SIZE as u64, pixels.chunks(8192), || false)
        .unwrap();
    assert!(store.restore(&handle, &budget, || false).is_err());
    assert_eq!(store.usage().unwrap().bytes, SIZE as u64);
    drop(pixels);
    assert_eq!(budget.used(), 0);
    let restored = store.restore(&handle, &budget, || false).unwrap();
    assert_eq!(&*restored, &[73u8; SIZE]);
    drop(restored);
    drop(handle);
    assert_eq!(budget.used(), 0);
    assert_eq!(store.usage().unwrap().bytes, 0);
    assert_eq!(store.usage().unwrap().objects, 0);
}
