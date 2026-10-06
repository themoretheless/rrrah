//! Qualifies the native model codec against the real asynchronous swap store.
use rrrah_cache::{CacheLimits, ImageSwapCache, ImageSwapConfig, SwapPayload};
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodedModel;

#[path = "../src/model_swap.rs"]
mod model_swap;
use model_swap::ModelPayload;
fn mesh(value: &ModelPayload) -> &rrrah_decode::ModelBuffer<rrrah_decode::StlMesh> {
    match &value.0 {
        DecodedModel::Stl(mesh) => mesh,
        _ => panic!("STL expected"),
    }
}
fn source(root: &MemoryBudget) -> ModelPayload {
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models/triangle.stl");
    let mut request = rrrah_decode::DecodeRequest::new(path);
    request.memory_budget = Some(root.clone());
    let DecodedModel::Stl(mesh) = rrrah_decode::decode_model(&request).unwrap() else {
        panic!("STL expected");
    };
    ModelPayload::from_model(DecodedModel::Stl(mesh)).unwrap()
}
fn config() -> ImageSwapConfig {
    ImageSwapConfig {
        limits: CacheLimits {
            max_bytes: 4096,
            max_entries: Some(2),
            ttl: None,
        },
        queue_bytes: 4096,
        queue_count: 2,
        restore_bytes: 4096,
    }
}
#[test]
fn all_model_codecs_retain_swap_entries_across_repeated_memory_pressure() {
    for name in [
        "triangle.stl",
        "obj-shared-metadata.obj",
        "off-attributes-binary.off",
        "ply-32-ascii.ply",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source_root = MemoryBudget::new(1024 * 1024);
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/models")
            .join(name);
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = Some(source_root.clone());
        let value = ModelPayload::from_model(rrrah_decode::decode_model(&request).unwrap()).unwrap();
        let mut expected = Vec::new();
        value.write_payload(&mut expected).unwrap();
        let root = MemoryBudget::new(1024 * 1024);
        let mut cfg = config();
        cfg.limits.max_bytes = 1024 * 1024;
        cfg.queue_bytes = 1024 * 1024;
        cfg.restore_bytes = 1024 * 1024;
        let swap: ImageSwapCache<u64, ModelPayload> = ImageSwapCache::new_with_budgets(
            directory.path(),
            cfg,
            MemoryBudget::new(1024 * 1024),
            root.clone(),
        )
        .unwrap();
        swap.enqueue(1, value);
        swap.wait_idle().unwrap();
        assert_eq!(source_root.used(), 0);
        for _ in 0..20 {
            let pressure = root.try_reserve(1024 * 1024).unwrap();
            assert!(swap.try_get(&1, || false).is_err(), "{name}");
            assert_eq!(root.used(), 1024 * 1024);
            assert_eq!(swap.stats().errors, 0);
            drop(pressure);
            assert!(swap.try_get(&1, || true).unwrap().is_none());
            assert_eq!(root.used(), 0);
            let restored = swap.try_get(&1, || false).unwrap().unwrap();
            let mut actual = Vec::new();
            restored.write_payload(&mut actual).unwrap();
            assert_eq!(actual, expected);
            drop(restored);
            assert_eq!(root.used(), 0);
        }
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(swap.stats().reads, 20);
        assert_eq!(swap.stats().errors, 0);
    }
}
#[test]
fn corrupt_model_entries_release_credit_and_allow_replacement() {
    for name in [
        "triangle.stl",
        "obj-shared-metadata.obj",
        "off-attributes-binary.off",
        "ply-32-ascii.ply",
    ] {
        for truncate in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let root = MemoryBudget::new(1024 * 1024);
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/models")
                .join(name);
            let mut request = rrrah_decode::DecodeRequest::new(path);
            request.memory_budget = Some(root.clone());
            let decode = || ModelPayload::from_model(rrrah_decode::decode_model(&request).unwrap()).unwrap();
            let mut cfg = config();
            cfg.limits.max_bytes = 1024 * 1024;
            cfg.queue_bytes = 1024 * 1024;
            cfg.restore_bytes = 1024 * 1024;
            let swap: ImageSwapCache<u64, ModelPayload> = ImageSwapCache::new_with_budgets(
                directory.path(),
                cfg,
                MemoryBudget::new(1024 * 1024),
                root.clone(),
            )
            .unwrap();
            swap.enqueue(1, decode());
            swap.wait_idle().unwrap();
            assert_eq!(root.used(), 0);
            let session = std::fs::read_dir(directory.path())
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let blob = std::fs::read_dir(&session)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let mut bytes = std::fs::read(&blob).unwrap();
            if truncate {
                bytes.pop();
            } else {
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
            }
            std::fs::write(&blob, bytes).unwrap();
            assert!(swap.try_get(&1, || false).unwrap().is_none(), "{name}");
            assert_eq!(root.used(), 0);
            assert_eq!(swap.stats().errors, 1);
            assert!(!blob.exists());
            assert!(swap.try_get(&1, || false).unwrap().is_none());
            assert_eq!(swap.stats().errors, 1);
            swap.enqueue(1, decode());
            swap.wait_idle().unwrap();
            let restored = swap.try_get(&1, || false).unwrap().unwrap();
            assert_eq!(root.used(), restored.capacity_bytes());
            drop(restored);
            assert_eq!(root.used(), 0);
        }
    }
}
#[test]
fn sustained_small_ram_and_swap_navigation_releases_every_owner() {
    let directory = tempfile::tempdir().unwrap();
    let probe_root = MemoryBudget::new(4096);
    let probe = source(&probe_root);
    let weight = probe.capacity_bytes();
    let mut expected = Vec::new();
    probe.write_payload(&mut expected).unwrap();
    drop(probe);
    assert_eq!(probe_root.used(), 0);
    // Source decoding also reserves its bounded input while the old frame lives.
    let root = MemoryBudget::new(weight + probe_root.peak());
    let queue = MemoryBudget::new(weight);
    let swap: ImageSwapCache<u64, ModelPayload> =
        ImageSwapCache::new_with_budgets(directory.path(), config(), queue.clone(), root.child(weight * 2))
            .unwrap();
    let mut ram = rrrah_cache::WeightedLru::with_limits(CacheLimits {
        max_bytes: weight,
        max_entries: Some(1),
        ttl: None,
    });
    let mut hits = 0;
    let mut misses = 0;
    // Alternate short revisits and scans beyond the two-entry disk capacity.
    for iteration in 0..200 {
        let key = [0, 1, 0, 1, 2, 3, 2, 3][iteration % 8];
        let value = match swap.try_get(&key, || false).unwrap() {
            Some(value) => {
                hits += 1;
                value
            }
            None => {
                misses += 1;
                source(&root)
            }
        };
        let mut actual = Vec::new();
        value.write_payload(&mut actual).unwrap();
        assert_eq!(actual, expected);
        let visible_owner = value.clone();
        let (inserted, victims) = ram.insert_prefetch_with_evictions(key, value, weight);
        assert!(inserted);
        assert_eq!(ram.len(), 1);
        for (old_key, old_value) in victims {
            swap.enqueue(old_key, old_value);
        }
        swap.wait_idle().unwrap();
        assert_eq!(queue.used(), 0);
        assert_eq!(root.used(), weight);
        drop(visible_owner);
        assert_eq!(root.used(), weight);
    }
    assert!(hits > 0 && misses > 0);
    assert_eq!(swap.stats().errors, 0);
    assert_eq!(swap.stats().dropped, 0);
    drop(ram);
    assert_eq!(root.used(), 0);
    drop(swap);
    assert_eq!(queue.used(), 0);
}
#[test]
fn real_store_roundtrip_preserves_geometry_and_last_owner_budget() {
    let directory = tempfile::tempdir().unwrap();
    let source_root = MemoryBudget::new(4096);
    let value = source(&source_root);
    let expected = mesh(&value).facets.clone();
    let bounds = mesh(&value).bounds;
    let restore_root = MemoryBudget::new(4096);
    let cache: ImageSwapCache<u64, ModelPayload> = ImageSwapCache::new_with_budgets(
        directory.path(),
        config(),
        MemoryBudget::new(4096),
        restore_root.clone(),
    )
    .unwrap();
    cache.enqueue(7, value);
    cache.wait_idle().unwrap();
    assert_eq!(cache.stats().writes, 1);
    assert_eq!(cache.stats().queued_bytes, 0);
    assert_eq!(source_root.used(), 0);
    let restored = cache.try_get(&7, || false).unwrap().unwrap();
    assert_eq!(mesh(&restored).facets, expected);
    assert_eq!(mesh(&restored).bounds, bounds);
    assert!(mesh(&restored).is_managed());
    assert_eq!(restore_root.used(), restored.capacity_bytes());
    let clone = restored.clone();
    drop(restored);
    assert!(restore_root.used() > 0);
    drop(clone);
    assert_eq!(restore_root.used(), 0);
    assert_eq!(cache.stats().reads, 1);
    assert_eq!(cache.stats().errors, 0);
}
#[test]
fn pressure_keeps_disk_entry_for_retry_and_cancellation_does_not_restore() {
    let directory = tempfile::tempdir().unwrap();
    let source_root = MemoryBudget::new(4096);
    let value = source(&source_root);
    let restore_root = MemoryBudget::new(value.capacity_bytes());
    let cache: ImageSwapCache<u64, ModelPayload> = ImageSwapCache::new_with_budgets(
        directory.path(),
        config(),
        MemoryBudget::new(4096),
        restore_root.clone(),
    )
    .unwrap();
    cache.enqueue(9, value);
    cache.wait_idle().unwrap();
    let pressure = restore_root.try_reserve(1).unwrap();
    assert!(cache.try_get(&9, || false).is_err());
    assert_eq!(restore_root.used(), 1);
    assert_eq!(cache.stats().errors, 0);
    drop(pressure);
    assert!(cache.try_get(&9, || true).unwrap().is_none());
    assert_eq!(restore_root.used(), 0);
    let restored = cache.try_get(&9, || false).unwrap().unwrap();
    assert_eq!(cache.stats().writes, 1);
    assert_eq!(cache.stats().reads, 1);
    drop(restored);
    assert_eq!(restore_root.used(), 0);
    assert_eq!(source_root.used(), 0);
}

#[test]
fn model_payload_obeys_disk_size_count_and_fixed_ttl_limits() {
    for limits in [
        CacheLimits {
            max_bytes: 1,
            max_entries: None,
            ttl: None,
        },
        CacheLimits {
            max_bytes: 4096,
            max_entries: Some(0),
            ttl: None,
        },
        CacheLimits {
            max_bytes: 4096,
            max_entries: None,
            ttl: Some(std::time::Duration::ZERO),
        },
    ] {
        let directory = tempfile::tempdir().unwrap();
        let root = MemoryBudget::new(4096);
        let mut settings = config();
        settings.limits = limits;
        let cache: ImageSwapCache<u64, ModelPayload> =
            ImageSwapCache::new(directory.path(), settings).unwrap();
        cache.enqueue(1, source(&root));
        cache.wait_idle().unwrap();
        assert!(cache.try_get(&1, || false).unwrap().is_none());
        assert_eq!(root.used(), 0);
        assert_eq!(cache.stats().queued_bytes, 0);
    }
    let directory = tempfile::tempdir().unwrap();
    let root = MemoryBudget::new(4096);
    let mut settings = config();
    settings.limits.max_entries = Some(1);
    let cache: ImageSwapCache<u64, ModelPayload> = ImageSwapCache::new(directory.path(), settings).unwrap();
    cache.enqueue(1, source(&root));
    cache.wait_idle().unwrap();
    assert!(cache.try_get(&1, || false).unwrap().is_some());
    cache.enqueue(2, source(&root));
    cache.wait_idle().unwrap();
    assert!(cache.try_get(&1, || false).unwrap().is_none());
    let restored = cache.try_get(&2, || false).unwrap().unwrap();
    drop(restored);
    assert_eq!(root.used(), 0);
    assert_eq!(cache.restore_budget().used(), 0);
}
