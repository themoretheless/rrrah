#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "requires pinned EOS 10D CRW; actual GPU"]
fn imported_crw_swap_preserves_sensor_metadata_and_metal_frame() {
    let source = std::path::PathBuf::from(std::env::var("RRRAH_CRW_SOURCE").unwrap());
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(source);
    request.memory_budget = Some(budget.clone());
    use rrrah_decode::RawDecoder;
    assert_eq!(
        rrrah_decode::image_source_kind(&request).unwrap(),
        rrrah_decode::ImageSourceKind::Sensor
    );
    let recipe = rrrah_decode::NativeRawDecoder.mosaic_recipe(&request).unwrap();
    assert_eq!(
        recipe,
        rrrah_decode::NativeCrwDecoder.mosaic_recipe(&request).unwrap()
    );
    let mosaic = rrrah_decode::NativeRawDecoder.decode(&request).unwrap().mosaic;
    let expected_pixels = mosaic.pixels.to_vec(); // Independent test-owned comparison, outside managed root.
    let metadata = mosaic.metadata.clone();
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("CRW adapter: {}", gpu.adapter_name());
    let frame = gpu.render(&mosaic, [128, 96]).pixels;
    let weight = budget.used();
    let limits = rrrah_cache::CacheLimits {
        max_bytes: weight,
        max_entries: Some(1),
        ttl: Some(std::time::Duration::from_secs(1)),
    };
    let mut ram = rrrah_cache::LeaseCache::new(limits);
    assert!(ram.insert(1u8, mosaic, weight).unwrap().is_empty());
    let lease = ram.get(&1).unwrap();
    assert!(ram.insert(2, (*lease).clone(), weight).is_err());
    assert_eq!(budget.used(), weight);
    assert!(
        ram.set_limits(rrrah_cache::CacheLimits {
            max_entries: Some(0),
            ..limits
        })
        .is_none()
    );
    assert!(
        ram.set_limits(rrrah_cache::CacheLimits {
            max_bytes: weight - 1,
            ..limits
        })
        .is_none()
    );
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert!(ram.get(&1).is_none());
    assert!(ram.drain_expired().is_empty());
    assert_eq!(gpu.render(&lease, [128, 96]).pixels, frame);
    let retained = lease.clone();
    drop(lease);
    assert!(ram.drain_expired().is_empty());
    drop(retained);
    let mut expired = ram.drain_expired();
    assert_eq!(expired.len(), 1);
    let (key, mosaic) = expired.pop().unwrap();
    assert_eq!(key, 1);
    assert!(ram.is_empty());
    assert_eq!(budget.used(), weight);
    let directory = tempfile::tempdir().unwrap();
    let swap = rrrah_cache::ImageSwapCache::<u8, rrrah_core::DecodedMosaic>::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits::bytes(64 * 1024 * 1024),
            queue_bytes: 64 * 1024 * 1024,
            queue_count: 1,
            restore_bytes: 64 * 1024 * 1024,
        },
        rrrah_core::MemoryBudget::new(64 * 1024 * 1024),
        budget.clone(),
    )
    .unwrap();
    assert_eq!(swap.try_enqueue(1, mosaic), rrrah_cache::SpillAdmission::Queued);
    swap.wait_idle().unwrap();
    assert_eq!(budget.used(), 0);
    let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
    assert!(swap.get(&1, || false).is_none());
    drop(pressure);
    let restored = swap.get(&1, || false).unwrap();
    assert_eq!(restored.metadata, metadata);
    assert_eq!(&*restored.pixels, &expected_pixels);
    assert_eq!(gpu.render(&restored, [128, 96]).pixels, frame);
    for (index, orientation) in [
        rrrah_core::Orientation::Normal,
        rrrah_core::Orientation::Rotate90,
        rrrah_core::Orientation::Rotate180,
        rrrah_core::Orientation::Rotate270,
    ]
    .into_iter()
    .enumerate()
    {
        let mut rotated_metadata = metadata.clone();
        rotated_metadata.orientation = orientation;
        let rotated =
            rrrah_core::DecodedMosaic::new(rotated_metadata.clone(), restored.pixels.clone()).unwrap();
        let before = gpu.render(&rotated, [128, 96]).pixels;
        let key = (index + 2) as u8;
        assert_eq!(
            swap.try_enqueue(key, rotated),
            rrrah_cache::SpillAdmission::Queued
        );
        swap.wait_idle().unwrap();
        assert_eq!(budget.used(), weight);
        let rotated_restored = swap.get(&key, || false).unwrap();
        assert_eq!(rotated_restored.metadata, rotated_metadata);
        assert_eq!(&*rotated_restored.pixels, &expected_pixels);
        assert_eq!(gpu.render(&rotated_restored, [128, 96]).pixels, before);
        drop(rotated_restored);
        assert_eq!(budget.used(), weight);
    }
    drop(restored);
    assert_eq!(budget.used(), 0);
    assert_eq!(swap.stats().writes, 5);
    assert_eq!(swap.stats().reads, 5);
}
