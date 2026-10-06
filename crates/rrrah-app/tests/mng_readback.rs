#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;

fn assert_equal(a: &DecodedRaster, b: &DecodedRaster) {
    assert_eq!(
        (a.width(), a.height(), a.image_index(), a.image_count()),
        (b.width(), b.height(), b.image_index(), b.image_count())
    );
    assert_eq!(a.color_space(), b.color_space());
    match (a.pixels(), b.pixels()) {
        (RasterPixels::Rgba8(a), RasterPixels::Rgba8(b)) => assert_eq!(a, b),
        (RasterPixels::Rgba16(a), RasterPixels::Rgba16(b)) => assert_eq!(a, b),
        (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) => assert_eq!(a, b),
        _ => panic!("precision changed"),
    }
}

#[test]
fn mng_frames_preserve_native_and_prepared_samples_ram_swap_and_metal() {
    let gpu = common::qualification_gpu().expect("actual GPU required for MNG qualification");
    eprintln!("MNG cache adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mng");
    for (name, count, bits) in [
        ("rgb8-two-frames.mng", 2, 8),
        ("gray16-still.mng", 1, 16),
        ("rgb8-two-layer-still.mng", 1, 8),
        ("rgb8-global-srgb.mng", 2, 8),
        ("rgb8-partial-two-frames.mng", 2, 8),
        ("rgb16-partial-two-frames.mng", 2, 16),
        ("rgb8-partial-still.mng", 1, 8),
        ("rgb8-tall-partial-two-frames.mng", 2, 8),
    ] {
        for index in 0..count {
            let budget = rrrah_core::MemoryBudget::new(4096);
            let mut request = rrrah_decode::DecodeRequest::new(fixtures.join(name));
            request.memory_budget = Some(budget.clone());
            request.image_index = index;
            let decoded = rrrah_decode::decode_mng(&request).unwrap();
            assert_eq!(decoded.ticks_per_second, if count == 1 { 0 } else { 10 });
            let native = decoded.raster;
            let raw = std::fs::read(fixtures.join(format!("{name}-frame-{index}.rgba"))).unwrap();
            let pixels = if bits == 8 {
                RasterPixels::Rgba8(Arc::new(raw).into())
            } else {
                RasterPixels::Rgba16(
                    Arc::new(
                        raw.chunks_exact(2)
                            .map(|p| u16::from_le_bytes([p[0], p[1]]))
                            .collect::<Vec<_>>(),
                    )
                    .into(),
                )
            };
            let golden = DecodedRaster::new(3, 2, pixels, RasterColorSpace::Srgb)
                .unwrap()
                .with_image_selection(index, count)
                .unwrap();
            assert_equal(&native, &golden);
            let prepared =
                rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&budget)).unwrap();
            let golden = golden.to_linear_srgb().unwrap();
            assert_equal(&prepared, &golden);
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
            let key = (name.to_owned(), index);
            let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits {
                max_bytes: prepared.capacity_bytes(),
                max_entries: Some(1),
                ttl: None,
            });
            assert!(ram.insert(key.clone(), prepared.clone()));
            let lease = ram.get_lease(&key).unwrap();
            let (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) =
                (lease.pixels(), prepared.pixels())
            else {
                panic!("not prepared")
            };
            assert!(a.ptr_eq(b));
            assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
            drop(lease);
            drop(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).unwrap());
            drop(ram);
            for (variant, source) in [("native", native), ("prepared", prepared)] {
                let weight = source.capacity_bytes();
                let directory = tempfile::tempdir().unwrap();
                let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
                    directory.path(),
                    rrrah_cache::ImageSwapConfig {
                        limits: rrrah_cache::CacheLimits {
                            max_bytes: 4096,
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
                assert_equal(&restored, &source);
                let display =
                    rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
                assert_eq!(gpu.render_raster(&display, parameters, [96, 64]).pixels, expected);
                drop((display, restored, source));
                assert_eq!(
                    (swap.stats().writes, swap.stats().reads, swap.stats().errors),
                    (1, 1, 0)
                );
                eprintln!(
                    "{name} frame {index} {variant}: samples, selection/color, RAM leases, swap pressure/retry and Metal frame exact"
                );
            }
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
fn alpha_mng_linear_frames_survive_ram_swap_and_metal() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("alpha MNG adapter: {}", gpu.adapter_name());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mng");
    for (name, count) in [
        ("rgba8-alpha-animated.mng", 3),
        ("rgba16-alpha-animated.mng", 3),
        ("rgba8-alpha-still.mng", 1),
        ("rgb8-trns.mng", 1),
        ("gray8-trns.mng", 1),
        ("palette8-trns.mng", 1),
        ("grayalpha8.mng", 1),
        ("palette8-global.mng", 1),
        ("palette8-global-override.mng", 1),
        ("rgb16-trns.mng", 1),
        ("gray16-trns.mng", 1),
        ("grayalpha16.mng", 1),
        ("intrapixel-rgba8-alpha-animated.mng", 3),
        ("intrapixel-rgba16-alpha-animated.mng", 3),
        ("intrapixel-rgb8-trns.mng", 1),
        ("intrapixel-rgb16-trns.mng", 1),
        ("intrapixel-rgba8-filters.mng", 1),
        ("intrapixel-rgba16-filters.mng", 1),
        ("intrapixel-rgba8-adam7.mng", 1),
        ("intrapixel-rgba16-adam7.mng", 1),
    ] {
        for index in 0..count {
            let budget = rrrah_core::MemoryBudget::new(4096);
            let mut request = rrrah_decode::DecodeRequest::new(root.join(name));
            request.image_index = index;
            request.memory_budget = Some(budget.clone());
            let source = rrrah_decode::decode_mng(&request).unwrap().raster;
            let weight = source.capacity_bytes();
            let reference = std::fs::read(root.join(format!("{name}-frame-{index}.f32"))).unwrap();
            let values: Vec<f32> = reference
                .chunks_exact(4)
                .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                .collect();
            let RasterPixels::Rgba32Float(p) = source.pixels() else {
                panic!()
            };
            for (a, b) in p.iter().zip(&values) {
                assert!((a - b).abs() < 2e-7);
            }
            let golden = DecodedRaster::new(
                source.width(),
                source.height(),
                RasterPixels::Rgba32Float(Arc::new(values).into()),
                RasterColorSpace::LinearSrgb,
            )
            .unwrap();
            let parameters = rrrah_gpu::ViewParameters {
                viewport: [96., 64.],
                ..Default::default()
            };
            let expected = gpu.render_raster(&source, parameters, [96, 64]).pixels;
            let reference_frame = gpu.render_raster(&golden, parameters, [96, 64]).pixels;
            for (a, b) in expected.iter().zip(reference_frame) {
                assert!(a.abs_diff(b) <= 1);
            }
            let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(weight));
            assert!(ram.insert(index, source.clone()));
            let lease = ram.get_lease(&index).unwrap();
            assert_equal(&lease, &source);
            assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
            drop(lease);
            drop(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).unwrap());
            let directory = tempfile::tempdir().unwrap();
            let swap: rrrah_cache::RasterSwapCache<usize> = rrrah_cache::ImageSwapCache::new_with_budgets(
                directory.path(),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits {
                        max_bytes: 4096,
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
                swap.try_enqueue(index, source.clone()),
                rrrah_cache::SpillAdmission::Queued
            );
            swap.wait_idle().unwrap();
            let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
            assert!(swap.try_get(&index, || false).is_err());
            assert_eq!(swap.stats().errors, 0);
            drop(pressure);
            let restored = swap.try_get(&index, || false).unwrap().unwrap();
            assert_equal(&restored, &source);
            assert_eq!(
                gpu.render_raster(&restored, parameters, [96, 64]).pixels,
                expected
            );
            drop((restored, source, ram));
            assert_eq!(budget.used(), 0);
            eprintln!(
                "{name} frame {index}: independent float reference, RAM lease, swap pressure/retry, Metal verified"
            );
        }
    }
}
