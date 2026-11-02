#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
use rrrah_gpu::ViewParameters;
use std::sync::Arc;

#[test]
#[ignore = "requires actual Metal for authored BMP V5 ICC and swap qualification"]
fn bmp_v5_icc_preserves_independent_linear_color_and_swap_metal_frames() {
    let gpu = common::qualification_gpu().expect("required actual GPU");
    eprintln!("BMP V5 ICC adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let oracle: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("bmp-v5-icc-reference.json")).unwrap()).unwrap();
    let expected_native: Vec<u8> = oracle["rgba8"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|row| row.as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u8))
        .collect();
    let expected_rgb: Vec<f32> = oracle["linear_rgb_f64"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();
    let expected_rgba: Vec<f32> = expected_rgb
        .chunks_exact(3)
        .zip(expected_native.chunks_exact(4))
        .flat_map(|(rgb, native)| [rgb[0], rgb[1], rgb[2], f32::from(native[3]) / 255.])
        .collect();
    let reference = DecodedRaster::new(
        2,
        2,
        RasterPixels::Rgba32Float(Arc::new(expected_rgba).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let params = ViewParameters {
        viewport: [31., 29.],
        zoom: 8.,
        ..Default::default()
    };
    let sdr = gpu.render_raster(&reference, params, [31, 29]).pixels;
    let hdr = gpu.render_raster_half(&reference, params, [31, 29]);
    let profile = std::fs::read(fixtures.join("animation-linear-float.icc")).unwrap();
    for origin in ["top", "bottom"] {
        for prepared in [false, true] {
            let root = MemoryBudget::new(1024 * 1024);
            let queue = MemoryBudget::new(16384);
            let mut request =
                rrrah_decode::DecodeRequest::new(fixtures.join(format!("bmp-v5-linear-icc-{origin}.bmp")));
            request.memory_budget = Some(root.clone());
            let native = rrrah_decode::decode_raster(&request).unwrap();
            assert_eq!(native.color_space(), &RasterColorSpace::Icc(profile.clone()));
            let RasterPixels::Rgba8(p) = native.pixels() else {
                panic!()
            };
            assert_eq!(&p[..], &expected_native);
            let raster = if prepared {
                rrrah_decode::prepare_raster_for_display_with_budget_and_cancel(&native, Some(&root), || {
                    false
                })
                .unwrap()
            } else {
                native.clone()
            };
            drop(native);
            let check = |frame: &DecodedRaster| {
                let display = rrrah_decode::prepare_raster_for_display_with_budget_and_cancel(
                    frame,
                    Some(&root),
                    || false,
                )
                .unwrap();
                let (RasterPixels::Rgba32Float(actual), RasterPixels::Rgba32Float(expected)) =
                    (display.pixels(), reference.pixels())
                else {
                    panic!()
                };
                for (&a, &e) in actual.iter().zip(expected.iter()) {
                    assert!((a - e).abs() <= 2e-4, "{origin} prepared={prepared}: {a} != {e}");
                }
                for (a, e) in gpu
                    .render_raster(&display, params, [31, 29])
                    .pixels
                    .iter()
                    .zip(&sdr)
                {
                    assert!(a.abs_diff(*e) <= 1);
                }
                for (a, e) in gpu
                    .render_raster_half(&display, params, [31, 29])
                    .iter()
                    .zip(&hdr)
                {
                    assert!(a.abs_diff(*e) <= 1, "half-float {a} != {e}");
                }
            };
            check(&raster);
            let mut expected_payload = Vec::new();
            rrrah_cache::write_raster_payload(&mut expected_payload, &raster).unwrap();
            let directory = tempfile::tempdir().unwrap();
            let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
                directory.path(),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits::bytes(65536),
                    queue_bytes: 16384,
                    queue_count: 1,
                    restore_bytes: root.limit(),
                },
                queue.clone(),
                root.clone(),
            )
            .unwrap();
            let limits = rrrah_cache::CacheLimits::bytes(raster.capacity_bytes());
            let mut ram = rrrah_cache::RasterRamCache::new(limits);
            ram.enable_swap(swap);
            assert!(ram.insert(1, raster));
            assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
            ram.swap().unwrap().wait_idle().unwrap();
            assert_eq!((root.used(), queue.used()), (0, 0));
            assert!(ram.set_limits(limits).unwrap().is_empty());
            let pressure = root.try_reserve(root.limit()).unwrap();
            assert!(ram.get_background_with_cancel(&1, || false).is_none());
            drop(pressure);
            assert!(ram.get_background_with_cancel(&1, || true).is_none());
            let restored = ram.get_background_with_cancel(&1, || false).unwrap();
            let mut actual_payload = Vec::new();
            rrrah_cache::write_raster_payload(&mut actual_payload, &restored).unwrap();
            assert_eq!(actual_payload, expected_payload);
            check(&restored);
            ram.swap().unwrap().shutdown();
            drop((restored, ram));
            assert_eq!((root.used(), queue.used()), (0, 0));
            eprintln!(
                "BMP V5 origin={origin} prepared={prepared}: LCMS color, exact native samples/payload, pressure/cancel/retry, Metal SDR/HDR, final=0"
            );
        }
    }
}
