#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
#[test]
fn basis_codecs_mips_ram_swap_and_metal_preserve_opaque_reference() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("Basis adapter: {}", gpu.adapter_name());
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/basis");
    assert!(rrrah_decode::is_supported_image_path(std::path::Path::new(
        "texture.BASIS"
    )));
    for name in [
        "etc1s-srgb",
        "etc1s-linear",
        "uastc-srgb",
        "uastc-linear",
        "etc1s-colored-alpha",
        "uastc-colored-alpha",
        "etc1s-multi",
        "uastc-multi",
    ] {
        let colored = name.ends_with("colored-alpha");
        let multi = name.ends_with("multi");
        let levels = if multi {
            7
        } else if colored {
            1
        } else {
            4
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("misleading.mrw");
        std::fs::copy(root.join(format!("{name}.basis")), &path).unwrap();
        for index in 0..levels {
            let budget = rrrah_core::MemoryBudget::new(16384);
            let mut request = rrrah_decode::DecodeRequest::new(&path);
            request.image_index = index;
            request.memory_budget = Some(budget.clone());
            let rrrah_decode::DecodedImage::Raster(native) = rrrah_decode::decode_image(&request).unwrap()
            else {
                panic!()
            };
            let dimension = if multi && index >= 4 {
                4 >> (index - 4)
            } else {
                8 >> index
            };
            let (width, height) = if colored { (7, 5) } else { (dimension, dimension) };
            assert_eq!(
                (
                    native.width(),
                    native.height(),
                    native.image_count(),
                    native.image_index()
                ),
                (width, height, levels, index)
            );
            let rrrah_core::RasterPixels::Rgba8(values) = native.pixels() else {
                panic!()
            };
            assert!(values.is_managed());
            if colored {
                let reference = std::fs::read(root.join("colored-alpha.rgba")).unwrap();
                assert_eq!(values.len(), reference.len());
                assert!(values.iter().zip(reference).all(|(&v, r)| v.abs_diff(r) <= 8));
            } else if multi {
                let color = if index < 4 {
                    [255u8, 0, 0, 255]
                } else {
                    [0, 255, 0, 255]
                };
                assert!(
                    values
                        .chunks_exact(4)
                        .all(|pixel| pixel.iter().zip(color).all(|(&v, c)| v.abs_diff(c) <= 8))
                );
            } else {
                assert!(values.iter().all(|&v| v == 255));
            }
            let declared = if name == "etc1s-linear" {
                rrrah_core::RasterColorSpace::LinearSrgb
            } else {
                rrrah_core::RasterColorSpace::Srgb
            };
            assert_eq!(native.color_space(), &declared);
            let prepared =
                rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&budget)).unwrap();
            let golden = rrrah_core::DecodedRaster::new(
                dimension,
                dimension,
                rrrah_core::RasterPixels::Rgba32Float(
                    std::sync::Arc::new(vec![1.; dimension as usize * dimension as usize * 4]).into(),
                ),
                rrrah_core::RasterColorSpace::LinearSrgb,
            )
            .unwrap();
            let parameters = rrrah_gpu::ViewParameters {
                viewport: [96., 64.],
                zoom: 1.,
                ..Default::default()
            };
            // Colored lossy cases use the admitted first-render baseline only for
            // exact cache/swap preservation; their source fidelity is checked above.
            let expected = gpu
                .render_raster(
                    if colored || multi { &prepared } else { &golden },
                    parameters,
                    [96, 64],
                )
                .pixels;
            assert_eq!(
                gpu.render_raster(&prepared, parameters, [96, 64]).pixels,
                expected
            );
            let mut ram =
                rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(prepared.capacity_bytes()));
            assert!(ram.insert(1u8, prepared.clone()));
            let lease = ram.get_lease(&1).unwrap();
            assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
            assert_eq!(gpu.render_raster(&lease, parameters, [96, 64]).pixels, expected);
            drop(lease);
            drop(ram);
            for source in [native, prepared] {
                let weight = source.capacity_bytes();
                let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
                    directory.path(),
                    rrrah_cache::ImageSwapConfig {
                        limits: rrrah_cache::CacheLimits::bytes(8192),
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
                    (
                        restored.image_index(),
                        restored.image_count(),
                        restored.color_space()
                    ),
                    (source.image_index(), source.image_count(), source.color_space())
                );
                match (source.pixels(), restored.pixels()) {
                    (rrrah_core::RasterPixels::Rgba8(a), rrrah_core::RasterPixels::Rgba8(b)) => {
                        assert_eq!(&**a, &**b)
                    }
                    (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) => {
                        assert_eq!(a.len(), b.len());
                        assert!(a.iter().zip(b.iter()).all(|(a, b)| a.to_bits() == b.to_bits()));
                    }
                    _ => panic!("swap changed pixel representation"),
                }
                let display =
                    rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
                assert_eq!(gpu.render_raster(&display, parameters, [96, 64]).pixels, expected);
                drop((source, restored, display));
            }
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
#[ignore = "requires pinned external upstream corpus; set RRRAH_BASIS_CORPUS"]
fn external_producer_ram_swap_and_metal_preserve_decoded_samples() {
    let root = std::path::PathBuf::from(std::env::var_os("RRRAH_BASIS_CORPUS").expect("corpus path"));
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("External Basis adapter: {}", gpu.adapter_name());
    for name in ["alpha3", "kodim01_mipmapped", "kodim03", "kodim03_uastc"] {
        let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(root.join(format!("{name}.basis")));
        request.memory_budget = Some(budget.clone());
        let first = rrrah_decode::decode_raster(&request).unwrap();
        let count = first.image_count();
        let dimensions = (first.width(), first.height());
        drop(first);
        assert_eq!(budget.used(), 0);
        eprintln!("External Basis {name}: {count} selections");
        for index in 0..count {
            request.image_index = index;
            let native = rrrah_decode::decode_raster(&request).unwrap();
            assert_eq!((native.image_index(), native.image_count()), (index, count));
            if name == "kodim01_mipmapped" {
                assert_eq!(
                    (native.width(), native.height()),
                    ((dimensions.0 >> index).max(1), (dimensions.1 >> index).max(1))
                );
            }

            let prepared =
                rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&budget)).unwrap();
            let parameters = rrrah_gpu::ViewParameters {
                viewport: [96., 64.],
                zoom: 1.,
                ..Default::default()
            };
            let expected = gpu.render_raster(&prepared, parameters, [96, 64]).pixels;
            let mut ram =
                rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(prepared.capacity_bytes()));
            assert!(ram.insert(1u8, prepared.clone()));
            let lease = ram.get_lease(&1).unwrap();
            assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
            assert_eq!(gpu.render_raster(&lease, parameters, [96, 64]).pixels, expected);
            drop((lease, ram));
            for source in [native, prepared] {
                let directory = tempfile::tempdir().unwrap();
                let weight = source.capacity_bytes();
                let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
                    directory.path(),
                    rrrah_cache::ImageSwapConfig {
                        limits: rrrah_cache::CacheLimits::bytes(64 * 1024 * 1024),
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
                    (
                        restored.width(),
                        restored.height(),
                        restored.image_count(),
                        restored.image_index(),
                        restored.color_space()
                    ),
                    (
                        source.width(),
                        source.height(),
                        source.image_count(),
                        source.image_index(),
                        source.color_space()
                    )
                );
                match (source.pixels(), restored.pixels()) {
                    (rrrah_core::RasterPixels::Rgba8(a), rrrah_core::RasterPixels::Rgba8(b)) => {
                        assert_eq!(&**a, &**b)
                    }
                    (rrrah_core::RasterPixels::Rgba32Float(a), rrrah_core::RasterPixels::Rgba32Float(b)) => {
                        assert_eq!(a.len(), b.len());
                        assert!(a.iter().zip(b.iter()).all(|(a, b)| a.to_bits() == b.to_bits()));
                    }
                    _ => panic!("changed sample representation"),
                }
                let display =
                    rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
                assert_eq!(gpu.render_raster(&display, parameters, [96, 64]).pixels, expected);
                drop((source, restored, display));
            }
            assert_eq!(budget.used(), 0);
        }
        request.image_index = count;
        assert!(rrrah_decode::decode_raster(&request).is_err());
        assert_eq!(budget.used(), 0);
    }
}
