#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "requires pinned CC0 Epson R-D1 ERF source and independent LibRaw sensor dump"]
fn epson_erf_matches_independent_sensor_cache_swap_and_metal() {
    use rrrah_decode::RawDecoder;
    let root =
        std::path::PathBuf::from(std::env::var("RRRAH_EPSON_ERF_CORPUS").expect("RRRAH_EPSON_ERF_CORPUS"));
    {
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(root.join("2680.erf"));
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::NativeRawDecoder.decode(&request).unwrap();
        let native = decoded.mosaic;
        assert_eq!((native.metadata.width, native.metadata.height), (3040, 2024));
        assert!(native.pixels.is_managed());
        let bytes = std::fs::read(root.join("reference.u16le")).unwrap();
        assert_eq!(bytes.len(), 3040 * 2024 * 2);
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
        assert_eq!((crop.x, crop.y, crop.width, crop.height), (0, 0, 3040, 2024));
        let mut metadata = native.metadata.clone();
        metadata.white_balance = [1.75630188, 1.0, 2.124221802, 1.0];
        metadata.black_level = rrrah_core::LevelGrid {
            width: 2,
            height: 2,
            components: 1,
            values: vec![61.0, 64.0, 63.0, 60.0],
        };
        metadata.white_level.0 = vec![4095.0];
        metadata.xyz_to_camera = [
            [0.6826999784, -0.1878000051, -0.07320000231],
            [-0.8428999782, 1.601199985, 0.2563999891],
            [-0.07039999962, 0.05920000002, 0.71450001],
            [0.0; 3],
        ];
        let reference = rrrah_core::DecodedMosaic::new(metadata, std::sync::Arc::new(samples)).unwrap();
        let gpu = common::qualification_gpu().expect("GPU required for Epson ERF qualification");
        eprintln!("Epson ERF qualification adapter: {}", gpu.adapter_name());
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
#[ignore = "requires pinned CC0 Epson R-D1 ERF object 2680"]
fn erf_rejects_bad_color_profile_and_truncated_sensor_without_preview_fallback() {
    let root = std::path::PathBuf::from(std::env::var("RRRAH_EPSON_ERF_CORPUS").unwrap());
    let source = std::fs::read(root.join("2680.erf")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut bad_wb = source.clone();
    bad_wb[1086..1088].fill(0);
    let mut bad_black = source.clone();
    bad_black[1000..1004].fill(255);
    let mut bad_profile = source.clone();
    let offset = source.windows(5).position(|s| s == b"R-D1\0").unwrap();
    bad_profile[offset..offset + 4].copy_from_slice(b"R-X1");
    let truncated = source[..59108 + 9844736 - 1].to_vec();
    let short_metadata = source[..1293].to_vec();
    for (i, bytes) in [bad_wb, bad_black, bad_profile, truncated, short_metadata]
        .into_iter()
        .enumerate()
    {
        let path = directory.path().join(format!("bad-{i}.ERF"));
        std::fs::write(&path, bytes).unwrap();
        let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        assert!(
            rrrah_decode::decode_image(&request).is_err(),
            "mutation {i} unexpectedly decoded"
        );
        assert_eq!(budget.used(), 0, "mutation {i} leaked managed memory");
    }
}
