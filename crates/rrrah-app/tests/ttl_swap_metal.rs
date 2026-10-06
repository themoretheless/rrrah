#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_cache::{CacheLimits, ImageSwapConfig, MemoryBudget};
use std::time::Duration;

#[test]
fn expired_hdr_lease_and_swap_restore_preserve_metal_frame_and_float_bits() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("TTL HDR adapter: {}", gpu.adapter_name());
    let budget = MemoryBudget::new(4096);
    let samples = vec![-0.0f32, 2.0, 0.5, 1.0, 8.0, 0.25, -0.5, 1.0];
    let expected_bits: Vec<_> = samples.iter().map(|v| v.to_bits()).collect();
    let raster = rrrah_core::DecodedRaster::new(
        2,
        1,
        rrrah_core::RasterPixels::Rgba32Float(std::sync::Arc::new(samples).into()),
        rrrah_core::RasterColorSpace::LinearSrgb,
    )
    .unwrap()
    .try_manage_pixels(&budget)
    .unwrap();
    let params = rrrah_gpu::ViewParameters {
        viewport: [96., 64.],
        zoom: 1.,
        ..Default::default()
    };
    let expected = gpu.render_raster(&raster, params, [96, 64]).pixels;
    let weight = raster.capacity_bytes();
    let directory = tempfile::tempdir().unwrap();
    let mut ram = rrrah_cache::RasterRamCache::new(CacheLimits {
        max_bytes: weight,
        max_entries: Some(1),
        ttl: Some(Duration::from_secs(1)),
    });
    ram.enable_swap(
        rrrah_cache::RasterSwapCache::new_with_budgets(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(4096),
                queue_bytes: weight,
                queue_count: 1,
                restore_bytes: weight,
            },
            MemoryBudget::new(weight),
            budget.clone(),
        )
        .unwrap(),
    );
    assert!(ram.insert(1u8, raster));
    let lease = ram.get_lease(&1).unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    assert!(ram.get_lease(&1).is_none());
    assert_eq!(ram.spill_expired(), 0);
    assert_eq!(ram.swap().unwrap().stats().writes, 0);
    assert_eq!(gpu.render_raster(&lease, params, [96, 64]).pixels, expected);
    drop(lease);
    assert_eq!(ram.spill_expired(), 1);
    ram.swap().unwrap().wait_idle().unwrap();
    assert!(ram.is_empty());
    assert_eq!(budget.used(), 0);
    let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
    assert!(ram.get(&1).is_none());
    assert_eq!(ram.swap().unwrap().stats().errors, 0);
    drop(pressure);
    let restored = ram.get(&1).unwrap();
    let rrrah_core::RasterPixels::Rgba32Float(values) = restored.pixels() else {
        panic!()
    };
    assert_eq!(
        values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected_bits
    );
    assert_eq!(gpu.render_raster(&restored, params, [96, 64]).pixels, expected);
    drop(restored);
    drop(ram);
    assert_eq!(budget.used(), 0);
}

#[test]
fn expired_real_cr3_lease_and_swap_restore_preserve_metal_frame() {
    use rrrah_decode::RawDecoder;
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("TTL RAW adapter: {}", gpu.adapter_name());
    let budget = MemoryBudget::new(256 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3"),
    );
    request.memory_budget = Some(budget.clone());
    let native = rrrah_decode::NativeRawDecoder.decode(&request).unwrap().mosaic;
    let metadata = native.metadata.clone();
    let expected_samples = native.pixels.to_vec();
    let expected = gpu.render(&native, [128, 96]).pixels;
    let weight = native.pixels.capacity_bytes();
    let key = rrrah_cache::CacheKey::for_mosaic_recipe(
        &rrrah_cache::SourceFingerprint::from_path(&request.path).unwrap(),
        0,
        rrrah_decode::NativeRawDecoder.mosaic_recipe(&request).unwrap(),
    );
    let directory = tempfile::tempdir().unwrap();
    let mut ram = rrrah_cache::MosaicRamCache::with_limits(CacheLimits {
        max_bytes: weight,
        max_entries: Some(1),
        ttl: Some(Duration::from_secs(1)),
    });
    ram.enable_swap(
        rrrah_cache::MosaicSwapCache::new_with_budgets(
            directory.path(),
            rrrah_cache::MosaicSwapConfig {
                limits: CacheLimits::bytes(weight + 65536),
                queue_bytes: weight,
                queue_count: 1,
                restore_bytes: weight,
            },
            MemoryBudget::new(weight),
            budget.clone(),
        )
        .unwrap(),
    );
    assert!(ram.insert(key, native));
    let lease = ram.get_lease(&key).unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    assert!(ram.get_lease(&key).is_none());
    assert_eq!(ram.spill_expired(), 0);
    assert_eq!(gpu.render(&lease, [128, 96]).pixels, expected);
    drop(lease);
    assert_eq!(ram.spill_expired(), 1);
    ram.swap().unwrap().wait_idle().unwrap();
    assert_eq!(budget.used(), 0);
    let restored = ram.get(&key).unwrap();
    assert_eq!(restored.metadata, metadata);
    assert_eq!(&*restored.pixels, expected_samples.as_slice());
    assert_eq!(gpu.render(&restored, [128, 96]).pixels, expected);
    drop(restored);
    drop(ram);
    assert_eq!(budget.used(), 0);
}
