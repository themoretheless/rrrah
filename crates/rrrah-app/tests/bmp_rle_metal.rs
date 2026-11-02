#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
use rrrah_gpu::ViewParameters;
use std::sync::Arc;

#[test]
#[ignore = "requires actual Metal for independent RLE4/RLE8 storage qualification"]
fn native_bmp_rle_preserves_independent_pixels_through_swap_and_metal() {
    let gpu = common::qualification_gpu().expect("required actual GPU");
    eprintln!("Native BMP RLE adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("bmp-rle-reference.json")).unwrap()).unwrap();
    let params = ViewParameters {
        viewport: [31., 29.],
        zoom: 1.,
        ..Default::default()
    };
    let mut lifecycles = 0;
    for case in manifest["cases"].as_array().unwrap() {
        let name = case["path"].as_str().unwrap();
        let hex = case["rgba8_hex"].as_str().unwrap();
        let rgba: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
            .collect();
        let width = case["dimensions"][0].as_u64().unwrap() as u32;
        let height = case["dimensions"][1].as_u64().unwrap() as u32;
        let linear: Vec<f32> = rgba
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let x = f64::from(v) / 255.;
                if i % 4 == 3 {
                    x as f32
                } else if x <= 0.04045 {
                    (x / 12.92) as f32
                } else {
                    ((x + 0.055) / 1.055).powf(2.4) as f32
                }
            })
            .collect();
        let reference = DecodedRaster::new(
            width,
            height,
            RasterPixels::Rgba32Float(Arc::new(linear).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let sdr = gpu.render_raster(&reference, params, [31, 29]).pixels;
        let hdr = gpu.render_raster_half(&reference, params, [31, 29]);
        for prepared in [false, true] {
            let root = MemoryBudget::new(1024 * 1024);
            let queue = MemoryBudget::new(65536);
            let mut request = rrrah_decode::DecodeRequest::new(fixtures.join(name));
            request.memory_budget = Some(root.clone());
            let native = rrrah_decode::decode_raster(&request).unwrap();
            let RasterPixels::Rgba8(p) = native.pixels() else {
                panic!()
            };
            assert_eq!(&p[..], &rgba);
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
                for (a, e) in actual.iter().zip(expected.iter()) {
                    assert!((a - e).abs() <= 2e-6);
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
                    assert!(a.abs_diff(*e) <= 1);
                }
            };
            check(&raster);
            let mut expected = Vec::new();
            rrrah_cache::write_raster_payload(&mut expected, &raster).unwrap();
            let directory = tempfile::tempdir().unwrap();
            let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
                directory.path(),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits::bytes(65536),
                    queue_bytes: 65536,
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
            let mut actual = Vec::new();
            rrrah_cache::write_raster_payload(&mut actual, &restored).unwrap();
            assert_eq!(actual, expected);
            check(&restored);
            ram.swap().unwrap().shutdown();
            drop((restored, ram));
            assert_eq!((root.used(), queue.used()), (0, 0));
            lifecycles += 1;
            eprintln!(
                "BMP RLE {name} prepared={prepared}: independent samples, exact swap, pressure/cancel/retry, SDR/HDR, final=0"
            );
        }
    }
    assert_eq!(lifecycles, 40);
    eprintln!("Native BMP RLE complete: sources=20 lifecycles=40 final=0");
}
