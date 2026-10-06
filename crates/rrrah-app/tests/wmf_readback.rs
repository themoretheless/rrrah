#[test]
fn authored_wmf_routes_by_magic_and_preserves_pixels_and_budget() {
    let directory = tempfile::tempdir().unwrap();
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/wmf");
    for name in ["red", "green", "blue", "polygon", "saved-context", "object-reuse"] {
        let path = directory.path().join("misleading.mrw");
        std::fs::copy(fixtures.join(format!("{name}.wmf")), &path).unwrap();
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = rrrah_decode::DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        let rrrah_decode::DecodedImage::Raster(image) = rrrah_decode::decode_image(&request).unwrap() else {
            panic!("placeable WMF must route as raster")
        };
        assert_eq!(
            (
                image.width(),
                image.height(),
                image.image_index(),
                image.image_count()
            ),
            (10, 10, 0, 1)
        );
        let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
            panic!()
        };
        assert!(pixels.is_managed());
        assert_eq!(
            &pixels[..],
            std::fs::read(fixtures.join(format!("{name}.wmf.rgba"))).unwrap()
        );
        drop(image);
        assert_eq!(budget.used(), 0);
        request.image_index = 1;
        assert!(rrrah_decode::decode_image(&request).is_err());
        assert_eq!(budget.used(), 0);
    }
    assert!(rrrah_decode::is_supported_image_path(std::path::Path::new(
        "image.WMF"
    )));
}

#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
fn wmf_ram_swap_pressure_and_metal_preserve_the_frame() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("WMF adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/wmf");
    for name in ["red", "green", "blue", "polygon", "saved-context", "object-reuse"] {
        let budget = rrrah_core::MemoryBudget::new(16384);
        let mut request = rrrah_decode::DecodeRequest::new(fixtures.join(format!("{name}.wmf")));
        request.memory_budget = Some(budget.clone());
        let native = rrrah_decode::decode_raster(&request).unwrap();
        let golden = rrrah_core::DecodedRaster::new(
            10,
            10,
            rrrah_core::RasterPixels::Rgba8(
                std::sync::Arc::new(std::fs::read(fixtures.join(format!("{name}.wmf.rgba"))).unwrap()).into(),
            ),
            rrrah_core::RasterColorSpace::Srgb,
        )
        .unwrap()
        .to_linear_srgb()
        .unwrap();
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&budget)).unwrap();
        let parameters = rrrah_gpu::ViewParameters {
            viewport: [96., 64.],
            zoom: 1.,
            ..Default::default()
        };
        let expected = gpu.render_raster(&golden, parameters, [96, 64]).pixels;
        assert_eq!(
            gpu.render_raster(&prepared, parameters, [96, 64]).pixels,
            expected
        );
        let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits {
            max_bytes: prepared.capacity_bytes(),
            max_entries: Some(1),
            ttl: None,
        });
        assert!(ram.insert(1, prepared.clone()));
        let lease = ram.get_lease(&1).unwrap();
        assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
        assert_eq!(gpu.render_raster(&lease, parameters, [96, 64]).pixels, expected);
        drop(lease);
        drop(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).unwrap());
        drop(ram);
        for source in [native, prepared] {
            let weight = source.capacity_bytes();
            let directory = tempfile::tempdir().unwrap();
            let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
                directory.path(),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits {
                        max_bytes: 8192,
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
                swap.try_enqueue(1, source.clone()),
                rrrah_cache::SpillAdmission::Queued
            );
            swap.wait_idle().unwrap();
            let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
            assert!(swap.try_get(&1, || false).is_err());
            assert_eq!(swap.stats().errors, 0);
            drop(pressure);
            let restored = swap.try_get(&1, || false).unwrap().unwrap();
            assert_eq!(
                (restored.width(), restored.height(), restored.color_space()),
                (source.width(), source.height(), source.color_space())
            );
            match (restored.pixels(), source.pixels()) {
                (rrrah_core::RasterPixels::Rgba8(a), rrrah_core::RasterPixels::Rgba8(b)) => {
                    assert_eq!(&a[..], &b[..])
                }
                (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) => {
                    assert_eq!(&a[..], &b[..])
                }
                _ => panic!("precision changed"),
            }
            let display =
                rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
            assert_eq!(gpu.render_raster(&display, parameters, [96, 64]).pixels, expected);
            drop((display, restored, source));
            assert_eq!(
                (swap.stats().writes, swap.stats().reads, swap.stats().errors),
                (1, 1, 0)
            );
        }
        assert_eq!(budget.used(), 0);
    }
}

#[test]
#[ignore = "requires external libwmf arrow01 source and rendered RGBA reference"]
fn external_arrow01_matches_libwmf_exactly() {
    let source = std::env::var("RRRAH_WMF_EXTERNAL_SOURCE").unwrap();
    let reference = std::env::var("RRRAH_WMF_EXTERNAL_RGBA").unwrap();
    let image = rrrah_decode::decode_raster(&rrrah_decode::DecodeRequest::new(source)).unwrap();
    let rrrah_core::RasterPixels::Rgba8(pixels) = image.pixels() else {
        panic!()
    };
    let expected = std::fs::read(reference).unwrap();
    assert_eq!(pixels.len(), expected.len());
    let differing_pixels = pixels
        .chunks_exact(4)
        .zip(expected.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    let max_difference = pixels
        .iter()
        .zip(&expected)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    if let Ok(output) = std::env::var("RRRAH_WMF_NATIVE_RGBA") {
        std::fs::write(output, &pixels[..]).unwrap();
    }
    eprintln!(
        "external WMF {}x{}: differing_pixels={differing_pixels}, max_sample_difference={max_difference}",
        image.width(),
        image.height()
    );
    assert_eq!(differing_pixels, 0, "independent renderer disagreement");
}
