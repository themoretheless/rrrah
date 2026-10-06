#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "requires pinned CC0 Sony DSC-R1 SR2 source and independent LibRaw sensor dump"]
fn sony_sr2_matches_independent_sensor_cache_swap_and_metal() {
    use rrrah_decode::RawDecoder;
    let root =
        std::path::PathBuf::from(std::env::var("RRRAH_SONY_SR2_CORPUS").expect("RRRAH_SONY_SR2_CORPUS"));
    {
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(root.join("3221.sr2"));
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::NativeRawDecoder.decode(&request).unwrap();
        let native = decoded.mosaic;
        assert_eq!((native.metadata.width, native.metadata.height), (3984, 2608));
        assert!(native.pixels.is_managed());
        let bytes = std::fs::read(root.join("reference.u16le")).unwrap();
        assert_eq!(bytes.len(), 3984 * 2608 * 2);
        let samples: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
            .collect();
        assert_eq!(&*native.pixels, samples.as_slice());
        assert_eq!(
            native.metadata.cfa.as_ref().unwrap().cells,
            [
                rrrah_core::CfaColor::Green,
                rrrah_core::CfaColor::Red,
                rrrah_core::CfaColor::Blue,
                rrrah_core::CfaColor::Green
            ]
        );
        let crop = native.metadata.effective_crop();
        assert_eq!((crop.x, crop.y, crop.width, crop.height), (0, 0, 3925, 2608));
        let mut metadata = native.metadata.clone();
        metadata.white_balance = [500.0 / 256.0, 1.0, 385.0 / 256.0, 1.0];
        metadata.black_level = rrrah_core::LevelGrid {
            width: 2,
            height: 2,
            components: 1,
            values: vec![511.0, 514.0, 512.0, 513.0],
        };
        metadata.white_level.0 = vec![16383.0];
        metadata.xyz_to_camera = [
            [0.8512, -0.2641, -0.0694],
            [-0.8042, 1.5670, 0.2526],
            [-0.1821, 0.2117, 0.7414],
            [0.0; 3],
        ];
        let reference = rrrah_core::DecodedMosaic::new(metadata, std::sync::Arc::new(samples)).unwrap();
        let gpu = common::qualification_gpu().expect("GPU required for Sony SR2 qualification");
        eprintln!("Sony SR2 qualification adapter: {}", gpu.adapter_name());
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
