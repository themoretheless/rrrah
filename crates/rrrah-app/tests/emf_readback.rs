#[test]
fn authored_emf_routes_by_magic_and_preserves_pixels_and_budget() {
    let directory = tempfile::tempdir().unwrap();
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/emf");
    for name in [
        "black",
        "white",
        "red",
        "green",
        "blue",
        "saved-context",
        "polygon32",
        "polygon16",
        "fill-evenodd",
        "fill-winding",
        "fill-restored",
        "line-restored",
        "line-null-advance",
        "object-reuse",
        "header100",
        "header108",
        "description88",
        "description100",
        "description108",
        "mapping-lowmetric",
        "mapping-highmetric",
        "mapping-anisotropic",
        "mapping-restored",
        "mapping-reflected",
        "bezier32",
        "bezier16",
        "bezierto32",
        "bezierto16",
        "lineto32",
        "lineto16",
    ] {
        let path = directory.path().join("misleading.mrw");
        std::fs::copy(fixtures.join(format!("{name}.emf")), &path).unwrap();
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = rrrah_decode::DecodeRequest::new(&path);
        request.memory_budget = Some(budget.clone());
        let rrrah_decode::DecodedImage::Raster(image) = rrrah_decode::decode_image(&request).unwrap() else {
            panic!("base-header EMF must route as raster")
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
            std::fs::read(fixtures.join(format!("{name}.emf.rgba"))).unwrap()
        );
        drop(image);
        assert_eq!(budget.used(), 0);
        request.image_index = 1;
        assert!(rrrah_decode::decode_image(&request).is_err());
        assert_eq!(budget.used(), 0);
    }
    assert!(rrrah_decode::is_supported_image_path(std::path::Path::new(
        "image.EMF"
    )));
}

#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
fn emf_ram_swap_pressure_and_metal_preserve_the_frame() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("EMF adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/emf");
    for name in [
        "black",
        "white",
        "red",
        "green",
        "blue",
        "saved-context",
        "polygon32",
        "polygon16",
        "fill-evenodd",
        "fill-winding",
        "fill-restored",
        "line-restored",
        "line-null-advance",
        "object-reuse",
        "header100",
        "header108",
        "description88",
        "description100",
        "description108",
        "mapping-lowmetric",
        "mapping-highmetric",
        "mapping-anisotropic",
        "mapping-restored",
        "mapping-reflected",
        "bezier32",
        "bezier16",
        "bezierto32",
        "bezierto16",
        "lineto32",
        "lineto16",
    ] {
        let budget = rrrah_core::MemoryBudget::new(16384);
        let mut request = rrrah_decode::DecodeRequest::new(fixtures.join(format!("{name}.emf")));
        request.memory_budget = Some(budget.clone());
        let native = rrrah_decode::decode_raster(&request).unwrap();
        let golden = rrrah_core::DecodedRaster::new(
            10,
            10,
            rrrah_core::RasterPixels::Rgba8(
                std::sync::Arc::new(std::fs::read(fixtures.join(format!("{name}.emf.rgba"))).unwrap()).into(),
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
