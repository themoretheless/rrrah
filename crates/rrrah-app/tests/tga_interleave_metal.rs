#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
fn payload(frame: &DecodedRaster) -> Vec<u8> {
    match frame.pixels() {
        RasterPixels::Rgba8(p) => p.to_vec(),
        RasterPixels::Rgba32Float(p) => p.iter().flat_map(|v| v.to_bits().to_le_bytes()).collect(),
        _ => panic!("unexpected TGA sample type"),
    }
}
fn display(gpu: &common::GpuReadback, frame: &DecodedRaster, root: &MemoryBudget) -> (Vec<u8>, Vec<u16>) {
    let prepared = rrrah_decode::prepare_raster_for_display_with_budget(frame, Some(root)).unwrap();
    let view = rrrah_gpu::ViewParameters {
        viewport: [63., 65.],
        ..Default::default()
    };
    (
        gpu.render_raster(&prepared, view, [63, 65]).pixels,
        gpu.render_raster_half(&prepared, view, [63, 65]),
    )
}
#[test]
#[ignore = "manual actual Metal legacy TGA interleave/native/prepared/RAM/swap lifecycle"]
fn legacy_tga_rows_match_ffmpeg_and_survive_swap_and_metal() {
    let gpu = common::qualification_gpu().expect("required Metal adapter");
    eprintln!("TGA adapter: {}", gpu.adapter_name());
    for manifest in [
        include_str!("../../../tests/fixtures/raster/tga-interleave-reference.json"),
        include_str!("../../../tests/fixtures/raster/tga2-alpha-reference.json"),
        include_str!("../../../tests/fixtures/raster/tga2-aspect-reference.json"),
    ] {
        let reference: serde_json::Value = serde_json::from_str(manifest).unwrap();
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
        for row in reference["rows"].as_array().unwrap() {
            let name = row["name"].as_str().unwrap();
            let oracle = std::fs::read(fixtures.join(row["rgba8_file"].as_str().unwrap())).unwrap();
            let width = row["width"].as_u64().unwrap() as u32;
            let height = row["height"].as_u64().unwrap() as u32;
            let aspect = row["pixel_aspect"].as_f64().map(|v| v as f32);
            // Independent f64 sRGB transform of the declared fixture sample oracle.
            let linear = oracle
                .chunks_exact(4)
                .flat_map(|p| {
                    [0, 1, 2, 3].map(|c| {
                        let v = f64::from(p[c]) / 255.;
                        if c == 3 {
                            v as f32
                        } else {
                            (if v <= 0.04045 {
                                v / 12.92
                            } else {
                                ((v + 0.055) / 1.055).powf(2.4)
                            }) as f32
                        }
                    })
                })
                .collect::<Vec<_>>();
            let golden = DecodedRaster::new(
                width,
                height,
                RasterPixels::Rgba32Float(std::sync::Arc::new(linear).into()),
                RasterColorSpace::LinearSrgb,
            )
            .unwrap()
            .with_pixel_aspect(aspect)
            .unwrap();
            let view = rrrah_gpu::ViewParameters {
                viewport: [63., 65.],
                ..Default::default()
            };
            let expected_sdr = gpu.render_raster(&golden, view, [63, 65]).pixels;
            let expected_hdr = gpu.render_raster_half(&golden, view, [63, 65]);
            // Independent geometry reference uses ordinary square pixels and
            // different dimensions, so it cannot repeat the aspect interpretation.
            if row["rational"].is_array() && name.ends_with("origin0-raw.tga") {
                let (rw, rh) = if aspect == Some(2.) {
                    (width * 2, height)
                } else if aspect == Some(0.5) {
                    (width, height * 2)
                } else {
                    (width, height)
                };
                let solid = DecodedRaster::new(
                    width,
                    height,
                    RasterPixels::Rgba32Float(
                        std::sync::Arc::new(vec![1.; (width * height * 4) as usize]).into(),
                    ),
                    RasterColorSpace::LinearSrgb,
                )
                .unwrap()
                .with_pixel_aspect(aspect)
                .unwrap();
                let reference = DecodedRaster::new(
                    rw,
                    rh,
                    RasterPixels::Rgba32Float(std::sync::Arc::new(vec![1.; (rw * rh * 4) as usize]).into()),
                    RasterColorSpace::LinearSrgb,
                )
                .unwrap();
                assert_eq!(
                    gpu.render_raster(&solid, view, [63, 65]).pixels,
                    gpu.render_raster(&reference, view, [63, 65]).pixels,
                    "{name}: independent square-grid geometry"
                );
                assert_eq!(
                    gpu.render_raster_half(&solid, view, [63, 65]),
                    gpu.render_raster_half(&reference, view, [63, 65]),
                    "{name}: independent HDR square-grid geometry"
                );
            }
            if aspect.is_some_and(|v| v != 1.) {
                let square = golden.clone().with_pixel_aspect(Some(1.)).unwrap();
                assert_ne!(
                    expected_sdr,
                    gpu.render_raster(&square, view, [63, 65]).pixels,
                    "{name}: geometry must distinguish square pixels"
                );
                assert_ne!(
                    expected_hdr,
                    gpu.render_raster_half(&square, view, [63, 65]),
                    "{name}: HDR geometry must distinguish square pixels"
                );
            }
            for prepared in [false, true] {
                let root = MemoryBudget::new(1024 * 1024);
                let mut request = rrrah_decode::DecodeRequest::new(fixtures.join(name));
                request.memory_budget = Some(root.clone());
                let unqualified = rrrah_decode::decode_raster(&request).unwrap();
                assert!(rrrah_decode::prepare_raster_for_display(&unqualified).is_err());
                drop(unqualified);
                assert_eq!(root.used(), 0);
                request.assume_untagged_srgb = true;
                let native = rrrah_decode::decode_raster(&request).unwrap();
                assert_eq!(native.pixel_aspect(), aspect, "{name}");
                assert_eq!(payload(&native), oracle);
                let frame = if prepared {
                    let p =
                        rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&root)).unwrap();
                    drop(native);
                    p
                } else {
                    native
                };
                let bits = payload(&frame);
                let before = display(&gpu, &frame, &root);
                // Undefined-retain Type2 qualifies byte transport, not intended appearance.
                if row["alpha_type"].as_u64() != Some(2) {
                    assert!(
                        before
                            .0
                            .iter()
                            .zip(&expected_sdr)
                            .all(|(a, b)| a.abs_diff(*b) <= 1),
                        "{name}: SDR oracle"
                    );
                    assert!(
                        before
                            .1
                            .iter()
                            .zip(&expected_hdr)
                            .all(|(a, b)| a.abs_diff(*b) <= 1),
                        "{name}: HDR oracle"
                    );
                }
                let weight = frame.capacity_bytes();
                let directory = tempfile::tempdir().unwrap();
                let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
                    directory.path(),
                    rrrah_cache::ImageSwapConfig {
                        limits: rrrah_cache::CacheLimits::bytes(weight + 65536),
                        queue_bytes: weight,
                        queue_count: 1,
                        restore_bytes: weight,
                    },
                    MemoryBudget::new(weight),
                    root.clone(),
                )
                .unwrap();
                let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(weight));
                ram.enable_swap(swap);
                assert!(ram.insert_visible(1, frame));
                let lease = ram.get_lease(&1).unwrap();
                assert!(!ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
                drop(lease);
                ram.mark_visible(2);
                assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
                let swap = ram.swap().unwrap();
                swap.wait_idle().unwrap();
                assert_eq!(root.used(), 0);
                let pressure = root.try_reserve(root.limit()).unwrap();
                assert!(swap.try_get(&1, || false).is_err());
                drop(pressure);
                assert!(swap.try_get(&1, || root.used() > 0).unwrap().is_none());
                assert_eq!(root.used(), 0);
                let restored = swap.try_get(&1, || false).unwrap().unwrap();
                assert_eq!(restored.pixel_aspect(), aspect, "{name}: swap metadata");
                assert_eq!(payload(&restored), bits);
                assert_eq!(display(&gpu, &restored, &root), before);
                drop(restored);
                assert_eq!(root.used(), 0);
                assert_eq!(swap.stats().errors, 0);
                assert_eq!(swap.stats().reads, 1);
                assert_eq!(swap.stats().writes, 1);
            }
            eprintln!(
                "{name}: declared fixture samples; defined-alpha SDR<=1/HDR<=1ulp (type2 transport only); native+prepared RAM/lease/swap/pressure/cancel/retry exact; final=0"
            );
        }
    }
}
