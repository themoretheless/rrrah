#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
use rrrah_decode::*;

#[test]
#[ignore = "requires actual GPU adapter; run explicitly with RRRAH_GPU_BACKEND=metal"]
fn independent_eps_import_pixels_survive_swap_and_metal() {
    let gpu = common::qualification_gpu().expect("required GPU adapter");
    eprintln!("Independent EPS import adapter: {}", gpu.adapter_name());
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/eps/import-pixel-ghostscript-reference.json"
    ))
    .unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 5);
    compare_independent_eps_cases_through_swap_and_gpu(&gpu, cases);
}

#[test]
#[ignore = "requires actual GPU adapter; run explicitly with RRRAH_GPU_BACKEND=metal"]
fn independent_eps_gray_rgb_ramp_survives_swap_and_metal() {
    let gpu = common::qualification_gpu().expect("required GPU adapter");
    eprintln!("EPS ramp lifecycle adapter: {}", gpu.adapter_name());
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/eps/color-ramp-ghostscript-reference.json"
    ))
    .unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 70);
    // Keep the four known reference color quantization disagreements explicit.
    // All admitted cases use exact pixel comparisons, including after GPU rendering.
    let cases: Vec<_> = cases
        .iter()
        .filter(|case| {
            !matches!(
                case["name"].as_str().unwrap(),
                "gray_0.5" | "gray_0.500015258789" | "rgb_0.5" | "rgb_0.500015258789"
            )
        })
        .cloned()
        .collect();
    assert_eq!(cases.len(), 66);
    compare_independent_eps_cases_through_swap_and_gpu(&gpu, &cases);
}

#[test]
#[ignore = "requires actual GPU adapter; run explicitly with RRRAH_GPU_BACKEND=metal"]
fn independent_eps_arc_pixels_survive_swap_and_metal() {
    let gpu = common::qualification_gpu().expect("required GPU adapter");
    eprintln!("EPS arc lifecycle adapter: {}", gpu.adapter_name());
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/eps/arc-pixel-zero-adjust-ghostscript-reference.json"
    ))
    .unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    // Full ellipse gate remains open: pixel 19 differs even with zero fill adjust.
    let qualified: Vec<_> = cases
        .iter()
        .filter(|case| case["name"] != "ellipse")
        .cloned()
        .collect();
    assert_eq!(qualified.len(), 5);
    compare_independent_eps_cases_with_sampling(&gpu, &qualified, Some(1));
}

#[test]
#[ignore = "requires actual GPU adapter; run explicitly with RRRAH_GPU_BACKEND=metal"]
fn independent_eps_clip_pixels_survive_swap_and_metal() {
    let gpu = common::qualification_gpu().expect("required GPU adapter");
    eprintln!("EPS clipping adapter: {}", gpu.adapter_name());
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/eps/clip-pixel-ghostscript-reference.json"
    ))
    .unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 10);
    compare_independent_eps_cases_through_swap_and_gpu(&gpu, cases);
}

fn compare_independent_eps_cases_through_swap_and_gpu(
    gpu: &common::GpuReadback,
    cases: &[serde_json::Value],
) {
    compare_independent_eps_cases_with_sampling(gpu, cases, None);
}

fn compare_independent_eps_cases_with_sampling(
    gpu: &common::GpuReadback,
    cases: &[serde_json::Value],
    samples: Option<u8>,
) {
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("import.eps");
        let source = format!(
            "%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 8 8\n%%EndComments\n{}\n%%EOF\n",
            case["source"].as_str().unwrap()
        );
        std::fs::write(&path, source).unwrap();
        let root = MemoryBudget::new(32 * 1024 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(root.clone());
        let decode = || {
            if let Some(samples) = samples {
                let mut limits = EpsDocumentLimits::default();
                limits.raster.fill.samples = samples;
                return decode_eps_file_document(
                    &request,
                    1.,
                    EpsRasterColorPolicy::DeviceGrayRgbAsSrgb,
                    limits,
                    &root,
                )
                .unwrap();
            }
            let DecodedImage::Raster(frame) = decode_image(&request).unwrap() else {
                panic!()
            };
            frame
        };
        let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
            directory.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: 4096,
                    max_entries: Some(2),
                    ttl: None,
                },
                queue_bytes: 512,
                queue_count: 2,
                restore_bytes: 256,
            },
            MemoryBudget::new(512),
            root.clone(),
        )
        .unwrap();
        let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits {
            max_bytes: 256,
            max_entries: Some(1),
            ttl: None,
        });
        ram.enable_swap(swap);
        assert!(ram.insert(1, decode()));
        assert!(ram.insert(2, decode()));
        ram.swap().unwrap().wait_idle().unwrap();
        assert!(ram.get_lease(&1).is_none());
        let resident = root.used();
        let pressure = root.try_reserve(root.limit() - resident).unwrap();
        assert!(ram.swap().unwrap().try_get(&1, || false).is_err());
        assert_eq!(root.used(), root.limit(), "failed restore leaked credit");
        drop(pressure);
        assert_eq!(root.used(), resident);
        assert!(ram.swap().unwrap().try_get(&1, || true).unwrap().is_none());
        assert_eq!(root.used(), resident, "cancelled restore retained credit");
        let restored = ram.get_background_with_cancel(&1, || false).unwrap();
        ram.swap().unwrap().wait_idle().unwrap();
        assert!(ram.swap().unwrap().stats().reads >= 1);
        drop(ram);
        let rgb = case["rgb"].as_array().unwrap();
        let expected: Vec<u8> = rgb
            .chunks_exact(3)
            .flat_map(|p| {
                [
                    p[0].as_u64().unwrap() as u8,
                    p[1].as_u64().unwrap() as u8,
                    p[2].as_u64().unwrap() as u8,
                    255,
                ]
            })
            .collect();
        let RasterPixels::Rgba8(native) = restored.pixels() else {
            panic!()
        };
        assert_eq!(&native[..], &expected[..], "{}", case["name"]);
        let prepared = prepare_raster_for_display_with_budget(&restored, Some(&root)).unwrap();
        let golden = DecodedRaster::new(
            8,
            8,
            RasterPixels::Rgba8(std::sync::Arc::new(expected).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .to_linear_srgb()
        .unwrap();
        let view = rrrah_gpu::ViewParameters {
            viewport: [32.; 2],
            zoom: 4.,
            ..Default::default()
        };
        assert_eq!(
            gpu.render_raster(&prepared, view, [32, 32]).pixels,
            gpu.render_raster(&golden, view, [32, 32]).pixels,
            "{}",
            case["name"]
        );
        drop(prepared);
        drop(restored);
        assert_eq!(root.used(), 0);
    }
}

fn exercise_lifecycle() -> (DecodedRaster, MemoryBudget) {
    exercise_lifecycle_with_source(false)
}
#[test]
#[ignore = "requires actual Metal adapter"]
fn empty_eps_swap_replaces_colored_frame_without_stale_gpu_pixels() {
    let gpu = common::qualification_gpu().expect("required GPU adapter");
    eprintln!("Empty EPS transition adapter: {}", gpu.adapter_name());
    for code in ["newpath fill", "1 1 moveto 1 1 lineto stroke"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.eps");
        std::fs::write(
            &path,
            format!("%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 8 8\n%%EndComments\n{code}\n"),
        )
        .unwrap();
        let root = MemoryBudget::new(32 * 1024 * 1024);
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(root.clone());
        let DecodedImage::Raster(empty) = decode_image(&request).unwrap() else {
            panic!()
        };
        let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
            dir.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: 4096,
                    max_entries: Some(2),
                    ttl: None,
                },
                queue_bytes: 256,
                queue_count: 1,
                restore_bytes: 256,
            },
            MemoryBudget::new(256),
            root.clone(),
        )
        .unwrap();
        swap.enqueue(1, empty);
        swap.wait_idle().unwrap();
        assert_eq!(swap.stats().writes, 1);
        let empty = swap.try_get(&1, || false).unwrap().unwrap();
        let RasterPixels::Rgba8(pixels) = empty.pixels() else {
            panic!()
        };
        assert!(pixels.iter().all(|&value| value == 0));
        let prepared = prepare_raster_for_display_with_budget(&empty, Some(&root)).unwrap();
        let make = |pixel: [u8; 4]| {
            DecodedRaster::new(
                8,
                8,
                RasterPixels::Rgba8(std::sync::Arc::new(pixel.repeat(64)).into()),
                RasterColorSpace::Srgb,
            )
            .unwrap()
            .to_linear_srgb()
            .unwrap()
        };
        let colored = make([255, 0, 0, 255]);
        let golden = make([0, 0, 0, 0]);
        let view = rrrah_gpu::ViewParameters {
            viewport: [32.; 2],
            zoom: 4.,
            ..Default::default()
        };
        let (before, after) = gpu.render_raster_frame_transition(&colored, &prepared, view, [32, 32]);
        assert_ne!(before.pixels, after.pixels);
        assert_eq!(after.pixels, gpu.render_raster(&golden, view, [32, 32]).pixels);
        drop(prepared);
        drop(empty);
        drop(swap);
        assert_eq!(root.used(), 0);
    }
}

fn exercise_lifecycle_with_source(computed: bool) -> (DecodedRaster, MemoryBudget) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native.eps");
    let body = if computed {
        "1 0 0 setrgbcolor 4 4 -4 -4 rectfill 0 0 1 setrgbcolor 2 setlinewidth 0 5 2 idiv moveto 16 sqrt 2 lineto stroke"
    } else {
        "1 0 0 setrgbcolor 0 0 moveto 4 0 lineto 4 4 lineto 0 4 lineto closepath fill 0 0 1 setrgbcolor 2 setlinewidth 0 2 moveto 4 2 lineto stroke"
    };
    std::fs::write(
        &path,
        format!("%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 4 4\n%%EndComments\n{body}\n"),
    )
    .unwrap();
    let root = MemoryBudget::new(32 * 1024 * 1024);
    let decode = || {
        assert!(is_supported_image_path(&path));
        let mut request = DecodeRequest::new(&path);
        request.memory_budget = Some(root.clone());
        assert_eq!(image_source_kind(&request).unwrap(), ImageSourceKind::Raster);
        let DecodedImage::Raster(raster) = decode_image(&request).unwrap() else {
            panic!()
        };
        raster
    };
    std::fs::create_dir(directory.path().join("swap")).unwrap();
    let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
        &directory.path().join("swap"),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 8192,
                max_entries: Some(2),
                ttl: None,
            },
            queue_bytes: 128,
            queue_count: 2,
            restore_bytes: 64,
        },
        MemoryBudget::new(128),
        root.clone(),
    )
    .unwrap();
    let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits {
        max_bytes: 64,
        max_entries: Some(1),
        ttl: None,
    });
    ram.enable_swap(swap);
    assert!(ram.insert(1, decode()));
    assert!(ram.insert(2, decode()));
    ram.swap().unwrap().wait_idle().unwrap();
    assert!(ram.get_lease(&1).is_none());
    assert_eq!(root.used(), 64);
    let pressure = root.try_reserve(root.limit() - root.used()).unwrap();
    assert!(ram.swap().unwrap().try_get(&1, || false).is_err());
    drop(pressure);
    assert!(ram.swap().unwrap().try_get(&1, || true).unwrap().is_none());
    let restored = ram.get_background_with_cancel(&1, || false).unwrap();
    ram.swap().unwrap().wait_idle().unwrap();
    assert_eq!(restored.color_space(), &RasterColorSpace::Srgb);
    let RasterPixels::Rgba8(pixels) = restored.pixels() else {
        panic!()
    };
    for (at, pixel) in pixels.chunks_exact(4).enumerate() {
        assert_eq!(
            pixel,
            if (1..=2).contains(&(at / 4)) {
                &[0, 0, 255, 255]
            } else {
                &[255, 0, 0, 255]
            }
        );
    }
    assert!(ram.swap().unwrap().stats().writes >= 1);
    assert!(ram.swap().unwrap().stats().reads >= 1);
    drop(ram);
    assert_eq!(root.used(), 64);
    (restored, root)
}
#[test]
fn eps_file_ram_count_eviction_swap_pressure_cancel_and_restore() {
    let (raster, root) = exercise_lifecycle();
    drop(raster);
    assert_eq!(root.used(), 0);
}
#[test]
fn eps_computed_rectangles_survive_ram_swap_pressure_and_restore() {
    let (raster, root) = exercise_lifecycle_with_source(true);
    drop(raster);
    assert_eq!(root.used(), 0);
}
#[test]
fn eps_expired_visible_frame_spills_after_unpin_and_restores_exactly() {
    let (raster, root) = exercise_lifecycle();
    let held = raster.clone();
    let directory = tempfile::tempdir().unwrap();
    let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 8192,
                max_entries: Some(1),
                ttl: None,
            },
            queue_bytes: 64,
            queue_count: 1,
            restore_bytes: 64,
        },
        MemoryBudget::new(64),
        root.clone(),
    )
    .unwrap();
    let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits {
        max_bytes: 64,
        max_entries: Some(1),
        ttl: Some(std::time::Duration::ZERO),
    });
    ram.enable_swap(swap);
    assert!(ram.insert_visible(1, raster));
    assert!(
        ram.get_lease(&1).is_none(),
        "expired pixels must not be handed to new consumers"
    );
    assert_eq!(ram.spill_expired(), 0, "visible pin protects storage despite TTL");
    assert_eq!(root.used(), 64);
    ram.mark_visible(2);
    assert_eq!(ram.spill_expired(), 1);
    ram.swap().unwrap().wait_idle().unwrap();
    assert!(ram.is_empty());
    assert_eq!(ram.swap().unwrap().stats().writes, 1);
    assert_eq!(root.used(), 64, "external owner retains pixel credit after spill");
    drop(held);
    assert_eq!(root.used(), 0);
    assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits {
        max_bytes: 64,
        max_entries: Some(1),
        ttl: None
    }));
    let restored = ram.get_background_with_cancel(&1, || false).unwrap();
    let RasterPixels::Rgba8(pixels) = restored.pixels() else {
        panic!()
    };
    for (at, pixel) in pixels.chunks_exact(4).enumerate() {
        assert_eq!(
            pixel,
            if (1..=2).contains(&(at / 4)) {
                &[0, 0, 255, 255]
            } else {
                &[255, 0, 0, 255]
            }
        );
    }
    assert_eq!(root.used(), 64);
    drop(ram);
    drop(restored);
    assert_eq!(root.used(), 0);
}
#[test]
#[ignore = "requires actual GPU adapter; run explicitly with RRRAH_GPU_BACKEND=metal"]
fn eps_restored_raster_matches_analytic_reference_on_metal() {
    let gpu = common::qualification_gpu().expect("required GPU adapter");
    eprintln!("EPS lifecycle adapter: {}", gpu.adapter_name());
    for computed in [false, true] {
        let (raster, root) = exercise_lifecycle_with_source(computed);
        let prepared = prepare_raster_for_display_with_budget(&raster, Some(&root)).unwrap();
        let mut pixels = Vec::new();
        for at in 0..16 {
            pixels.extend(if (1..=2).contains(&(at / 4)) {
                [0, 0, 255, 255]
            } else {
                [255, 0, 0, 255]
            });
        }
        let golden = DecodedRaster::new(
            4,
            4,
            RasterPixels::Rgba8(std::sync::Arc::new(pixels).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .to_linear_srgb()
        .unwrap();
        let view = rrrah_gpu::ViewParameters {
            viewport: [32.; 2],
            zoom: 4.,
            ..rrrah_gpu::ViewParameters::default()
        };
        assert_eq!(
            gpu.render_raster(&prepared, view, [32, 32]).pixels,
            gpu.render_raster(&golden, view, [32, 32]).pixels
        );
        drop(prepared);
        drop(raster);
        assert_eq!(root.used(), 0);
    }
}
