#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "requires pinned CC0 Panasonic FZ8 RAW source"]
fn unqualified_legacy_panasonic_format_fails_without_leaking_memory() {
    use rrrah_decode::RawDecoder;
    let root = std::path::PathBuf::from(std::env::var("RRRAH_PANASONIC_RAW_CORPUS").unwrap());
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let original = std::fs::read(root.join("2282.raw")).unwrap();
    let count = u16::from_le_bytes([original[8], original[9]]) as usize;
    let temporary = tempfile::tempdir().unwrap();
    for (tag, replacement, expected) in [
        (0x002d_u16, 3_u32, "mode 34316 without RawDataOffset"),
        (0x0116, 1, "invalid legacy Panasonic packed strip layout"),
        (0x0117, 16, "invalid legacy Panasonic packed strip layout"),
    ] {
        let mut bytes = original.clone();
        let entry = (0..count)
            .map(|i| 10 + i * 12)
            .find(|i| u16::from_le_bytes([bytes[*i], bytes[*i + 1]]) == tag)
            .unwrap();
        bytes[entry + 8..entry + 12].copy_from_slice(&replacement.to_le_bytes());
        let path = temporary.path().join(format!("unsupported-{tag}.raw"));
        std::fs::write(&path, bytes).unwrap();
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        let error = rrrah_decode::NativeRawDecoder.decode(&request).unwrap_err();
        assert!(error.to_string().contains(expected), "tag {tag}: {error}");
        assert_eq!(budget.used(), 0);
    }
}

#[test]
#[ignore = "requires pinned CC0 Panasonic FZ50 RAW source and independent LibRaw sensor dump"]
fn panasonic_raw_matches_independent_sensor_cache_swap_and_metal() {
    use rrrah_decode::RawDecoder;
    let root = std::path::PathBuf::from(
        std::env::var("RRRAH_PANASONIC_RAW_CORPUS").expect("RRRAH_PANASONIC_RAW_CORPUS"),
    );
    {
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(root.join("2234.raw"));
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::NativeRawDecoder.decode(&request).unwrap();
        let native = decoded.mosaic;
        assert_eq!((native.metadata.width, native.metadata.height), (3690, 2751));
        assert!(native.pixels.is_managed());
        let bytes = std::fs::read(root.join("reference.u16le")).unwrap();
        assert_eq!(bytes.len(), 3690 * 2751 * 2);
        let samples: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
            .collect();
        assert_eq!(&*native.pixels, samples.as_slice());
        let crop = native.metadata.effective_crop();
        assert_eq!((crop.x, crop.y, crop.width, crop.height), (16, 7, 3648, 2736));
        let mut metadata = native.metadata.clone();
        metadata.white_balance = [575.0 / 263.0, 1.0, 419.0 / 263.0, 1.0];
        metadata.black_level.values = vec![0.0; 4];
        metadata.white_level.0 = vec![4095.0; 3];
        metadata.xyz_to_camera = [
            [0.7906, -0.2709, -0.0594],
            [-0.6231, 1.3351, 0.322],
            [-0.1922, 0.2631, 0.6537],
            [0.0; 3],
        ];
        let reference = rrrah_core::DecodedMosaic::new(metadata, std::sync::Arc::new(samples)).unwrap();
        let gpu = common::qualification_gpu().expect("GPU required for Panasonic RAW qualification");
        eprintln!("Panasonic RAW qualification adapter: {}", gpu.adapter_name());
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
        let disk = rrrah_cache::DiskMosaicCache::with_max_bytes(
            directory.path().join("persistent"),
            64 * 1024 * 1024,
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
                        max_bytes: 64 * 1024 * 1024,
                        max_entries: Some(1),
                        ttl: None,
                    },
                    queue_bytes: 64 * 1024 * 1024,
                    queue_count: 1,
                    restore_bytes: 128 * 1024 * 1024,
                },
                rrrah_core::MemoryBudget::new(64 * 1024 * 1024),
                budget.clone(),
            )
            .unwrap();
        swap.enqueue(1, native.clone());
        swap.wait_idle().unwrap();
        let expected_metadata = native.metadata.clone();
        drop(native);
        assert_eq!(budget.used(), 0);
        let pressure = budget.try_reserve(128 * 1024 * 1024).unwrap();
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
#[ignore = "requires pinned CC0 Panasonic FZ8 RAW source and independent LibRaw sensor dump"]
fn panasonic_fz8_matches_independent_sensor_cache_swap_and_metal() {
    use rrrah_decode::RawDecoder;
    let root = std::path::PathBuf::from(
        std::env::var("RRRAH_PANASONIC_RAW_CORPUS").expect("RRRAH_PANASONIC_RAW_CORPUS"),
    );
    {
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(root.join("2282.raw"));
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::NativeRawDecoder.decode(&request).unwrap();
        let native = decoded.mosaic;
        assert_eq!((native.metadata.width, native.metadata.height), (3130, 2319));
        assert!(native.pixels.is_managed());
        let bytes = std::fs::read(root.join("fz8-reference.u16le")).unwrap();
        assert_eq!(bytes.len(), 3130 * 2319 * 2);
        let samples: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
            .collect();
        assert_eq!(&*native.pixels, samples.as_slice());
        let crop = native.metadata.effective_crop();
        assert_eq!((crop.x, crop.y, crop.width, crop.height), (15, 7, 3072, 2304));
        let mut metadata = native.metadata.clone();
        metadata.white_balance = [496.0 / 263.0, 1.0, 446.0 / 263.0, 1.0];
        metadata.black_level.values = vec![0.0; 4];
        metadata.white_level.0 = vec![3967.0; 3];
        metadata.xyz_to_camera = [
            [0.8986, -0.2755, -0.0802],
            [-0.6341, 1.3575, 0.3077],
            [-0.1476, 0.2144, 0.6379],
            [0.0; 3],
        ];
        let reference = rrrah_core::DecodedMosaic::new(metadata, std::sync::Arc::new(samples)).unwrap();
        let gpu = common::qualification_gpu().expect("GPU required for Panasonic RAW qualification");
        eprintln!("Panasonic RAW qualification adapter: {}", gpu.adapter_name());
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
        let disk = rrrah_cache::DiskMosaicCache::with_max_bytes(
            directory.path().join("persistent"),
            64 * 1024 * 1024,
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
                        max_bytes: 64 * 1024 * 1024,
                        max_entries: Some(1),
                        ttl: None,
                    },
                    queue_bytes: 64 * 1024 * 1024,
                    queue_count: 1,
                    restore_bytes: 128 * 1024 * 1024,
                },
                rrrah_core::MemoryBudget::new(64 * 1024 * 1024),
                budget.clone(),
            )
            .unwrap();
        swap.enqueue(1, native.clone());
        swap.wait_idle().unwrap();
        let expected_metadata = native.metadata.clone();
        drop(native);
        assert_eq!(budget.used(), 0);
        let pressure = budget.try_reserve(128 * 1024 * 1024).unwrap();
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
