#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "requires pinned CC0 Sony DSC-F828 SRF and independent full sensor oracle"]
fn sony_srf_matches_independent_sensor_cache_swap_and_metal() {
    use rrrah_decode::RawDecoder;
    {
        let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(std::env::var("RRRAH_SRF_SOURCE").unwrap());
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::NativeRawDecoder.decode(&request).unwrap();
        let native = decoded.mosaic;
        assert_eq!((native.metadata.width, native.metadata.height), (3360, 2460));
        assert!(native.pixels.is_managed());
        let bytes = std::fs::read(std::env::var("RRRAH_SRF_ORACLE").unwrap()).unwrap();
        assert_eq!(bytes.len(), 3360 * 2460 * 2);
        let samples: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
            .collect();
        assert_eq!(&*native.pixels, samples.as_slice());
        assert_eq!(
            native.metadata.cfa.as_ref().unwrap().rgbe_quad().unwrap(),
            [3, 0, 2, 1]
        );
        let mut metadata = native.metadata.clone();
        metadata.white_balance = [397. / 256., 1., 728. / 256., 1.];
        metadata.black_level = rrrah_core::LevelGrid {
            width: 2,
            height: 2,
            components: 1,
            values: vec![492., 558., 491., 621.],
        };
        metadata.white_level.0 = vec![16368.];
        metadata.xyz_to_camera = [
            [0.7924, -0.1910, -0.0777],
            [-0.8226, 1.5459, 0.2998],
            [-0.1517, 0.2199, 0.6818],
            [-0.7242, 1.1401, 0.3481],
        ];
        metadata.crop_area = Some(rrrah_core::Rect::new(6, 0, 3287, 2460));
        metadata.orientation = rrrah_core::Orientation::Normal;
        let reference = rrrah_core::DecodedMosaic::new(metadata, std::sync::Arc::new(samples)).unwrap();
        let gpu = common::qualification_gpu().expect("GPU required for Sony SRF qualification");
        eprintln!("Sony SRF qualification adapter: {}", gpu.adapter_name());
        let actual = gpu.render(&native, [128, 96]);
        let expected = gpu.render(&reference, [128, 96]);
        assert_eq!(actual.pixels, expected.pixels);
        let thumbnail = native.thumbnail_rgba8(128);
        assert_eq!(thumbnail, reference.thumbnail_rgba8(128));
        assert_eq!(thumbnail.len(), 128 * 96 * 4);
        let cpu_frame = independent_viewport_reference(&reference);
        let max_delta = actual
            .pixels
            .iter()
            .zip(&cpu_frame)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        eprintln!("Real SRF independently reconstructed viewport max byte delta: {max_delta}");
        assert!(max_delta <= 2);
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
        let mut ram = rrrah_cache::MosaicRamCache::new(64 * 1024 * 1024);
        assert!(ram.insert(key, native.clone()));
        let lease = ram.get_lease(&key).unwrap();
        assert!(lease.pixels.ptr_eq(&native.pixels));
        assert_eq!(lease.metadata, native.metadata);
        assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
        assert_eq!(gpu.render(&lease, [128, 96]).pixels, expected.pixels);
        drop(lease);
        drop(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).unwrap());
        assert!(ram.is_empty());
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

// Fixed 128x96 cover-view geometry, independent of production RGBE helpers.
// Matrix is LibRaw's rgb_cam[3][4]; sensor samples come from its unpack oracle.
fn independent_viewport_reference(mosaic: &rrrah_core::DecodedMosaic) -> Vec<u8> {
    let matrix = [
        [1.63739419, -0.252761811, -0.003541585058, -0.38109079],
        [0.06718456, 0.8223665357, -0.5305757523, 0.6410246491],
        [-0.0008971288335, -0.3551472425, 1.415327907, -0.05928355828],
    ];
    let scale = (128f64 / 3287.).max(96. / 2460.);
    let quad = [3usize, 0, 2, 1];
    let black = [492., 558., 491., 621.];
    let gains = [397. / 256., 1., 728. / 256., 1.];
    let mut output = Vec::with_capacity(128 * 96 * 4);
    for y in 0..96 {
        for x in 0..128 {
            let uv = [
                (x as f64 + 0.5 - 64.) / (3287. * scale) + 0.5,
                (y as f64 + 0.5 - 48.) / (2460. * scale) + 0.5,
            ];
            let center = [6. + uv[0] * 3286. + 0.5, uv[1] * 2459. + 0.5];
            let radius = 0.5 / scale;
            let low = [(center[0] - radius).max(6.), (center[1] - radius).max(0.)];
            let high = [(center[0] + radius).min(3293.), (center[1] + radius).min(2460.)];
            let mut planes = [0f64; 4];
            let mut total = 0.;
            for sy in ((low[1].floor() as usize / 2 * 2)..high[1].ceil() as usize).step_by(2) {
                for sx in ((low[0].floor() as usize / 2 * 2)..high[0].ceil() as usize).step_by(2) {
                    let weight = (((sx + 2) as f64).min(high[0]) - (sx as f64).max(low[0])).max(0.)
                        * (((sy + 2) as f64).min(high[1]) - (sy as f64).max(low[1])).max(0.);
                    for phase in 0..4 {
                        let channel = quad[phase];
                        let xx = sx + phase % 2;
                        let yy = sy + phase / 2;
                        let raw = f64::from(mosaic.pixels[yy * 3360 + xx]);
                        planes[channel] +=
                            (raw - black[phase]).max(0.) / (16368. - black[phase]) * gains[channel] * weight;
                    }
                    total += weight;
                }
            }
            let linear = matrix.map(|row| row.into_iter().zip(planes).map(|(m, v)| m * v / total).sum());
            output.extend(common::cpu_reference_rgb(linear));
            output.push(255);
        }
    }
    output
}
