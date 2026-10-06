#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "requires pinned CC0 Minolta Dynax 7D MRW source and independent LibRaw sensor dump"]
fn minolta_mrw_matches_independent_sensor_cache_swap_and_metal() {
    use rrrah_decode::RawDecoder;
    let root = std::path::PathBuf::from(
        std::env::var("RRRAH_MINOLTA_MRW_CORPUS").expect("RRRAH_MINOLTA_MRW_CORPUS"),
    );
    {
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(root.join("1826.mrw"));
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::NativeRawDecoder.decode(&request).unwrap();
        let native = decoded.mosaic;
        assert_eq!((native.metadata.width, native.metadata.height), (3016, 2008));
        assert!(native.pixels.is_managed());
        let bytes = std::fs::read(root.join("reference.u16le")).unwrap();
        assert_eq!(bytes.len(), 3016 * 2008 * 2);
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
        let crop = native.metadata.effective_crop();
        assert_eq!((crop.x, crop.y, crop.width, crop.height), (0, 0, 3016, 2008));
        let mut metadata = native.metadata.clone();
        metadata.white_balance = [435.0 / 256.0, 1.0, 435.0 / 256.0, 1.0];
        metadata.black_level = rrrah_core::LevelGrid {
            width: 1,
            height: 1,
            components: 1,
            values: vec![0.0],
        };
        metadata.white_level.0 = vec![4091.0];
        metadata.xyz_to_camera = [
            [1.0239, -0.3104, -0.1099],
            [-0.8037, 1.5727, 0.2451],
            [-0.0927, 0.0925, 0.6871],
            [0.0; 3],
        ];
        let reference = rrrah_core::DecodedMosaic::new(metadata, std::sync::Arc::new(samples)).unwrap();
        let gpu = common::qualification_gpu().expect("GPU required for Minolta MRW qualification");
        eprintln!("Minolta MRW qualification adapter: {}", gpu.adapter_name());
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
#[ignore = "requires pinned CC0 Minolta Dynax 7D MRW source"]
fn mrw_rejects_unqualified_profiles_and_malformed_blocks_without_memory_leaks() {
    let root = std::path::PathBuf::from(std::env::var("RRRAH_MINOLTA_MRW_CORPUS").unwrap());
    let source = std::fs::read(root.join("1826.mrw")).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let mut cases = Vec::new();
    let mut bytes = source.clone();
    bytes[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
    cases.push(bytes);
    let mut bytes = source.clone();
    bytes[32] = 14; // PRD stored precision, qualified mode requires 12.
    cases.push(bytes);
    let mut bytes = source.clone();
    bytes[52..54].fill(0); // WBG first native coefficient.
    cases.push(bytes);
    let mut bytes = source.clone();
    let locations: Vec<_> = bytes
        .windows(8)
        .enumerate()
        .filter_map(|(i, v)| (v == b"DYNAX 7D").then_some(i))
        .collect();
    assert!(!locations.is_empty());
    for offset in locations {
        bytes[offset..offset + 8].copy_from_slice(b"OTHER 7D");
    }
    cases.push(bytes);
    cases.push(source[..source.len() - 1].to_vec());
    for (i, bytes) in cases.iter().enumerate() {
        let path = temporary.path().join(format!("malformed-{i}.MRW"));
        std::fs::write(&path, bytes).unwrap();
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        assert!(
            rrrah_decode::decode_image(&request).is_err(),
            "malformed case {i}"
        );
        assert_eq!(budget.used(), 0, "case {i} retained managed allocation");
    }
}

#[test]
#[ignore = "requires pinned CC0 Minolta DiMAGE A2 MRW source and independent LibRaw sensor dump"]
fn minolta_a2_mrw_matches_independent_sensor_cache_swap_and_metal() {
    use rrrah_decode::RawDecoder;
    let root =
        std::path::PathBuf::from(std::env::var("RRRAH_MINOLTA_A2_CORPUS").expect("RRRAH_MINOLTA_A2_CORPUS"));
    {
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(root.join("4419.mrw"));
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::NativeRawDecoder.decode(&request).unwrap();
        let native = decoded.mosaic;
        assert_eq!((native.metadata.width, native.metadata.height), (3272, 2456));
        assert!(native.pixels.is_managed());
        let bytes = std::fs::read(root.join("reference.u16le")).unwrap();
        assert_eq!(bytes.len(), 3272 * 2456 * 2);
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
        let crop = native.metadata.effective_crop();
        assert_eq!((crop.x, crop.y, crop.width, crop.height), (0, 0, 3272, 2456));
        let mut metadata = native.metadata.clone();
        metadata.white_balance = [513.0 / 256.0, 1.0, 368.0 / 256.0, 1.0];
        metadata.black_level = rrrah_core::LevelGrid {
            width: 1,
            height: 1,
            components: 1,
            values: vec![0.0],
        };
        metadata.white_level.0 = vec![3983.0];
        metadata.xyz_to_camera = [
            [0.9097, -0.2726, -0.1053],
            [-0.8073, 1.5506, 0.2762],
            [-0.0966, 0.0981, 0.7763],
            [0.0; 3],
        ];
        let reference = rrrah_core::DecodedMosaic::new(metadata, std::sync::Arc::new(samples)).unwrap();
        let gpu = common::qualification_gpu().expect("GPU required for Minolta MRW qualification");
        eprintln!("Minolta MRW qualification adapter: {}", gpu.adapter_name());
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
#[ignore = "requires pinned CC0 Minolta Dynax 7D MRW source"]
fn mrw_magic_overrides_wrong_extensions_and_rejects_truncated_container() {
    use rrrah_decode::RawDecoder;
    let root = std::path::PathBuf::from(std::env::var("RRRAH_MINOLTA_MRW_CORPUS").unwrap());
    let directory = tempfile::tempdir().unwrap();
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(root.join("1826.mrw"));
    request.memory_budget = Some(budget.clone());
    let expected = rrrah_decode::NativeRawDecoder.decode(&request).unwrap().mosaic;
    let recipe = rrrah_decode::NativeRawDecoder.mosaic_recipe(&request).unwrap();
    for suffix in ["jpg", "dng", "bin", "MRW"] {
        let path = directory.path().join(format!("sensor.{suffix}"));
        std::fs::copy(root.join("1826.mrw"), &path).unwrap();
        request.path = path;
        assert_eq!(
            rrrah_decode::image_source_kind(&request).unwrap(),
            rrrah_decode::ImageSourceKind::Sensor
        );
        assert_eq!(
            rrrah_decode::NativeRawDecoder.mosaic_recipe(&request).unwrap(),
            recipe
        );
        let rrrah_decode::DecodedImage::Sensor(actual) = rrrah_decode::decode_image(&request).unwrap() else {
            panic!("MRM must select sensor");
        };
        assert_eq!(actual.mosaic.metadata, expected.metadata);
        assert_eq!(actual.mosaic.pixels, expected.pixels);
    }
    drop(expected);
    assert_eq!(budget.used(), 0);
    request.path = directory.path().join("truncated.jpg");
    std::fs::write(&request.path, b"\0MRM").unwrap();
    assert_eq!(
        rrrah_decode::image_source_kind(&request).unwrap(),
        rrrah_decode::ImageSourceKind::Sensor
    );
    assert!(rrrah_decode::decode_image(&request).is_err());
    assert_eq!(budget.used(), 0);
}
