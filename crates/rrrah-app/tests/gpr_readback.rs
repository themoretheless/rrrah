#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "requires pinned HERO9 GPR and independently staged GoPro sensor dump"]
fn gpr_sensor_cache_swap_and_metal_preserve_pixels_and_metadata() {
    use rrrah_decode::RawDecoder;
    let root = std::path::PathBuf::from(std::env::var("RRRAH_GPR_CORPUS").expect("RRRAH_GPR_CORPUS"));
    {
        let budget = rrrah_core::MemoryBudget::new(256 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(root.join("HERO9.GPR"));
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::NativeRawDecoder.decode(&request).unwrap();
        let native = decoded.mosaic;
        assert_eq!((native.metadata.width, native.metadata.height), (5568, 4176));
        assert!(native.pixels.is_managed());
        let bytes = std::fs::read(root.join("sensor-oracle.u16le")).unwrap();
        assert_eq!(bytes.len(), 5568 * 4176 * 2);
        let samples: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
            .collect();
        assert_eq!(&*native.pixels, samples.as_slice());
        assert_eq!(
            native.metadata.cfa.as_ref().unwrap().cells,
            [
                rrrah_core::CfaColor::Red,
                rrrah_core::CfaColor::Green,
                rrrah_core::CfaColor::Green,
                rrrah_core::CfaColor::Blue
            ]
        );
        assert_eq!(native.metadata.bits_per_sample, 14);
        // Sensor pixels have an independent oracle. Render equality below proves
        // transport and metadata preservation, not independent camera color fidelity.
        let reference =
            rrrah_core::DecodedMosaic::new(native.metadata.clone(), std::sync::Arc::new(samples)).unwrap();
        let gpu = common::qualification_gpu().expect("GPU required for GPR HERO9 qualification");
        eprintln!("GPR HERO9 qualification adapter: {}", gpu.adapter_name());
        let actual = gpu.render(&native, [128, 96]);
        let expected = gpu.render(&reference, [128, 96]);
        assert_eq!(actual.pixels, expected.pixels);
        assert!(
            actual
                .pixels
                .chunks_exact(4)
                .any(|p| p[..3].iter().any(|v| *v > 0))
        );
        assert!(actual.pixels.chunks_exact(4).all(|p| p[3] == 255));
        let directory = tempfile::tempdir().unwrap();
        let recipe = rrrah_decode::NativeRawDecoder.mosaic_recipe(&request).unwrap();
        let key = rrrah_cache::CacheKey::for_mosaic_recipe(
            &rrrah_cache::SourceFingerprint::from_path(&request.path).unwrap(),
            0,
            recipe,
        );
        let sensor_bytes = native.pixels.capacity_bytes();
        let mut size_limited = rrrah_cache::MosaicRamCache::new(sensor_bytes - 1);
        assert!(!size_limited.insert(key, native.clone()));
        let mut count_limited = rrrah_cache::MosaicRamCache::with_limits(rrrah_cache::CacheLimits {
            max_bytes: sensor_bytes,
            max_entries: Some(0),
            ttl: None,
        });
        assert!(!count_limited.insert(key, native.clone()));
        let mut expiring = rrrah_cache::MosaicRamCache::with_limits(rrrah_cache::CacheLimits {
            max_bytes: sensor_bytes,
            max_entries: Some(1),
            ttl: Some(std::time::Duration::ZERO),
        });
        assert!(expiring.insert(key, native.clone()));
        assert!(expiring.get(&key).is_none());
        drop(expiring);
        let mut pinned = rrrah_cache::MosaicRamCache::new(sensor_bytes);
        assert!(pinned.insert_visible(key, native.clone()));
        assert!(
            pinned
                .set_limits(rrrah_cache::CacheLimits {
                    max_bytes: 0,
                    max_entries: Some(0),
                    ttl: None,
                })
                .is_none()
        );
        let visible = pinned.get(&key).unwrap();
        assert_eq!(visible.pixels.as_ptr(), native.pixels.as_ptr());
        assert_eq!(gpu.render(&visible, [128, 96]).pixels, expected.pixels);
        drop(visible);
        drop(pinned);
        assert_eq!(budget.used(), sensor_bytes);
        let disk = rrrah_cache::DiskMosaicCache::with_max_bytes(
            directory.path().join("persistent"),
            256 * 1024 * 1024,
        );
        disk.store(key, &native).unwrap();
        let cached = disk.load_with_budget(key, &budget).unwrap().unwrap().mosaic;
        assert_eq!(cached.metadata, native.metadata);
        assert_eq!(cached.pixels, native.pixels);
        assert_eq!(gpu.render(&cached, [128, 96]).pixels, expected.pixels);
        drop(cached);

        let swap: rrrah_cache::ImageSwapCache<u8, rrrah_core::DecodedMosaic> =
            rrrah_cache::ImageSwapCache::new_with_budgets(
                directory.path(),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits {
                        max_bytes: 256 * 1024 * 1024,
                        max_entries: Some(1),
                        ttl: None,
                    },
                    queue_bytes: 256 * 1024 * 1024,
                    queue_count: 1,
                    restore_bytes: 256 * 1024 * 1024,
                },
                rrrah_core::MemoryBudget::new(256 * 1024 * 1024),
                budget.clone(),
            )
            .unwrap();
        assert_eq!(
            swap.try_enqueue(1, native.clone()),
            rrrah_cache::SpillAdmission::Queued
        );
        swap.wait_idle().unwrap();
        let expected_metadata = native.metadata.clone();
        drop(native);
        assert_eq!(budget.used(), 0);
        let pressure = budget.try_reserve(256 * 1024 * 1024).unwrap();
        assert!(swap.try_get(&1, || false).is_err());
        assert_eq!(swap.stats().errors, 0);
        drop(pressure);
        let restored = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(restored.metadata, expected_metadata);
        assert!(restored.pixels.is_managed());
        assert_eq!(&*restored.pixels, &*reference.pixels);
        assert_eq!(gpu.render(&restored, [128, 96]).pixels, expected.pixels);
        drop(restored);
        assert_eq!(budget.used(), 0);
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(swap.stats().reads, 1);
        assert_eq!(swap.stats().errors, 0);
        assert_eq!(swap.stats().queued_bytes, 0);
    }
}

#[test]
#[ignore = "requires pinned HERO9 GPR source"]
fn navigation_cancels_active_gpr_decode_and_releases_all_managed_buffers() {
    use rrrah_decode::RawDecoder;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let root = std::path::PathBuf::from(std::env::var("RRRAH_GPR_CORPUS").unwrap());
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let generation = Arc::new(AtomicU64::new(1));
    let mut request = rrrah_decode::DecodeRequest::new(root.join("HERO9.GPR"));
    request.memory_budget = Some(budget.clone());
    request.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
    let worker = std::thread::spawn(move || rrrah_decode::NativeRawDecoder.decode(&request));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    // Source + final sensor are below 60 MiB. Crossing this boundary proves
    // intermediate VC-5 reconstruction has started, rather than cancelling I/O.
    while budget.peak() <= 60 * 1024 * 1024 && !worker.is_finished() {
        assert!(
            std::time::Instant::now() < deadline,
            "decode did not reach intermediate admission"
        );
        std::thread::yield_now();
    }
    assert!(budget.peak() > 60 * 1024 * 1024);
    generation.store(2, Ordering::Release);
    assert!(matches!(
        worker.join().unwrap(),
        Err(rrrah_decode::DecodeError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
#[ignore = "requires pinned HERO9 GPR and Metal adapter"]
fn developed_gpr_float_swap_and_metal_preserve_corrected_pixels() {
    qualify_developed_gpr("HERO9", 4, 0);
}

#[test]
#[ignore = "requires pinned Fusion GPR sources and Metal adapter"]
fn fusion_rectilinear_float_swap_and_metal_preserve_corrected_pixels() {
    for name in ["Fusion-back", "Fusion-front"] {
        qualify_developed_gpr(name, 0, 1);
    }
}

fn qualify_developed_gpr(name: &str, gain_maps: usize, warps: usize) {
    use rrrah_decode::RawDecoder;
    let root = std::path::PathBuf::from(std::env::var("RRRAH_GPR_CORPUS").unwrap());
    let budget = rrrah_core::MemoryBudget::new(1024 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(root.join(format!("{name}.GPR")));
    request.memory_budget = Some(budget.clone());
    let mosaic = rrrah_decode::NativeRawDecoder.decode(&request).unwrap().mosaic;
    let lists = rrrah_decode::raw_development_opcodes(&request).unwrap();
    assert_eq!(lists.list2.len(), gain_maps);
    assert_eq!(lists.list3.len(), warps);
    let raster =
        rrrah_core::develop::develop_raw(&mosaic, &Default::default(), &lists, Some(&budget), &|| false)
            .unwrap();
    drop(mosaic);
    let weight = raster.capacity_bytes();
    assert_eq!(budget.used(), weight);
    // Full developed float allocation participates in the same policies as other rasters.
    let mut too_small = rrrah_cache::RasterRamCache::<u8>::new(rrrah_cache::CacheLimits::bytes(weight - 1));
    assert!(!too_small.insert(1, raster.clone()));
    let mut no_entries = rrrah_cache::RasterRamCache::<u8>::new(rrrah_cache::CacheLimits {
        max_bytes: weight,
        max_entries: Some(0),
        ttl: None,
    });
    assert!(!no_entries.insert(1, raster.clone()));
    let mut expired = rrrah_cache::RasterRamCache::<u8>::new(rrrah_cache::CacheLimits {
        max_bytes: weight,
        max_entries: Some(1),
        ttl: Some(std::time::Duration::ZERO),
    });
    assert!(expired.insert(1, raster.clone()));
    assert!(expired.get_background_with_cancel(&1, || false).is_none());
    let mut pinned = rrrah_cache::RasterRamCache::<u8>::new(rrrah_cache::CacheLimits {
        max_bytes: weight,
        max_entries: Some(1),
        ttl: Some(std::time::Duration::ZERO),
    });
    assert!(pinned.insert_visible(1, raster.clone()));
    // TTL prevents new acquisitions, while the visible owner's pin retains membership.
    assert!(pinned.get_visible_lease_for(&raster).is_none());
    assert_eq!(pinned.len(), 1);
    assert!(
        pinned
            .set_limits(rrrah_cache::CacheLimits::bytes(weight - 1))
            .is_none()
    );
    assert_eq!(pinned.len(), 1);
    assert_eq!(
        budget.used(),
        weight,
        "cache clones must share the managed allocation"
    );
    drop((pinned, expired, no_entries, too_small));

    let gpu = common::qualification_gpu().expect("Metal required");
    eprintln!("developed {name} adapter: {}", gpu.adapter_name());
    let view = rrrah_gpu::ViewParameters {
        viewport: [128.0, 96.0],
        zoom: 0.02,
        ..Default::default()
    };
    let expected = gpu.render_raster(&raster, view, [128, 96]);
    assert!(
        expected
            .pixels
            .chunks_exact(4)
            .any(|p| p[..3].iter().any(|v| *v > 0))
    );
    let directory = tempfile::tempdir().unwrap();
    let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: weight + 4096,
                max_entries: Some(1),
                ttl: None,
            },
            queue_bytes: weight,
            queue_count: 1,
            restore_bytes: weight,
        },
        rrrah_core::MemoryBudget::new(weight),
        budget.clone(),
    )
    .unwrap();
    assert_eq!(
        swap.try_enqueue(1, raster.clone()),
        rrrah_cache::SpillAdmission::Queued
    );
    swap.wait_idle().unwrap();
    let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
    assert!(swap.try_get(&1, || false).is_err());
    assert_eq!(swap.stats().errors, 0);
    drop(pressure);
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!(restored.color_space(), raster.color_space());
    let (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) =
        (raster.pixels(), restored.pixels())
    else {
        panic!("float output required")
    };
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(x.to_bits(), y.to_bits());
    }
    assert_eq!(
        gpu.render_raster(&restored, view, [128, 96]).pixels,
        expected.pixels
    );
    drop(restored);
    drop(raster);
    assert_eq!(budget.used(), 0);
    assert_eq!(swap.stats().writes, 1);
    assert_eq!(swap.stats().reads, 1);
    assert_eq!(swap.stats().queued_bytes, 0);
}
