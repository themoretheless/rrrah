#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "requires pinned external AI corpus; set RRRAH_AI_CORPUS"]
fn real_ai_pages_swap_preserves_pixels_and_metal_frame() {
    let corpus = std::path::PathBuf::from(std::env::var("RRRAH_AI_CORPUS").unwrap());
    qualify_pdf_swap_pages(
        &corpus,
        &[
            ("VectorApple.ai", 0, (301, 246), 1),
            ("one.ai", 0, (1366, 768), 2),
            ("one.ai", 1, (177, 175), 2),
        ],
    );
}

#[test]
#[ignore = "requires actual GPU adapter"]
fn short_flag_tensor_pages_swap_preserves_pixels_and_metal_frame() {
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pdf");
    qualify_pdf_swap_pages(
        &corpus,
        &[
            ("tensor-shared-edge-2bit.pdf", 0, (64, 32), 1),
            ("tensor-shared-edge-4bit.pdf", 0, (64, 32), 1),
            ("tensor-shared-edge-8bit.pdf", 0, (64, 32), 1),
            ("tensor-fractional-rgb.pdf", 0, (32, 32), 1),
        ],
    );
}

fn qualify_pdf_swap_pages(corpus: &std::path::Path, cases: &[(&str, usize, (u32, u32), usize)]) {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("PDF adapter: {}", gpu.adapter_name());
    for &(name, index, extent, count) in cases {
        let directory = tempfile::tempdir().unwrap();
        let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(corpus.join(name));
        request.image_index = index;
        request.memory_budget = Some(budget.clone());
        let native = rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!((native.width(), native.height()), extent);
        assert_eq!((native.image_index(), native.image_count()), (index, count));
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&budget)).unwrap();
        let parameters = rrrah_gpu::ViewParameters {
            viewport: [96., 64.],
            zoom: 1.,
            ..Default::default()
        };
        // This baseline proves transport preservation, not independent AI color correctness.
        let expected = gpu.render_raster(&prepared, parameters, [96, 64]).pixels;
        for source in [native, prepared] {
            let weight = source.capacity_bytes();
            let swap = rrrah_cache::ImageSwapCache::<u32, rrrah_core::DecodedRaster>::new_with_budgets(
                directory.path(),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits::bytes(32 * 1024 * 1024),
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
            assert_eq!(swap.stats().writes, 1);
            assert_eq!(swap.stats().queued_bytes, 0);
            let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
            assert!(swap.try_get(&1, || false).is_err());
            assert_eq!(swap.stats().errors, 0);
            drop(pressure);
            let restored = swap.try_get(&1, || false).unwrap().unwrap();
            assert_eq!((restored.width(), restored.height()), extent);
            assert_eq!((restored.image_index(), restored.image_count()), (index, count));
            assert_eq!(restored.color_space(), source.color_space());
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
        }
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn selected_pdf_pages_ram_swap_and_metal_match_opaque_reference() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("PDF adapter: {}", gpu.adapter_name());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("misleading.mrw");
    assert!(rrrah_decode::is_supported_image_path(std::path::Path::new(
        "document.PDF"
    )));
    for (fixture, index, count, color) in [
        ("red-green-pages.pdf", 0, 2, [255u8, 0, 0, 255]),
        ("red-green-pages.pdf", 1, 2, [0, 255, 0, 255]),
        ("red-alpha.pdf", 0, 1, [255, 0, 0, 128]),
    ] {
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/pdf")
                .join(fixture),
            &path,
        )
        .unwrap();
        let budget = rrrah_core::MemoryBudget::new(16384);
        let mut request = rrrah_decode::DecodeRequest::new(&path);
        request.image_index = index;
        request.memory_budget = Some(budget.clone());
        let rrrah_decode::DecodedImage::Raster(native) = rrrah_decode::decode_image(&request).unwrap() else {
            panic!()
        };
        assert_eq!(
            (
                native.width(),
                native.height(),
                native.image_index(),
                native.image_count()
            ),
            (10, 10, index, count)
        );
        let rrrah_core::RasterPixels::Rgba8(values) = native.pixels() else {
            panic!()
        };
        assert!(values.is_managed());
        assert!(values.chunks_exact(4).all(|p| p == color));
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&budget)).unwrap();
        let golden = rrrah_core::DecodedRaster::new(
            10,
            10,
            rrrah_core::RasterPixels::Rgba32Float(
                std::sync::Arc::new(color.map(|v| f32::from(v) / 255.).repeat(100)).into(),
            ),
            rrrah_core::RasterColorSpace::LinearSrgb,
        )
        .unwrap();
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

#[test]
fn independently_referenced_pdfs_and_profiled_jpegs_match_through_ram_swap_and_metal() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("Independent raster adapter: {}", gpu.adapter_name());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pdf");
    let mut cases: Vec<_> = [
        ("cairo-rect", "cairo-rect.rgba", 48, 36, 0u8),
        ("rotate90", "rotate90.rgba", 20, 10, 0),
        ("geometry-unit", "geometry-unit.rgba", 20, 40, 0),
        (
            "gradient-center-strips-cmyk",
            "cmyk-float-littlecms.rgba",
            12,
            8,
            0,
        ),
        ("cmyk-image8", "cmyk-image8-littlecms.rgba", 4721, 1, 1),
        ("cmyk-app14-profiled", "cmyk-jpeg-app14-littlecms.rgba", 64, 8, 0),
        ("ycck-app14-profiled", "ycck-jpeg-app14-littlecms.rgba", 64, 8, 0),
    ]
    .into_iter()
    .map(|(name, golden, width, height, tolerance)| {
        (name.to_owned(), golden.to_owned(), width, height, tolerance)
    })
    .collect();
    for kind in ["cmyk", "ycck"] {
        for orientation in 1..=8 {
            let name = format!("{kind}-profiled-orientation-{orientation}");
            let (width, height) = if orientation >= 5 { (13, 17) } else { (17, 13) };
            cases.push((name.clone(), format!("../raster/{name}.rgba"), width, height, 2));
        }
    }
    for (name, golden_name, width, height, source_tolerance) in cases {
        let directory = tempfile::tempdir().unwrap();
        let budget = rrrah_core::MemoryBudget::new(256 * 1024);
        let path = if name.contains("profiled") {
            root.parent().unwrap().join("raster").join(format!("{name}.jpg"))
        } else {
            root.join(format!("{name}.pdf"))
        };
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        let native = rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!((native.width(), native.height()), (width, height));
        let reference = std::fs::read(root.join(golden_name)).unwrap();
        let rrrah_core::RasterPixels::Rgba8(values) = native.pixels() else {
            panic!()
        };
        assert_eq!(values.len(), reference.len());
        assert!(
            values
                .iter()
                .zip(&reference)
                .all(|(a, b)| a.abs_diff(*b) <= source_tolerance)
        );
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&budget)).unwrap();
        let golden = rrrah_core::DecodedRaster::new(
            width,
            height,
            rrrah_core::RasterPixels::Rgba8(std::sync::Arc::new(reference).into()),
            rrrah_core::RasterColorSpace::Srgb,
        )
        .unwrap()
        .to_linear_srgb()
        .unwrap();
        let target = [width.max(96), 64];
        let parameters = rrrah_gpu::ViewParameters {
            viewport: [target[0] as f32, target[1] as f32],
            zoom: 1.,
            pan: if name == "cmyk-image8" {
                [0.0, 0.5]
            } else {
                [0.0, 0.0]
            },
            ..Default::default()
        };
        let oracle_frame = gpu.render_raster(&golden, parameters, target).pixels;
        let expected = gpu.render_raster(&prepared, parameters, target).pixels;
        // Compare displayed output to the independently converted reference.
        let max_error = expected
            .iter()
            .zip(&oracle_frame)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        eprintln!("{name}: independent GPU max channel error {max_error}");
        assert!(max_error <= source_tolerance);
        if name == "cmyk-image8" {
            let colors: std::collections::HashSet<_> =
                expected.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect();
            assert!(
                colors.len() > 128,
                "CMYK corpus must actually appear in GPU output"
            );
        }
        if name.contains("profiled") {
            let colors: std::collections::HashSet<_> =
                expected.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect();
            assert!(colors.len() >= 8, "JPEG stripes must appear in GPU output");
        }
        let mut ram =
            rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(prepared.capacity_bytes()));
        assert!(ram.insert(1u8, prepared.clone()));
        let lease = ram.get_lease(&1).unwrap();
        assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
        assert_eq!(gpu.render_raster(&lease, parameters, target).pixels, expected);
        drop(lease);
        drop(ram);
        for source in [native, prepared] {
            let weight = source.capacity_bytes();
            let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
                directory.path(),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits::bytes(256 * 1024),
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
            assert_eq!(gpu.render_raster(&display, parameters, target).pixels, expected);
            drop((source, restored, display));
        }
        assert_eq!(budget.used(), 0);
    }
}
