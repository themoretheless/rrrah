#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "requires pinned CC0 Samsung EX1 SRW source and independent LibRaw sensor dump"]
fn samsung_srw_matches_independent_sensor_cache_swap_and_metal() {
    use rrrah_decode::RawDecoder;
    let root = std::path::PathBuf::from(
        std::env::var("RRRAH_SAMSUNG_SRW_CORPUS").expect("RRRAH_SAMSUNG_SRW_CORPUS"),
    );
    {
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(root.join("1204.srw"));
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::NativeRawDecoder.decode(&request).unwrap();
        let native = decoded.mosaic;
        assert_eq!((native.metadata.width, native.metadata.height), (3688, 2780));
        assert!(native.pixels.is_managed());
        let bytes = std::fs::read(root.join("reference.u16le")).unwrap();
        assert_eq!(bytes.len(), 3688 * 2780 * 2);
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
        assert_eq!((crop.x, crop.y, crop.width, crop.height), (0, 2, 3682, 2760));
        let mut metadata = native.metadata.clone();
        metadata.white_balance = [7792.0 / 4096.0, 1.0, 8256.0 / 4096.0, 1.0];
        metadata.black_level = rrrah_core::LevelGrid {
            width: 1,
            height: 1,
            components: 1,
            values: vec![0.0],
        };
        metadata.white_level.0 = vec![15872.0];
        metadata.xyz_to_camera = [
            [0.8898000121, -0.2497999966, -0.09939999878],
            [-0.3143999875, 1.132799983, 0.2065999955],
            [-0.07599999756, 0.1380999982, 0.4575999975],
            [0.0; 3],
        ];
        let reference = rrrah_core::DecodedMosaic::new(metadata, std::sync::Arc::new(samples)).unwrap();
        let gpu = common::qualification_gpu().expect("GPU required for Samsung SRW qualification");
        eprintln!("Samsung SRW qualification adapter: {}", gpu.adapter_name());
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
        assert_eq!(
            swap.try_enqueue(1, native.clone()),
            rrrah_cache::SpillAdmission::Queued
        );
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
#[ignore = "requires pinned CC0 Samsung EX1 SRW source"]
fn srw_rejects_invalid_wb_profile_and_truncated_sensor_without_leaks() {
    let root = std::path::PathBuf::from(std::env::var("RRRAH_SAMSUNG_SRW_CORPUS").unwrap());
    let source = std::fs::read(root.join("1204.srw")).unwrap();
    let mut cases = Vec::new();
    // Pin every mutation to the qualified source layout before changing it.
    assert_eq!(&source[94..97], b"EX1");
    assert_eq!(u32::from_be_bytes(source[17854..17858].try_into().unwrap()), 8097);
    let mut bad = source.clone();
    bad[94..97].copy_from_slice(b"EX9");
    cases.push(bad);
    let mut bad = source.clone();
    bad[17854..17858].copy_from_slice(&305u32.to_be_bytes());
    cases.push(bad); // Key subtraction produces zero.
    let mut bad = source.clone();
    bad[17854..17858].fill(0);
    cases.push(bad); // Key subtraction underflows.
    cases.push(source[..1194033 + 20505280 - 1].to_vec());
    let dir = tempfile::tempdir().unwrap();
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    for (i, bytes) in cases.iter().enumerate() {
        let path = dir.path().join(format!("invalid-{i}.SRW"));
        std::fs::write(&path, bytes).unwrap();
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        assert!(rrrah_decode::decode_image(&request).is_err(), "case {i}");
        assert_eq!(budget.used(), 0, "case {i}");
    }
}
