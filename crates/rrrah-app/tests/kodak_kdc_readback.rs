#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "requires pinned CC0 Kodak P880 KDC source and independent LibRaw sensor dump"]
fn kodak_kdc_matches_independent_sensor_cache_swap_and_metal() {
    use rrrah_decode::RawDecoder;
    let root =
        std::path::PathBuf::from(std::env::var("RRRAH_KODAK_KDC_CORPUS").expect("RRRAH_KODAK_KDC_CORPUS"));
    {
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(root.join("2339.kdc"));
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::NativeRawDecoder.decode(&request).unwrap();
        let native = decoded.mosaic;
        assert_eq!((native.metadata.width, native.metadata.height), (3280, 2454));
        assert!(native.pixels.is_managed());
        let bytes = std::fs::read(root.join("reference.u16le")).unwrap();
        assert_eq!(bytes.len(), 3280 * 2454 * 2);
        let samples: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
            .collect();
        assert_eq!(&*native.pixels, samples.as_slice());
        assert_eq!(
            native.metadata.cfa.as_ref().unwrap().cells,
            [
                rrrah_core::CfaColor::Blue,
                rrrah_core::CfaColor::Green,
                rrrah_core::CfaColor::Green,
                rrrah_core::CfaColor::Red
            ]
        );
        let crop = native.metadata.effective_crop();
        assert_eq!((crop.x, crop.y, crop.width, crop.height), (0, 0, 3280, 2454));
        let mut metadata = native.metadata.clone();
        metadata.white_balance = [102656.0 / 65536.0, 1.0, 92672.0 / 65536.0, 1.0];
        metadata.black_level = rrrah_core::LevelGrid {
            width: 1,
            height: 1,
            components: 1,
            values: vec![0.0],
        };
        metadata.white_level.0 = vec![3963.0];
        metadata.xyz_to_camera = [
            [1.280500054, -0.4661999941, -0.1376000047],
            [-0.7480000257, 1.52670002, 0.2360000014],
            [-0.1625999957, 0.2194000036, 0.7904000282],
            [0.0; 3],
        ];
        let reference = rrrah_core::DecodedMosaic::new(metadata, std::sync::Arc::new(samples)).unwrap();
        let gpu = common::qualification_gpu().expect("GPU required for Kodak KDC qualification");
        eprintln!("Kodak KDC qualification adapter: {}", gpu.adapter_name());
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
#[ignore = "requires pinned CC0 Kodak P880 KDC source"]
fn kdc_rejects_unqualified_and_malformed_sensor_without_preview_or_leaks() {
    let root = std::path::PathBuf::from(std::env::var("RRRAH_KODAK_KDC_CORPUS").unwrap());
    let source = std::fs::read(root.join("2339.kdc")).unwrap();
    fn field(bytes: &[u8], ifd: usize, tag: u16) -> usize {
        let count = u16::from_le_bytes(bytes[ifd..ifd + 2].try_into().unwrap()) as usize;
        (0..count)
            .map(|i| ifd + 2 + i * 12)
            .find(|p| u16::from_le_bytes(bytes[*p..*p + 2].try_into().unwrap()) == tag)
            .unwrap()
            + 8
    }
    let private = u32::from_le_bytes(
        source[field(&source, 8, 0xfe00)..field(&source, 8, 0xfe00) + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let raw = u32::from_le_bytes(
        source[field(&source, 8, 330)..field(&source, 8, 330) + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let mut cases = Vec::new();
    let mut bytes = source.clone();
    let wb = u32::from_le_bytes(
        bytes[field(&bytes, private, 0xfa25)..field(&bytes, private, 0xfa25) + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    bytes[wb..wb + 4].fill(0);
    cases.push(bytes);
    let mut bytes = source.clone();
    let p = field(&bytes, private, 0xfa13);
    bytes[p..p + 2].fill(0);
    cases.push(bytes);
    let mut bytes = source.clone();
    let p = field(&bytes, private, 0xfa0d);
    bytes[p] = 255;
    cases.push(bytes);
    let mut bytes = source.clone();
    let p = field(&bytes, raw, 0xfd09);
    bytes[p..p + 4].fill(0);
    cases.push(bytes);
    let mut bytes = source.clone();
    let p = bytes.windows(4).position(|s| s == b"P880").unwrap();
    bytes[p..p + 4].copy_from_slice(b"P999");
    cases.push(bytes);
    let mut bytes = source.clone();
    let p = field(&bytes, raw, 0xfd04);
    bytes[p - 6..p - 4].copy_from_slice(&7u16.to_le_bytes());
    cases.push(bytes);
    let mut bytes = source.clone();
    let p = field(&bytes, private, 0xfa25);
    bytes[p - 6..p - 4].copy_from_slice(&3u16.to_le_bytes());
    cases.push(bytes);
    cases.push(source[..12150832 - 1].to_vec());
    let temporary = tempfile::tempdir().unwrap();
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let uppercase = temporary.path().join("good.KDC");
    std::fs::write(&uppercase, &source).unwrap();
    let mut request = rrrah_decode::DecodeRequest::new(uppercase);
    request.memory_budget = Some(budget.clone());
    let rrrah_decode::DecodedImage::Sensor(output) = rrrah_decode::decode_image(&request).unwrap() else {
        panic!("uppercase KDC selected a preview instead of the sensor");
    };
    let reference = std::fs::read(root.join("reference.u16le")).unwrap();
    assert_eq!(output.mosaic.pixels.len() * 2, reference.len());
    assert!(
        output
            .mosaic
            .pixels
            .iter()
            .zip(reference.chunks_exact(2))
            .all(|(sample, bytes)| *sample == u16::from_le_bytes([bytes[0], bytes[1]]))
    );
    drop(output);
    assert_eq!(budget.used(), 0);
    for (i, bytes) in cases.into_iter().enumerate() {
        let path = temporary.path().join(format!("bad-{i}.KDC"));
        std::fs::write(&path, bytes).unwrap();
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        assert!(rrrah_decode::decode_image(&request).is_err(), "invalid case {i}");
        assert_eq!(budget.used(), 0, "invalid case {i} retained managed memory");
    }
}
