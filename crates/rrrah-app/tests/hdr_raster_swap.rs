#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use rrrah_gpu::ViewParameters;
use std::sync::Arc;

fn frame(width: u32, pixels: Vec<f32>) -> DecodedRaster {
    DecodedRaster::new(
        width,
        1,
        RasterPixels::Rgba32Float(Arc::new(pixels).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap()
}
fn view() -> ViewParameters {
    ViewParameters {
        viewport: [64.0; 2],
        zoom: 2.0,
        ..ViewParameters::default()
    }
}

#[test]
fn exr_physical_pixels_survive_ram_spill_pressure_retry_and_metal() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("EXR physical swap adapter: {}", gpu.adapter_name());
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/raster/float-none-rec709-declared.exr");
    let bytes = std::fs::read(&fixture).unwrap();
    let attribute = b"pixelAspectRatio\0float\0\x04\0\0\0";
    let offset = bytes
        .windows(attribute.len())
        .position(|v| v == attribute)
        .unwrap()
        + attribute.len();
    let reference = rrrah_decode::prepare_raster_for_display(
        &rrrah_decode::decode_raster_file(&fixture).unwrap(),
    )
    .unwrap();
    let parameters = ViewParameters { zoom: 1., ..view() };
    let square = gpu.render_raster(&reference, parameters, [64, 64]);
    let window_attribute = b"displayWindow\0box2i\0\x10\0\0\0";
    let window_offset = bytes
        .windows(window_attribute.len())
        .position(|v| v == window_attribute)
        .unwrap()
        + window_attribute.len();
    let cases = [2f32, 0.5, 1.]
        .into_iter()
        .map(|aspect| (aspect, None, [0i32, 0], None))
        .chain(
            [
                [-1i32, -1, 3, 2],
                [1, 0, 2, 1],
                [-2, 1, 1, 3],
                [5, 5, 6, 6],
                [0, 0, 2, 1],
            ]
            .into_iter()
            .map(|bounds| (1., Some(bounds), [0, 0], None)),
        );
    let cases = cases.chain([
        (
            1.,
            Some([-3, -2, 2, 2]),
            [-2, -1],
            Some("float-shifted-window-negative.exr"),
        ),
        (
            1.,
            Some([3, 1, 5, 3]),
            [2, 1],
            Some("float-shifted-window-positive-crop.exr"),
        ),
        (
            1.,
            Some([-7, 9, -5, 10]),
            [-7, 9],
            Some("float-shifted-window-translated.exr"),
        ),
    ]);
    for (aspect, bounds, origin, shifted_fixture) in cases {
        let expected = if let Some(bounds) = bounds {
            let width = (bounds[2] - bounds[0] + 1) as u32;
            let height = (bounds[3] - bounds[1] + 1) as u32;
            let RasterPixels::Rgba32Float(source) = reference.pixels() else {
                panic!("EXR precision")
            };
            assert_eq!((reference.width(), reference.height()), (3, 2));
            let mut pixels = vec![0f32; (width * height * 4) as usize];
            for y in bounds[1]..=bounds[3] {
                for x in bounds[0]..=bounds[2] {
                    if (origin[0]..origin[0] + 3).contains(&x)
                        && (origin[1]..origin[1] + 2).contains(&y)
                    {
                        let target = (((y - bounds[1]) as u32 * width + (x - bounds[0]) as u32) * 4)
                            as usize;
                        let original = (((y - origin[1]) * 3 + x - origin[0]) * 4) as usize;
                        pixels[target..target + 4].copy_from_slice(&source[original..original + 4]);
                    }
                }
            }
            DecodedRaster::new(
                width,
                height,
                RasterPixels::Rgba32Float(Arc::new(pixels).into()),
                reference.color_space().clone(),
            )
            .unwrap()
            .with_pixel_aspect(Some(aspect))
            .unwrap()
        } else {
            reference.clone().with_pixel_aspect(Some(aspect)).unwrap()
        };
        let expected_sdr = gpu.render_raster(&expected, parameters, [64, 64]);
        let expected_hdr = gpu.render_raster_half(&expected, parameters, [64, 64]);
        if aspect != 1. {
            assert_ne!(
                expected_sdr.pixels, square.pixels,
                "geometry control must distinguish square pixels"
            );
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("physical.exr");
        let mut encoded = if let Some(name) = shifted_fixture {
            std::fs::read(fixture.with_file_name(name)).unwrap()
        } else {
            bytes.clone()
        };
        encoded[offset..offset + 4].copy_from_slice(&aspect.to_le_bytes());
        if let Some(bounds) = bounds {
            for (index, coordinate) in bounds.iter().enumerate() {
                encoded[window_offset + index * 4..window_offset + index * 4 + 4]
                    .copy_from_slice(&coordinate.to_le_bytes());
            }
        }
        std::fs::write(&path, encoded).unwrap();
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(path.clone());
        request.memory_budget = Some(root.clone());
        let decoded = rrrah_decode::decode_raster(&request).unwrap();
        let weight = decoded.capacity_bytes();
        assert_eq!(root.used(), weight);
        let queue = rrrah_core::MemoryBudget::new(weight);
        let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
            directory.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: 65536,
                    max_entries: Some(1),
                    ttl: None,
                },
                queue_bytes: weight,
                queue_count: 1,
                restore_bytes: weight,
            },
            queue.clone(),
            root.clone(),
        )
        .unwrap();
        let limits = rrrah_cache::CacheLimits {
            max_bytes: weight,
            max_entries: Some(1),
            ttl: None,
        };
        let mut ram = rrrah_cache::RasterRamCache::new(limits);
        ram.enable_swap(swap);
        assert!(ram.insert(1, decoded));
        let pressure = root.try_reserve(root.limit() - root.used()).unwrap();
        let resident = ram.get_background_with_cancel(&1, || false).unwrap();
        assert_eq!(resident.pixel_aspect(), Some(aspect));
        drop(resident);
        drop(pressure);
        assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
        ram.swap().unwrap().wait_idle().unwrap();
        assert!(ram.is_empty());
        assert_eq!((root.used(), queue.used()), (0, 0));
        std::fs::remove_file(path).unwrap();
        assert!(ram.set_limits(limits).unwrap().is_empty());
        let pressure = root.try_reserve(root.limit()).unwrap();
        assert!(ram.get_background_with_cancel(&1, || false).is_none());
        drop(pressure);
        assert!(ram.get_background_with_cancel(&1, || true).is_none());
        assert_eq!(root.used(), 0);
        let restored = ram.get_background_with_cancel(&1, || false).unwrap();
        assert_eq!(restored.pixel_aspect(), Some(aspect));
        assert_eq!(
            (restored.width(), restored.height()),
            (expected.width(), expected.height())
        );
        let (RasterPixels::Rgba32Float(actual), RasterPixels::Rgba32Float(golden)) =
            (restored.pixels(), expected.pixels())
        else {
            panic!("EXR precision");
        };
        assert_eq!(actual.len(), golden.len());
        assert!(
            actual
                .iter()
                .zip(golden.iter())
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        let prepared =
            rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&root)).unwrap();
        assert_eq!(
            gpu.render_raster(&prepared, parameters, [64, 64]).pixels,
            expected_sdr.pixels
        );
        assert_eq!(
            gpu.render_raster_half(&prepared, parameters, [64, 64]),
            expected_hdr
        );
        drop(prepared);
        drop(restored);
        assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
        ram.swap().unwrap().wait_idle().unwrap();
        let stats = ram.swap().unwrap().stats();
        assert_eq!(
            (stats.writes, stats.reads, stats.errors, stats.queued_bytes),
            (1, 1, 0, 0)
        );
        assert_eq!((root.used(), queue.used()), (0, 0));
        eprintln!(
            "EXR geometry swap: aspect={aspect}; bounds={bounds:?}; origin={origin:?}; source deleted; float bits and SDR/HDR exact; one write/read; root=0; queue=0"
        );
    }
}

#[test]
fn heif_physical_pixels_survive_ram_spill_pressure_retry_and_metal() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("HEIF physical swap adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/raster");
    for (original, physical) in [
        ("alpha.heic", "heif-physical-alpha.heic"),
        ("rotate90.heic", "heif-physical-rotate90.heic"),
        ("mirror.heif", "heif-physical-mirror.heif"),
    ] {
        let reference = rrrah_decode::prepare_raster_for_display(
            &rrrah_decode::decode_raster_file(&fixtures.join(original)).unwrap(),
        ).unwrap();
        let aspect = 2f32;
        let parameters = ViewParameters { zoom: 1., ..view() };
        let square = gpu.render_raster(&reference, parameters, [64, 64]);
        let expected = reference.clone().with_pixel_aspect(Some(aspect)).unwrap();
        let expected_sdr = gpu.render_raster(&expected, parameters, [64, 64]);
        let expected_hdr = gpu.render_raster_half(&expected, parameters, [64, 64]);
        if aspect != 1. {
            assert_ne!(
                expected_sdr.pixels, square.pixels,
                "geometry control must distinguish square pixels"
            );
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(physical);
        std::fs::copy(fixtures.join(physical), &path).unwrap();
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(path.clone());
        request.memory_budget = Some(root.clone());
        let raw = rrrah_decode::decode_raster(&request).unwrap();
        let decoded = rrrah_decode::prepare_raster_for_display_with_budget(&raw, Some(&root)).unwrap();
        drop(raw);
        let weight = decoded.capacity_bytes();
        assert_eq!(root.used(), weight);
        let queue = rrrah_core::MemoryBudget::new(weight);
        let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
            directory.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: 65536,
                    max_entries: Some(1),
                    ttl: None,
                },
                queue_bytes: weight,
                queue_count: 1,
                restore_bytes: weight,
            },
            queue.clone(),
            root.clone(),
        )
        .unwrap();
        let limits = rrrah_cache::CacheLimits {
            max_bytes: weight,
            max_entries: Some(1),
            ttl: None,
        };
        let mut ram = rrrah_cache::RasterRamCache::new(limits);
        ram.enable_swap(swap);
        assert!(ram.insert(1, decoded));
        let pressure = root.try_reserve(root.limit() - root.used()).unwrap();
        let resident = ram.get_background_with_cancel(&1, || false).unwrap();
        assert_eq!(resident.pixel_aspect(), Some(aspect));
        drop(resident);
        drop(pressure);
        assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
        ram.swap().unwrap().wait_idle().unwrap();
        assert!(ram.is_empty());
        assert_eq!((root.used(), queue.used()), (0, 0));
        std::fs::remove_file(path).unwrap();
        assert!(ram.set_limits(limits).unwrap().is_empty());
        let pressure = root.try_reserve(root.limit()).unwrap();
        assert!(ram.get_background_with_cancel(&1, || false).is_none());
        drop(pressure);
        assert!(ram.get_background_with_cancel(&1, || true).is_none());
        assert_eq!(root.used(), 0);
        let restored = ram.get_background_with_cancel(&1, || false).unwrap();
        assert_eq!(restored.pixel_aspect(), Some(aspect));
        let (RasterPixels::Rgba32Float(actual), RasterPixels::Rgba32Float(golden)) =
            (restored.pixels(), expected.pixels())
        else {
            panic!("prepared HEIF precision");
        };
        assert_eq!(actual.len(), golden.len());
        assert!(
            actual
                .iter()
                .zip(golden.iter())
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&root)).unwrap();
        assert_eq!(
            gpu.render_raster(&prepared, parameters, [64, 64]).pixels,
            expected_sdr.pixels
        );
        assert_eq!(
            gpu.render_raster_half(&prepared, parameters, [64, 64]),
            expected_hdr
        );
        drop(prepared);
        drop(restored);
        assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
        ram.swap().unwrap().wait_idle().unwrap();
        let stats = ram.swap().unwrap().stats();
        assert_eq!(
            (stats.writes, stats.reads, stats.errors, stats.queued_bytes),
            (1, 1, 0, 0)
        );
        assert_eq!((root.used(), queue.used()), (0, 0));
        eprintln!(
            "HEIF physical swap: aspect={aspect}; source deleted; float bits and SDR/HDR exact; one write/read; root=0; queue=0"
        );
    }
}

#[test]
fn profiled_png_and_archives_keep_color_through_swap_and_metal() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("ICC raster swap adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    for name in [
        "pattern.profiled.png",
        "pattern.profiled.tif",
        "oriented.profiled.tif",
        "pattern.profiled.png.ora",
        "pattern.profiled.png.kra",
        "../xcf/profiled-normal-overlay-v1.xcf",
    ] {
        let budget = rrrah_core::MemoryBudget::new(4 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(fixtures.join(name));
        request.memory_budget = Some(budget.clone());
        let decoded = rrrah_decode::decode_raster(&request).unwrap();
        let RasterColorSpace::Icc(profile) = decoded.color_space() else {
            panic!("{name}: ICC absent")
        };
        assert!(!profile.is_empty());
        let expected_profile = profile.clone();
        let parameters = ViewParameters { zoom: 1.0, ..view() };
        let source_weight = decoded.capacity_bytes();
        assert!(source_weight > decoded.pixel_capacity_bytes());
        assert_eq!(budget.used(), source_weight);
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&decoded, Some(&budget)).unwrap();
        let actual = gpu.render_raster(&prepared, parameters, [64, 64]);
        let (width, height, rgba) = if name == "oriented.profiled.tif" {
            (1, 2, vec![255, 0, 0, 255, 0, 255, 0, 128])
        } else if name == "../xcf/profiled-normal-overlay-v1.xcf" {
            (2, 1, vec![127, 128, 0, 255, 0, 0, 255, 255])
        } else {
            let golden_name = if name == "pattern.profiled.tif" {
                "pattern.profiled.png.rgba".to_owned()
            } else {
                format!("{name}.rgba")
            };
            (16, 16, std::fs::read(fixtures.join(golden_name)).unwrap())
        };
        let golden = DecodedRaster::new(
            width,
            height,
            RasterPixels::Rgba8(Arc::new(rgba).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .to_linear_srgb()
        .unwrap();
        let expected = gpu.render_raster(&golden, parameters, [64, 64]);
        assert!(
            actual
                .pixels
                .iter()
                .zip(&expected.pixels)
                .all(|(a, b)| a.abs_diff(*b) <= 1),
            "{name}: ICC reference differs"
        );
        let directory = tempfile::tempdir().unwrap();
        let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
            directory.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: 8192,
                    max_entries: Some(1),
                    ttl: None,
                },
                queue_bytes: source_weight,
                queue_count: 1,
                restore_bytes: source_weight,
            },
            rrrah_core::MemoryBudget::new(source_weight),
            budget.clone(),
        )
        .unwrap();
        swap.enqueue(1, decoded);
        swap.wait_idle().unwrap();
        assert_eq!(budget.used(), prepared.capacity_bytes());
        drop(prepared);
        assert_eq!(budget.used(), 0);
        let pressure = budget.try_reserve(budget.limit()).unwrap();
        assert!(swap.try_get(&1, || false).is_err());
        assert_eq!(swap.stats().errors, 0);
        drop(pressure);
        let restored = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(restored.color_space(), &RasterColorSpace::Icc(expected_profile));
        assert_eq!(budget.used(), source_weight);
        let prepared =
            rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
        assert_eq!(
            gpu.render_raster(&prepared, parameters, [64, 64]).pixels,
            actual.pixels,
            "{name}: restored color changed"
        );
        drop(restored);
        assert_eq!(budget.used(), prepared.capacity_bytes());
        drop(prepared);
        assert_eq!(budget.used(), 0);
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(swap.stats().reads, 1);
        assert_eq!(swap.stats().queued_bytes, 0);
    }
}

#[test]
fn masked_legacy_xcf_preserves_explicit_color_assumption_through_swap_and_metal() {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("Masked XCF adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/xcf");
    for (name, expected) in [
        ("normal-mask-enabled-v1.xcf", [255, 0, 0, 255, 0, 128, 127, 255]),
        (
            "normal-mask-disabled-v1.xcf",
            [127, 128, 0, 255, 0, 128, 127, 255],
        ),
        (
            "normal-mask-negative-offset-v1.xcf",
            [127, 128, 0, 255, 0, 0, 255, 255],
        ),
        ("normal-mask-hidden-v1.xcf", [255, 0, 0, 255, 0, 0, 255, 255]),
        (
            "normal-mask-zero-opacity-v1.xcf",
            [255, 0, 0, 255, 0, 0, 255, 255],
        ),
        (
            "normal-mask-half-opacity-v1.xcf",
            [255, 0, 0, 255, 0, 64, 191, 255],
        ),
        ("normal-gray-v1.xcf", [17, 17, 17, 255, 239, 239, 239, 255]),
        ("normal-indexed-v1.xcf", [17, 43, 91, 255, 239, 181, 7, 255]),
        ("normal-indexed-alpha-v1.xcf", [17, 43, 91, 255, 239, 181, 7, 255]),
        (
            "normal-indexed-alpha-boundary-v1.xcf",
            [0, 0, 0, 0, 239, 181, 7, 255],
        ),
        ("normal-gray-alpha-v1.xcf", [17, 17, 17, 128, 239, 239, 239, 255]),
    ] {
        let budget = rrrah_core::MemoryBudget::new(4096);
        let mut request = rrrah_decode::DecodeRequest::new(fixtures.join(name));
        request.memory_budget = Some(budget.clone());
        let strict = rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!(strict.color_space(), &RasterColorSpace::Unspecified);
        assert!(rrrah_decode::prepare_raster_for_display(&strict).is_err());
        drop(strict);
        assert_eq!(budget.used(), 0);
        request.assume_untagged_srgb = true;
        let raster = rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!(raster.color_space(), &RasterColorSpace::AssumedSrgb);
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&raster, Some(&budget)).unwrap();
        let parameters = ViewParameters { zoom: 1.0, ..view() };
        let actual = gpu.render_raster(&prepared, parameters, [64, 64]);
        let golden = DecodedRaster::new(
            2,
            1,
            RasterPixels::Rgba8(Arc::new(expected.to_vec()).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .to_linear_srgb()
        .unwrap();
        assert_eq!(
            actual.pixels,
            gpu.render_raster(&golden, parameters, [64, 64]).pixels
        );
        let directory = tempfile::tempdir().unwrap();
        let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
            directory.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits {
                    max_bytes: 8192,
                    max_entries: Some(1),
                    ttl: None,
                },
                queue_bytes: 8,
                queue_count: 1,
                restore_bytes: 8,
            },
            rrrah_core::MemoryBudget::new(8),
            budget.clone(),
        )
        .unwrap();
        swap.enqueue(1, raster);
        swap.wait_idle().unwrap();
        drop(prepared);
        assert_eq!(budget.used(), 0);
        let restored = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(restored.color_space(), &RasterColorSpace::AssumedSrgb);
        let RasterPixels::Rgba8(pixels) = restored.pixels() else {
            panic!("wrong precision")
        };
        assert_eq!(&**pixels, &expected);
        let prepared =
            rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
        assert_eq!(
            actual.pixels,
            gpu.render_raster(&prepared, parameters, [64, 64]).pixels
        );
        drop(restored);
        drop(prepared);
        assert_eq!(budget.used(), 0);
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(swap.stats().reads, 1);
        assert_eq!(swap.stats().queued_bytes, 0);
    }
}

#[test]
fn corrupt_raster_swap_entries_release_memory_and_allow_replacement() {
    for kind in 0..3 {
        for truncate in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let budget = rrrah_core::MemoryBudget::new(128);
            let source = || {
                let pixels = match kind {
                    0 => RasterPixels::Rgba8(Arc::new(vec![255, 128, 0, 255]).into()),
                    1 => RasterPixels::Rgba16(Arc::new(vec![65535, 12345, 0, 65535]).into()),
                    _ => RasterPixels::Rgba32Float(
                        Arc::new(vec![16.0, -0.0, f32::from_bits(0x7fc01234), 1.0]).into(),
                    ),
                };
                DecodedRaster::new(1, 1, pixels, RasterColorSpace::LinearSrgb)
                    .unwrap()
                    .with_image_selection(2, 3)
                    .unwrap()
                    .try_manage_pixels(&budget)
                    .unwrap()
            };
            let mut expected = Vec::new();
            rrrah_cache::write_raster_payload(&mut expected, &source()).unwrap();
            assert_eq!(budget.used(), 0);
            let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
                directory.path(),
                rrrah_cache::ImageSwapConfig {
                    limits: rrrah_cache::CacheLimits {
                        max_bytes: 4096,
                        max_entries: Some(1),
                        ttl: None,
                    },
                    queue_bytes: 128,
                    queue_count: 1,
                    restore_bytes: 128,
                },
                rrrah_core::MemoryBudget::new(128),
                budget.clone(),
            )
            .unwrap();
            swap.enqueue(1, source());
            swap.wait_idle().unwrap();
            assert_eq!(budget.used(), 0);
            let session = std::fs::read_dir(directory.path())
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let blob = std::fs::read_dir(session)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let mut bytes = std::fs::read(&blob).unwrap();
            if truncate {
                bytes.pop();
            } else {
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
            }
            std::fs::write(&blob, bytes).unwrap();
            assert!(swap.try_get(&1, || false).unwrap().is_none());
            assert_eq!(budget.used(), 0);
            assert_eq!(swap.stats().errors, 1);
            assert!(!blob.exists());
            assert!(swap.try_get(&1, || false).unwrap().is_none());
            assert_eq!(swap.stats().errors, 1);
            swap.enqueue(1, source());
            swap.wait_idle().unwrap();
            let restored = swap.try_get(&1, || false).unwrap().unwrap();
            let mut actual = Vec::new();
            rrrah_cache::write_raster_payload(&mut actual, &restored).unwrap();
            assert_eq!(actual, expected, "kind {kind}, truncated {truncate}");
            drop(restored);
            assert_eq!(budget.used(), 0);
            assert_eq!(swap.stats().queued_bytes, 0);
        }
    }
}

#[test]
fn hdr_raster_swap_preserves_samples_and_linear_metal_output() {
    qualify_linear_hdr_swap(false);
}

#[test]
fn float_rla_swap_preserves_hdr_samples_and_linear_metal_output() {
    qualify_linear_hdr_swap(true);
}

fn qualify_linear_hdr_swap(float_rla: bool) {
    let weight = if float_rla { 32 } else { 16 };
    let instance = common::headless_instance();
    let adapter = match pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    )) {
        Ok(adapter) => adapter,
        Err(error) if std::env::var("RRRAH_GPU_OPTIONAL").as_deref() == Ok("1") => {
            eprintln!("HDR raster swap: GPU unavailable, optional skip: {error}");
            return;
        }
        Err(error) => panic!("HDR raster swap requires a GPU: {error}"),
    };
    eprintln!("HDR raster adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let gpu_budget = rrrah_core::MemoryBudget::new(weight);
    let mut renderer = rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&device, gpu_budget.clone());
    let budget = rrrah_core::MemoryBudget::new(if float_rla { 4096 } else { weight });
    let source = if float_rla {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster/rla-float32-le-hdr-oiio.rla");
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = Some(budget.clone());
        rrrah_decode::decode_rla_float_with_interpretation(
            &request,
            rrrah_decode::RlaFloatByteOrder::Little,
            rrrah_decode::RlaAlphaMode::Straight,
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
    } else {
        frame(1, vec![4.0, 2.0, -0.5, 1.0])
            .try_manage_pixels(&budget)
            .unwrap()
    };
    assert_eq!(budget.used(), weight);
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
    let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(weight));
    ram.enable_swap(swap);
    assert!(ram.insert_visible(1, source));
    let lease = ram.get_lease(&1).unwrap();
    assert!(!ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
    drop(lease);
    // Move the visible owner away so the old HDR frame becomes spillable.
    ram.mark_visible(2);
    assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
    assert!(ram.is_empty());
    let swap = ram.swap().unwrap();
    swap.wait_idle().unwrap();
    assert_eq!(budget.used(), 0);
    let pressure = budget.try_reserve(budget.limit()).unwrap();
    assert!(swap.try_get(&1, || false).is_err());
    assert_eq!(swap.stats().errors, 0);
    drop(pressure);
    let source = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!(source.color_space(), &RasterColorSpace::LinearSrgb);
    let RasterPixels::Rgba32Float(samples) = source.pixels() else {
        panic!("HDR sample type changed")
    };
    let expected = if float_rla {
        vec![4.0, 2.0, -0.5, 1.0, 4.0, 2.0, -0.5, 1.0]
    } else {
        vec![4.0, 2.0, -0.5, 1.0]
    };
    assert_eq!(
        samples.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected.iter().map(|v: &f32| v.to_bits()).collect::<Vec<_>>()
    );
    assert!(samples.is_managed());
    assert_eq!(budget.used(), weight);
    assert_eq!(swap.stats().writes, 1);
    assert_eq!(swap.stats().reads, 1);
    assert_eq!(swap.stats().errors, 0);
    assert_eq!(swap.stats().queued_bytes, 0);
    renderer.upload(&device, &queue, &source).unwrap();
    // Upload owns its GPU copy: CPU reservation can be released before drawing.
    drop(source);
    assert_eq!(budget.used(), 0);
    let mut parameters = view();
    // Fit leaves a 32-pixel viewport margin; the 2:1 producer fixture needs
    // zoom four to cover the entire square target, including its top row.
    if float_rla {
        parameters.zoom = 4.0;
    }
    parameters.exposure_stops = 1.0;
    renderer.update_view(&queue, parameters);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("linear HDR readback target"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 512 * 64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.encode(&mut encoder, &target.create_view(&Default::default()));
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(512),
                rows_per_image: Some(64),
            },
        },
        wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
    );
    let submission = queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    let mapped = readback.slice(..).get_mapped_range().unwrap();
    // Exact IEEE binary16 encodings: 8, 4, -1, opaque alpha.
    for (index, pixel) in mapped.chunks_exact(8).enumerate() {
        let channels: Vec<u16> = pixel
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect();
        assert_eq!(channels, [0x4800, 0x4400, 0xbc00, 0x3c00], "HDR pixel {index}");
    }
    drop(mapped);
    readback.unmap();
    assert_eq!(gpu_budget.used(), weight);
    drop(renderer);
    assert_eq!(gpu_budget.used(), 0);
}

#[test]
fn pict_public_route_lease_swap_pressure_and_metal_preserve_rgba() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pict/red-rectangle-v2.pict");
    qualify_pict_transport(path, 4096, None);
}

#[test]
#[ignore = "requires actual GPU and external PICT/TwelveMonkeys corpus"]
fn pict_external_directbits_oracle_swap_pressure_and_metal() {
    let root = std::env::var_os("RRRAH_PICT_CORPUS").expect("RRRAH_PICT_CORPUS required");
    for name in ["mire16.pict", "mire32.pict", "16bit.pict", "32bit.pict"] {
        let root = std::path::Path::new(&root);
        let oracle = std::fs::read(root.join(format!("{name}.rgba"))).unwrap();
        qualify_pict_transport(root.join(name), 4 * 1024 * 1024, Some(&oracle));
    }
}

#[test]
fn pict_unpacked_xrgb_macos_pixels_survive_swap_pressure_and_metal() {
    // Recorded macOS sips RGBA; qualifier pins the fixture hash and manifest.
    let oracle = [255, 0, 0, 255, 17, 95, 203, 255];
    for name in ["unpacked-xrgb.pict", "drop-pad-rgb.pict"] {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/pict")
            .join(name);
        qualify_pict_transport(path, 4096, Some(&oracle));
    }
}

fn render_hdr_float_metal(source: &DecodedRaster) -> Vec<u16> {
    render_hdr_float_metal_sized(source, [128, 64], 2.0)
}

fn render_hdr_float_metal_sized(source: &DecodedRaster, output: [u32; 2], zoom: f32) -> Vec<u16> {
    let row_bytes = (output[0] * 8).div_ceil(256) * 256;
    let instance = common::headless_instance();
    let adapter = pollster::block_on(common::request_adapter(
        &instance,
        &wgpu::RequestAdapterOptions::default(),
    ))
    .unwrap();
    eprintln!("HDR float adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let gpu_budget = rrrah_core::MemoryBudget::new(source.pixel_capacity_bytes());
    let mut renderer = rrrah_gpu::RasterRenderer::new_linear_hdr_with_budget(&device, gpu_budget.clone());
    renderer.upload(&device, &queue, source).unwrap();
    let parameters = ViewParameters {
        viewport: [output[0] as f32, output[1] as f32],
        zoom,
        exposure_stops: 1.0,
        ..view()
    };
    renderer.update_view(&queue, parameters);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("linear HDR readback target"),
        size: wgpu::Extent3d {
            width: output[0],
            height: output[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row_bytes) * u64::from(output[1]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.encode(&mut encoder, &target.create_view(&Default::default()));
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(output[1]),
            },
        },
        wgpu::Extent3d {
            width: output[0],
            height: output[1],
            depth_or_array_layers: 1,
        },
    );
    let submission = queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    let mapped = readback.slice(..).get_mapped_range().unwrap();
    let result: Vec<u16> = mapped
        .chunks_exact(row_bytes as usize)
        .flat_map(|row| {
            row[..output[0] as usize * 8]
                .chunks_exact(2)
                .map(|v| u16::from_le_bytes([v[0], v[1]]))
        })
        .collect();
    drop(mapped);
    readback.unmap();
    drop(renderer);
    assert_eq!(gpu_budget.used(), 0);
    result
}

#[test]
fn pq_avif_linear_pixels_survive_pressure_swap_and_metal() {
    for name in ["avif-pq10-svt.avif", "avif-pq10-color-svt.avif"] {
        qualify_hdr_still_transport(name);
    }
}

#[test]
fn hlg_avif_linear_pixels_survive_pressure_swap_and_metal() {
    for name in ["avif-hlg10-svt.avif", "avif-hlg10-color-svt.avif"] {
        qualify_hdr_still_transport(name);
    }
}

#[test]
fn hdr_hevc_linear_pixels_survive_pressure_swap_and_metal() {
    for name in [
        "heif-pq12.heic",
        "heif-hlg12.heic",
        "heif-pq12-color.heic",
        "heif-hlg12-color.heic",
        "heif-pq12-color-alpha.heic",
        "heif-hlg12-color-alpha.heic",
    ] {
        qualify_hdr_still_transport(name);
    }
}

#[test]
#[ignore = "requires external Canon DPP 6000x4000 HDR PQ HIF and FFmpeg RGBA16 oracle"]
fn photographic_heif_pq_full_resolution_swap_and_metal() {
    let source_path = std::env::var_os("RRRAH_HEIF_CAMERA_SOURCE").expect("external source required");
    let oracle_path =
        std::env::var_os("RRRAH_HEIF_CAMERA_ORACLE").expect("independent FFmpeg oracle required");
    let budget = rrrah_core::MemoryBudget::new(1024 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(std::path::PathBuf::from(source_path));
    request.memory_budget = Some(budget.clone());
    let native_started = std::time::Instant::now();
    let native = rrrah_decode::decode_raster(&request).unwrap();
    eprintln!(
        "photographic HEIF native decode ms={:.3}",
        native_started.elapsed().as_secs_f64() * 1000.
    );
    assert_eq!((native.width(), native.height()), (6000, 4000));
    assert_eq!(
        native.color_space(),
        &RasterColorSpace::Cicp {
            primaries: 9,
            transfer: 16,
            matrix: 9,
            full_range: true,
        }
    );
    let RasterPixels::Rgba16(native_pixels) = native.pixels() else {
        panic!("10-bit source precision lost")
    };
    let oracle = std::fs::read(oracle_path).unwrap();
    assert_eq!(oracle.len(), native_pixels.len() * 2);
    let mut max_difference = 0_u16;
    let mut sum_difference = 0_u64;
    let mut above_256 = 0_u64;
    for (index, (actual, bytes)) in native_pixels.iter().zip(oracle.chunks_exact(2)).enumerate() {
        let expected = u16::from_le_bytes(bytes.try_into().unwrap());
        if index % 4 == 3 {
            assert_eq!((*actual, expected), (65535, 65535));
        } else {
            let difference = actual.abs_diff(expected);
            max_difference = max_difference.max(difference);
            sum_difference += u64::from(difference);
            above_256 += u64::from(difference > 256);
        }
    }
    eprintln!(
        "photographic HEIF independent RGB comparison: max_u16={max_difference}; mean_u16={:.6}; components_above_256={above_256}; RGB_components=72000000; alpha exact",
        sum_difference as f64 / 72_000_000.
    );
    // The two decoders use different u16 expansion/color-conversion rounding.
    // Bound this pinned photographic case to two 10-bit source code steps;
    // do not present it as byte-identical independent RGB reconstruction.
    assert!(max_difference <= 128);
    assert!(sum_difference as f64 / 72_000_000. <= 40.);
    drop(oracle);
    drop(native);
    assert_eq!(budget.used(), 0);
    request.pq_reference_white_nits = Some(100.);
    let linear_started = std::time::Instant::now();
    let source = rrrah_decode::decode_raster(&request).unwrap();
    eprintln!(
        "photographic HEIF decode plus PQ conversion ms={:.3}",
        linear_started.elapsed().as_secs_f64() * 1000.
    );
    assert_eq!(source.color_space(), &RasterColorSpace::LinearSrgb);
    let digest = |raster: &DecodedRaster| {
        let RasterPixels::Rgba32Float(pixels) = raster.pixels() else {
            panic!()
        };
        let mut hash = rrrah_cache::ObjectPayloadHasher::new();
        hash.update(bytemuck::cast_slice(pixels.as_slice()));
        hash.finalize()
    };
    let expected = digest(&source);
    let gpu = common::qualification_gpu().expect("actual Metal GPU required");
    eprintln!("photographic HEIF adapter: {}", gpu.adapter_name());
    let parameters = ViewParameters {
        viewport: [128., 96.],
        ..Default::default()
    };
    let sdr = gpu.render_raster(&source, parameters, [128, 96]).pixels;
    let full_hdr = std::env::var("RRRAH_HEIF_FULL_HDR_READBACK").as_deref() == Ok("1");
    let hdr_size = if full_hdr {
        [source.width(), source.height()]
    } else {
        [128, 64]
    };
    let hdr_zoom = if full_hdr { 1.0 } else { 2.0 };
    let hdr = render_hdr_float_metal_sized(&source, hdr_size, hdr_zoom);
    assert_eq!(hdr.len(), hdr_size[0] as usize * hdr_size[1] as usize * 4);
    assert!(
        hdr.iter().all(|v| v & 0x7c00 != 0x7c00),
        "finite HDR attachment required"
    );
    assert!(
        hdr.chunks_exact(4).all(|p| p[3] == 0x3c00),
        "opaque HDR attachment required"
    );
    assert!(
        hdr.chunks_exact(4).any(|p| p[..3] != hdr[..3]),
        "nonconstant photographic HDR required"
    );
    eprintln!(
        "photographic HEIF HDR readback: {}x{}; {} binary16 components; finite, opaque, nonconstant",
        hdr_size[0],
        hdr_size[1],
        hdr.len()
    );
    assert!(sdr.chunks_exact(4).any(|p| p != &sdr[..4]));
    let weight = source.capacity_bytes();
    assert_eq!(weight, 384_000_000);
    let directory = tempfile::tempdir().unwrap();
    let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits::bytes(1024 * 1024 * 1024),
            queue_bytes: weight,
            queue_count: 1,
            restore_bytes: weight,
        },
        rrrah_core::MemoryBudget::new(weight),
        budget.clone(),
    )
    .unwrap();
    let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(weight));
    ram.enable_swap(swap);
    assert!(ram.insert_visible(1, source));
    let lease = ram.get_lease(&1).unwrap();
    assert!(!ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
    drop(lease);
    ram.mark_visible(2);
    assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
    let swap = ram.swap().unwrap();
    swap.wait_idle().unwrap();
    assert_eq!(budget.used(), 0);
    let pressure = budget.try_reserve(budget.limit()).unwrap();
    assert!(swap.try_get(&1, || false).is_err());
    drop(pressure);
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!((restored.width(), restored.height()), (6000, 4000));
    assert_eq!(restored.color_space(), &RasterColorSpace::LinearSrgb);
    assert_eq!(digest(&restored), expected);
    assert_eq!(gpu.render_raster(&restored, parameters, [128, 96]).pixels, sdr);
    assert_eq!(render_hdr_float_metal_sized(&restored, hdr_size, hdr_zoom), hdr);
    drop(restored);
    assert_eq!(budget.used(), 0);
    assert_eq!(
        (swap.stats().reads, swap.stats().writes, swap.stats().errors),
        (1, 1, 0)
    );
    eprintln!(
        "photographic HEIF: 6000x4000; all float bits digest exact; Metal SDR/HDR exact; pressure retry; root=0; queue=0"
    );
}

fn qualify_hdr_still_transport(name: &str) {
    let hlg = name.contains("hlg");
    let gpu = common::qualification_gpu().expect("HDR still transport requires a GPU");
    eprintln!("HDR still adapter: {}", gpu.adapter_name());
    let budget = rrrah_core::MemoryBudget::new(512 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/raster")
            .join(name),
    );
    request.memory_budget = Some(budget.clone());
    request.pq_reference_white_nits = if hlg { None } else { Some(100.) };
    request.hlg_display = if hlg { Some((100., 1000., 1.2)) } else { None };
    let source = rrrah_decode::decode_raster(&request).unwrap();
    let extent = (source.width(), source.height());
    assert_eq!(source.color_space(), &RasterColorSpace::LinearSrgb);
    let RasterPixels::Rgba32Float(pixels) = source.pixels() else {
        panic!()
    };
    let expected: Vec<u32> = pixels.iter().map(|v| v.to_bits()).collect();
    assert!(pixels.chunks_exact(4).any(|p| p[0] > if hlg { 9.5 } else { 95. }));
    let float_before = render_hdr_float_metal(&source);
    assert!(
        float_before
            .chunks_exact(4)
            .any(|p| p[0] > if hlg { 0x4c00 } else { 0x5800 })
    );
    assert!(float_before.chunks_exact(4).all(|p| p[3] == 0x3c00));
    if name.contains("alpha") {
        let half_value = |bits: u16| {
            let sign = if bits & 0x8000 == 0 { 1. } else { -1. };
            let exponent = (bits >> 10) & 31;
            assert_ne!(exponent, 31, "finite HDR composition");
            sign * if exponent == 0 {
                f64::from(bits & 1023) * 2f64.powi(-24)
            } else {
                f64::from(1024 + (bits & 1023)) * 2f64.powi(i32::from(exponent) - 25)
            }
        };
        // 64x48 image fitted into 128x64 with zoom=2: these coordinates
        // lie strictly inside each constant RGB/alpha band, away from filtering edges.
        for (row, y) in [8_usize, 24, 40, 56].into_iter().enumerate() {
            for (band, x) in [32_usize, 53, 75, 96].into_iter().enumerate() {
                let source = &pixels[((row * 12 + 6) * 64 + band * 16 + 8) * 4..];
                let alpha = f64::from(source[3]);
                for channel in 0..3 {
                    let expected = f64::from(source[channel]) * alpha * 2. + 0.018 * (1. - alpha);
                    let actual = half_value(float_before[(y * 128 + x) * 4 + channel]);
                    assert!(
                        (actual - expected).abs() <= expected.abs().max(0.01) * 0.001,
                        "{name} band {band} alpha row {row} channel {channel}: {actual} != {expected}"
                    );
                }
            }
        }
        eprintln!("HEVC HDR alpha: {name}; 16 interior controls match linear premultiplied composition");
    }
    if name.contains("color") {
        assert!(
            float_before
                .chunks_exact(4)
                .any(|p| p[..3].iter().any(|v| *v & 0x8000 != 0 && *v & 0x7fff != 0))
        );
    }
    let peak_bits = float_before
        .chunks_exact(4)
        .map(|p| p[0])
        .filter(|v| v & 0x8000 == 0)
        .max()
        .unwrap();
    assert!(peak_bits & 0x8000 == 0 && peak_bits & 0x7c00 != 0x7c00);
    let peak = f64::from(1024 + (peak_bits & 1023)) * 2f64.powi(i32::from((peak_bits >> 10) & 31) - 25);
    let cpu_peak = pixels.chunks_exact(4).map(|p| p[0]).fold(0f32, f32::max);
    assert!((peak - f64::from(cpu_peak) * 2.0).abs() <= 0.25);
    let weight = source.capacity_bytes();
    assert_eq!(weight, u64::from(extent.0) * u64::from(extent.1) * 16);
    let parameters = ViewParameters { zoom: 1.0, ..view() };
    let before = gpu.render_raster(&source, parameters, [128, 64]);
    assert!(
        before
            .pixels
            .chunks_exact(4)
            .any(|p| p[..3] != before.pixels[..3])
    );
    let directory = tempfile::tempdir().unwrap();
    let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 1024 * 1024,
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
    let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(weight));
    ram.enable_swap(swap);
    assert!(ram.insert_visible(1, source));
    let lease = ram.get_lease(&1).unwrap();
    assert!(!ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
    drop(lease);
    ram.mark_visible(2);
    assert!(ram.set_limits_and_spill(rrrah_cache::CacheLimits::bytes(0)));
    let swap = ram.swap().unwrap();
    swap.wait_idle().unwrap();
    assert_eq!(budget.used(), 0);
    let pressure = budget.try_reserve(budget.limit()).unwrap();
    assert!(swap.try_get(&1, || false).is_err());
    drop(pressure);
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!(restored.color_space(), &RasterColorSpace::LinearSrgb);
    assert_eq!((restored.width(), restored.height()), extent);
    let RasterPixels::Rgba32Float(pixels) = restored.pixels() else {
        panic!()
    };
    assert_eq!(pixels.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), expected);
    assert_eq!(budget.used(), weight);
    let float_after = render_hdr_float_metal(&restored);
    assert_eq!(float_before, float_after);
    let after = gpu.render_raster(&restored, parameters, [128, 64]);
    assert_eq!(before.pixels, after.pixels);
    let alias = restored.clone();
    drop(restored);
    assert_eq!(budget.used(), weight);
    drop(alias);
    assert_eq!(budget.used(), 0);
    assert_eq!(swap.stats().writes, 1);
    assert_eq!(swap.stats().reads, 1);
    assert_eq!(swap.stats().errors, 0);
}

#[test]
fn avif_clean_aperture_ram_swap_and_metal_preserve_pixels() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let mut names = vec!["avif-clean-aperture-integer.avif".to_owned()];
    names.extend((2..=8).map(|i| format!("avif-clean-aperture-orientation-{i}.avif")));
    names.push("avif-clean-aperture-top-left-integer.avif".to_owned());
    names.extend((2..=8).map(|i| format!("avif-clean-aperture-top-left-orientation-{i}.avif")));
    names.push("avif-libavif-crop-rgb.avif".to_owned());
    names.extend((2..=8).map(|i| format!("avif-libavif-crop-orientation-{i}.avif")));
    for name in names {
        let path = root.join(&name);
        let decoded = rrrah_decode::decode_raster(&rrrah_decode::DecodeRequest::new(&path)).unwrap();
        let dimensions = if name == "avif-libavif-crop-rgb.avif" {
            (8, 8)
        } else if name.contains("integer") {
            (8, 6)
        } else if ["-5.avif", "-6.avif", "-7.avif", "-8.avif"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
        {
            (2, 4)
        } else {
            (4, 2)
        };
        assert_eq!((decoded.width(), decoded.height()), dimensions);
        assert_eq!(decoded.color_space(), &RasterColorSpace::Srgb);
        let RasterPixels::Rgba8(pixels) = decoded.pixels() else {
            panic!()
        };
        let oracle = std::fs::read(root.join(format!("{name}.rgba"))).unwrap();
        assert_eq!(pixels.len(), oracle.len());
        assert!(pixels.iter().zip(&oracle).all(|(a, b)| a.abs_diff(*b) <= 3));
        let actual = pixels.to_vec();
        drop(decoded);
        qualify_u8_raster_transport(path, 4096, Some(&actual), RasterColorSpace::Srgb);
    }
}

fn qualify_pict_transport(path: std::path::PathBuf, limit: u64, oracle: Option<&[u8]>) {
    qualify_u8_raster_transport(path, limit, oracle, RasterColorSpace::AssumedSrgb);
}

fn qualify_u8_raster_transport(
    path: std::path::PathBuf,
    limit: u64,
    oracle: Option<&[u8]>,
    color: RasterColorSpace,
) {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("Raster transport adapter: {}", gpu.adapter_name());
    let budget = rrrah_core::MemoryBudget::new(limit);
    let mut request = rrrah_decode::DecodeRequest::new(path);
    request.memory_budget = Some(budget.clone());
    request.assume_untagged_srgb = true;
    let rrrah_decode::DecodedImage::Raster(decoded) = rrrah_decode::decode_image(&request).unwrap() else {
        panic!()
    };
    if let Some(oracle) = oracle {
        let RasterPixels::Rgba8(p) = decoded.pixels() else {
            panic!()
        };
        assert_eq!(&**p, oracle);
    }
    let dimensions = (decoded.width(), decoded.height());
    let weight = decoded.capacity_bytes();
    let expected = decoded.clone();
    let mut ram = rrrah_cache::LeaseCache::new(rrrah_cache::CacheLimits {
        max_bytes: weight,
        max_entries: Some(1),
        ttl: None,
    });
    ram.insert(1u8, decoded, weight).unwrap();
    let lease = ram.get(&1).unwrap();
    assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
    let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&lease, Some(&budget)).unwrap();
    let parameters = view();
    let before = gpu.render_raster(&prepared, parameters, [64, 64]).pixels;
    if oracle.is_some() {
        assert!(
            before.chunks_exact(4).any(|pixel| pixel != &before[..4]),
            "oracle-qualified raster GPU frame must contain image detail"
        );
    }
    drop(prepared);
    drop(lease);
    let (_, victim) = ram.take_lru().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: limit,
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
    swap.enqueue(1, victim);
    swap.wait_idle().unwrap();
    // The independent comparison retains source allocation until explicitly dropped.
    let expected_pixels = match expected.pixels() {
        RasterPixels::Rgba8(p) => p.to_vec(),
        _ => panic!(),
    };
    drop(expected);
    assert_eq!(budget.used(), 0);
    let pressure = budget.try_buffer(limit as usize, 0u8).unwrap().freeze();
    assert!(swap.try_get(&1, || false).is_err());
    drop(pressure);
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    // The local restore cap must bind even while the shared parent has room.
    assert!(budget.available_bytes() >= weight);
    assert!(matches!(swap.try_get(&1,||false),
        Err(rrrah_core::BufferError::Capacity {limit,..}) if limit<=weight));
    let retained = restored.clone();
    assert_eq!((restored.width(), restored.height()), dimensions);
    assert_eq!(restored.color_space(), &color);
    let RasterPixels::Rgba8(p) = restored.pixels() else {
        panic!()
    };
    assert_eq!(&**p, &expected_pixels);
    if let Some(oracle) = oracle {
        assert_eq!(&**p, oracle);
    }
    let ready = rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
    assert_eq!(gpu.render_raster(&ready, parameters, [64, 64]).pixels, before);
    drop(ready);
    drop(restored);
    assert_eq!(budget.used(), weight);
    assert!(matches!(swap.try_get(&1,||false),
        Err(rrrah_core::BufferError::Capacity {limit,..}) if limit<=weight));
    drop(retained);
    assert_eq!(budget.used(), 0);
    let retry = swap.try_get(&1, || false).unwrap().unwrap();
    let RasterPixels::Rgba8(p) = retry.pixels() else {
        panic!()
    };
    assert_eq!(&**p, &expected_pixels);
    drop(retry);
    assert_eq!(budget.used(), 0);
    assert_eq!(swap.stats().writes, 1);
    assert_eq!(swap.stats().reads, 2);
}

#[test]
fn native_farbfeld_preserves_u16_and_display_through_budgeted_swap() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("native Farbfeld adapter: {}", gpu.adapter_name());
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/raster/adjacent16.ff");
    for prepared_swap in [false, true] {
        let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(path.clone());
        request.memory_budget = Some(budget.clone());
        let native = rrrah_decode::decode_raster(&request).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let RasterPixels::Rgba16(values) = native.pixels() else {
            panic!()
        };
        let expected: Vec<u16> = bytes[16..]
            .chunks_exact(2)
            .map(|v| u16::from_be_bytes([v[0], v[1]]))
            .collect();
        assert_eq!(&values[..], expected.as_slice());
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&budget)).unwrap();
        let parameters = ViewParameters { zoom: 1.0, ..view() };
        let expected_frame = gpu.render_raster(&prepared, parameters, [64, 64]).pixels;
        let frame = if prepared_swap {
            drop(native);
            prepared
        } else {
            drop(prepared);
            native
        };
        let weight = frame.capacity_bytes();
        let color = frame.color_space().clone();
        let mut expected_payload = Vec::new();
        rrrah_cache::SwapPayload::write_payload(&frame, &mut expected_payload).unwrap();
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
        swap.enqueue(1, frame);
        swap.wait_idle().unwrap();
        assert_eq!(budget.used(), 0);
        let pressure = budget.try_reserve(budget.limit()).unwrap();
        assert!(swap.try_get(&1, || false).is_err());
        drop(pressure);
        assert!(swap.try_get(&1, || true).unwrap().is_none());
        assert_eq!(budget.used(), 0);
        let restored = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(restored.color_space(), &color);
        let mut payload = Vec::new();
        rrrah_cache::SwapPayload::write_payload(&restored, &mut payload).unwrap();
        assert_eq!(payload, expected_payload);
        let display = rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
        assert_eq!(
            gpu.render_raster(&display, parameters, [64, 64]).pixels,
            expected_frame
        );
        drop(display);
        drop(restored);
        assert_eq!(budget.used(), 0);
        assert_eq!(swap.stats().reads, 1);
        assert_eq!(swap.stats().writes, 1);
        assert_eq!(swap.stats().queued_bytes, 0);
        assert_eq!(swap.stats().errors, 0);
    }
}

#[test]
fn icc_images_prepare_and_restore_exactly_for_linear_hdr_metal() {
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    for name in ["pattern.profiled.png", "pattern.profiled.tif"] {
        let root = rrrah_core::MemoryBudget::new(4 * 1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(fixtures.join(name));
        request.memory_budget = Some(root.clone());
        let decoded = rrrah_decode::decode_raster(&request).unwrap();
        let RasterColorSpace::Icc(profile) = decoded.color_space() else {
            panic!("ICC missing")
        };
        let profile = profile.clone();
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&decoded, Some(&root)).unwrap();
        assert_eq!(prepared.color_space(), &RasterColorSpace::LinearSrgb);
        let expected = render_hdr_float_metal(&prepared);
        assert!(
            expected
                .chunks_exact(4)
                .any(|pixel| pixel[..3].iter().any(|&v| v > 0x3c00 && v < 0x7c00)),
            "HDR exposure was clipped to SDR"
        );
        let directory = tempfile::tempdir().unwrap();
        let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
            directory.path(),
            rrrah_cache::ImageSwapConfig {
                limits: rrrah_cache::CacheLimits::bytes(1024 * 1024),
                queue_bytes: 1024 * 1024,
                queue_count: 2,
                restore_bytes: 1024 * 1024,
            },
            rrrah_core::MemoryBudget::new(1024 * 1024),
            root.clone(),
        )
        .unwrap();
        swap.enqueue(1, decoded);
        swap.wait_idle().unwrap();
        drop(prepared);
        assert_eq!(root.used(), 0);
        let pressure = root.try_reserve(root.limit()).unwrap();
        assert!(swap.try_get(&1, || false).is_err());
        drop(pressure);
        let restored = swap.try_get(&1, || false).unwrap().unwrap();
        assert_eq!(restored.color_space(), &RasterColorSpace::Icc(profile));
        let prepared = rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&root)).unwrap();
        assert_eq!(
            render_hdr_float_metal(&prepared),
            expected,
            "{name}: HDR float frame changed through ICC swap"
        );
        drop(prepared);
        drop(restored);
        drop(swap);
        assert_eq!(root.used(), 0);
    }
}

#[test]
fn qoi_transfer_flags_and_alpha_survive_native_prepared_swap_and_metal() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("QOI transport adapter: {}", gpu.adapter_name());
    let rgba = [
        128u8, 64, 32, 255, 16, 96, 192, 128, 240, 10, 50, 0, 80, 160, 240, 255,
    ];
    let directory = tempfile::tempdir().unwrap();
    let mut prior_frame = None;
    for flag in [0u8, 1] {
        // Authored QOI_RGBA chunks, not an encoder/decoder round trip.
        let mut bytes = b"qoif".to_vec();
        bytes.extend(4u32.to_be_bytes());
        bytes.extend(1u32.to_be_bytes());
        bytes.extend([4, flag]);
        for pixel in rgba.chunks_exact(4) {
            bytes.push(0xff);
            bytes.extend(pixel);
        }
        bytes.extend([0, 0, 0, 0, 0, 0, 0, 1]);
        let path = directory.path().join(format!("transfer-{flag}.qoi"));
        std::fs::write(&path, bytes).unwrap();
        let color = if flag == 0 {
            RasterColorSpace::Srgb
        } else {
            RasterColorSpace::LinearSrgb
        };
        let golden = DecodedRaster::new(
            4,
            1,
            RasterPixels::Rgba8(Arc::new(rgba.to_vec()).into()),
            color.clone(),
        )
        .unwrap()
        .to_linear_srgb()
        .unwrap();
        let parameters = ViewParameters { zoom: 1.0, ..view() };
        let expected_frame = gpu.render_raster(&golden, parameters, [64, 64]).pixels;
        if let Some(prior) = prior_frame.replace(expected_frame.clone()) {
            assert_ne!(prior, expected_frame, "QOI transfer flag did not affect display");
        }
        for prepared_swap in [false, true] {
            let budget = rrrah_core::MemoryBudget::new(1024 * 1024);
            let mut request = rrrah_decode::DecodeRequest::new(&path);
            request.memory_budget = Some(budget.clone());
            let native = rrrah_decode::decode_raster(&request).unwrap();
            assert_eq!(native.color_space(), &color);
            let RasterPixels::Rgba8(pixels) = native.pixels() else {
                panic!()
            };
            assert_eq!(&pixels[..], &rgba);
            let prepared =
                rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&budget)).unwrap();
            assert_eq!(
                gpu.render_raster(&prepared, parameters, [64, 64]).pixels,
                expected_frame
            );
            let frame = if prepared_swap {
                drop(native);
                prepared
            } else {
                drop(prepared);
                native
            };
            let weight = frame.capacity_bytes();
            let swap_dir = tempfile::tempdir().unwrap();
            let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
                swap_dir.path(),
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
            let mut expected_payload = Vec::new();
            rrrah_cache::SwapPayload::write_payload(&frame, &mut expected_payload).unwrap();
            swap.enqueue(1, frame);
            swap.wait_idle().unwrap();
            assert_eq!(budget.used(), 0);
            let held = budget.try_reserve(budget.limit()).unwrap();
            assert!(swap.try_get(&1, || false).is_err());
            drop(held);
            assert!(swap.try_get(&1, || true).unwrap().is_none());
            assert_eq!(budget.used(), 0);
            let restored = swap.try_get(&1, || false).unwrap().unwrap();
            let mut payload = Vec::new();
            rrrah_cache::SwapPayload::write_payload(&restored, &mut payload).unwrap();
            assert_eq!(payload, expected_payload);
            let display =
                rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
            assert_eq!(
                gpu.render_raster(&display, parameters, [64, 64]).pixels,
                expected_frame
            );
            drop(display);
            drop(restored);
            assert_eq!(budget.used(), 0);
            assert_eq!(swap.stats().reads, 1);
            assert_eq!(swap.stats().writes, 1);
            assert_eq!(swap.stats().errors, 0);
        }
    }
}

#[test]
#[ignore = "manual 12 MP float HDR attachment and disk swap qualification"]
fn large_float_hdr_swap_preserves_full_metal_attachment() {
    const WIDTH: u32 = 4000;
    const HEIGHT: u32 = 3000;
    const BYTES: u64 = WIDTH as u64 * HEIGHT as u64 * 16;
    let root = rrrah_core::MemoryBudget::new(BYTES);
    let queue = rrrah_core::MemoryBudget::new(BYTES);
    let colors = [
        [4.0f32, 2.0, -0.5, 1.0],
        [0.25, 8.0, 2.0, 1.0],
        [16.0, -1.0, 0.5, 1.0],
        [-0.25, 0.0, 32.0, 1.0],
    ];
    let expected_half = [
        [0x4800u16, 0x4400, 0xbc00, 0x3c00],
        [0x3800, 0x4c00, 0x4400, 0x3c00],
        [0x5000, 0xc000, 0x3c00, 0x3c00],
        [0xb800, 0x0000, 0x5400, 0x3c00],
    ];
    let quadrant = |x: u32, y: u32| usize::from(x >= WIDTH / 2) + 2 * usize::from(y >= HEIGHT / 2);
    let mut values = Vec::with_capacity((BYTES / 4) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            values.extend_from_slice(&colors[quadrant(x, y)]);
        }
    }
    let source = DecodedRaster::new(
        WIDTH,
        HEIGHT,
        RasterPixels::Rgba32Float(Arc::new(values).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap()
    .try_manage_pixels(&root)
    .unwrap();
    // Match one source pixel to one output pixel despite the viewer fit margin.
    let zoom = HEIGHT as f32 / (HEIGHT - 32) as f32;
    let before = render_hdr_float_metal_sized(&source, [WIDTH, HEIGHT], zoom);
    assert_eq!(before.len(), WIDTH as usize * HEIGHT as usize * 4);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            // Exclude the one-pixel boundary neighbourhood from the independent
            // constant-region oracle; the full before/after comparison includes it.
            if x.abs_diff(WIDTH / 2) <= 1 || y.abs_diff(HEIGHT / 2) <= 1 {
                continue;
            }
            let at = (y as usize * WIDTH as usize + x as usize) * 4;
            assert_eq!(&before[at..at + 4], &expected_half[quadrant(x, y)], "x={x} y={y}");
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::RasterSwapCache::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits::bytes(BYTES + 65536),
            queue_bytes: BYTES,
            queue_count: 1,
            restore_bytes: BYTES,
        },
        queue.clone(),
        root.clone(),
    )
    .unwrap();
    swap.enqueue(1, source);
    swap.wait_idle().unwrap();
    assert_eq!((root.used(), queue.used()), (0, 0));
    let pressure = root.try_reserve(root.limit()).unwrap();
    assert!(swap.try_get(&1, || false).is_err());
    drop(pressure);
    let mut admitted = false;
    assert!(
        swap.try_get(&1, || {
            admitted |= root.used() > 0;
            admitted
        })
        .unwrap()
        .is_none()
    );
    assert!(
        admitted,
        "12 MP float cancellation must follow allocation admission"
    );
    assert_eq!((root.used(), queue.used()), (0, 0));
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    let RasterPixels::Rgba32Float(values) = restored.pixels() else {
        panic!()
    };
    for (index, actual) in values.chunks_exact(4).enumerate() {
        let reference = colors[quadrant((index % WIDTH as usize) as u32, (index / WIDTH as usize) as u32)];
        for channel in 0..4 {
            assert_eq!(actual[channel].to_bits(), reference[channel].to_bits());
        }
    }
    assert_eq!(
        render_hdr_float_metal_sized(&restored, [WIDTH, HEIGHT], zoom),
        before
    );
    assert_eq!(
        (swap.stats().writes, swap.stats().reads, swap.stats().errors),
        (1, 1, 0)
    );
    drop(restored);
    drop(swap);
    assert_eq!((root.used(), queue.used()), (0, 0));
    eprintln!(
        "12 MP HDR float attachment: all binary16 pixels exact after swap; independent bright/negative/exposure oracle; root=0; queue=0"
    );
}

#[test]
fn declared_radiance_bt2020_keeps_extended_gamut_through_gpu() {
    qualify_declared_linear_gamut(
        "rec2020-linear-red.hdr",
        9,
        frame(1, vec![3.320982, -0.24910095, -0.03630153, 1.0]),
    );
}

#[test]
fn declared_radiance_p3_keeps_rgbw_hdr_through_swap_and_gpu() {
    qualify_declared_linear_gamut(
        "p3-linear-rgbw.hdr",
        12,
        frame(
            4,
            vec![
                2.4498803525611192,
                -0.08411390941937635,
                -0.03927510918066887,
                1.0,
                -0.4498803525611199,
                2.0841139094193766,
                -0.15727209110126364,
                1.0,
                -2.220446049250313e-16,
                -4.163336342344337e-17,
                2.196547200281933,
                1.0,
                2.0,
                2.0,
                2.0,
                1.0,
            ],
        ),
    );
}

fn qualify_declared_linear_gamut(name: &str, primaries: u16, golden: DecodedRaster) {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/raster")
        .join(name);
    let root = rrrah_core::MemoryBudget::new(1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(path);
    request.memory_budget = Some(root.clone());
    let source = rrrah_decode::decode_raster(&request).unwrap();
    qualify_linear_source(source, root, primaries, golden, name);
}

fn qualify_linear_source(
    source: DecodedRaster,
    root: rrrah_core::MemoryBudget,
    primaries: u16,
    golden: DecodedRaster,
    name: &str,
) {
    let Some(gpu) = common::qualification_gpu() else {
        return;
    };
    eprintln!("Declared linear gamut adapter: {}", gpu.adapter_name());
    let expected_color = source.color_space().clone();
    let source_weight = root.used();
    let pressure = root.try_reserve(root.available_bytes()).unwrap();
    assert!(rrrah_decode::prepare_raster_for_display_with_budget(&source, Some(&root)).is_err());
    drop(pressure);
    assert!(
        rrrah_decode::prepare_raster_for_display_with_budget_and_cancel(&source, Some(&root), || true)
            .is_err()
    );
    assert_eq!(root.used(), source_weight);
    let linear = rrrah_decode::prepare_raster_for_display_with_budget(&source, Some(&root)).unwrap();
    let (RasterPixels::Rgba32Float(actual), RasterPixels::Rgba32Float(expected)) =
        (linear.pixels(), golden.pixels())
    else {
        panic!()
    };
    for (actual, expected) in actual.iter().zip(expected.iter()) {
        assert!((actual - expected).abs() < 0.000002);
    }
    assert!(actual.iter().any(|v| *v > 1.0));
    assert!(actual.iter().any(|v| *v < 0.0));
    assert_eq!(
        gpu.render_raster(&linear, view(), [64, 64]).pixels,
        gpu.render_raster(&golden, view(), [64, 64]).pixels
    );
    assert_eq!(
        gpu.render_raster_half(&linear, view(), [64, 64]),
        gpu.render_raster_half(&golden, view(), [64, 64])
    );
    let weight = source.capacity_bytes();
    let directory = tempfile::tempdir().unwrap();
    let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits::bytes(1024 * 1024),
            queue_bytes: weight,
            queue_count: 1,
            restore_bytes: weight,
        },
        rrrah_core::MemoryBudget::new(weight),
        root.clone(),
    )
    .unwrap();
    swap.enqueue(1, source);
    swap.wait_idle().unwrap();
    drop(linear);
    assert_eq!(root.used(), 0);
    let pressure = root.try_reserve(root.limit()).unwrap();
    assert!(swap.try_get(&1, || false).is_err());
    drop(pressure);
    assert!(swap.try_get(&1, || true).unwrap().is_none());
    assert_eq!(root.used(), 0);
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!(restored.color_space(), &expected_color);
    let linear = rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&root)).unwrap();
    assert_eq!(
        gpu.render_raster(&linear, view(), [64, 64]).pixels,
        gpu.render_raster(&golden, view(), [64, 64]).pixels
    );
    assert_eq!(
        gpu.render_raster_half(&linear, view(), [64, 64]),
        gpu.render_raster_half(&golden, view(), [64, 64])
    );
    assert_eq!(swap.stats().errors, 0);
    drop((restored, linear, swap));
    assert_eq!(root.used(), 0);
    eprintln!(
        "Declared linear gamut {primaries}: {name}; declared source preserved; pressure/cancel/swap retry; SDR/HDR analytical frame exact; root=0"
    );
}

#[test]
fn declared_exr_wide_gamuts_preserve_native_color_and_metal_swap() {
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    for primaries in [9, 12] {
        for tag in ["chroma", "id", "both", "neutral", "id-neutral"] {
            let oracle = if tag.contains("neutral") {
                format!("wide-{primaries}-{tag}.linear-srgb.f32")
            } else {
                format!("wide-{primaries}.linear-srgb.f32")
            };
            let bytes = std::fs::read(fixtures.join(oracle)).unwrap();
            let values = bytes
                .chunks_exact(4)
                .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                .collect::<Vec<_>>();
            let golden = DecodedRaster::new(
                3,
                2,
                RasterPixels::Rgba32Float(Arc::new(values).into()),
                RasterColorSpace::LinearSrgb,
            )
            .unwrap();
            qualify_declared_linear_gamut(&format!("wide-{primaries}-{tag}.exr"), primaries, golden);
        }
        for tag in ["conflict", "aces"] {
            let request =
                rrrah_decode::DecodeRequest::new(fixtures.join(format!("wide-{primaries}-{tag}.exr")));
            assert!(matches!(
                rrrah_decode::decode_raster(&request),
                Err(rrrah_decode::RasterDecodeError::InvalidExrColor(_))
            ));
        }
        for tag in ["unknown"] {
            let root = rrrah_core::MemoryBudget::new(1024 * 1024);
            let mut request =
                rrrah_decode::DecodeRequest::new(fixtures.join(format!("wide-{primaries}-{tag}.exr")));
            request.memory_budget = Some(root.clone());
            let source = rrrah_decode::decode_raster(&request).unwrap();
            assert_eq!(source.color_space(), &RasterColorSpace::LinearRgbUnspecified);
            assert!(rrrah_decode::prepare_raster_for_display_with_budget(&source, Some(&root)).is_err());
            drop(source);
            assert_eq!(root.used(), 0);
        }
    }
    eprintln!(
        "EXR wide color: 10 qualified metadata cases; 4 conflicting cases refused; 2 unknown-ID cases remain unqualified; root=0"
    );
}

#[test]
fn explicit_chromaticity_metadata_survives_swap_and_metal() {
    let root = rrrah_core::MemoryBudget::new(1024 * 1024);
    let color = RasterColorSpace::LinearRgb(
        rrrah_core::RgbChromaticities::new([0.680, 0.320, 0.265, 0.690, 0.150, 0.060, 0.3127, 0.3290])
            .unwrap(),
    );
    let source = DecodedRaster::new(
        1,
        1,
        RasterPixels::Rgba32Float(Arc::new(vec![2.0, 0.0, 0.0, 1.0]).into()),
        color,
    )
    .unwrap()
    .try_manage_pixels(&root)
    .unwrap();
    qualify_linear_source(
        source,
        root,
        0,
        frame(
            1,
            vec![
                2.4498803525611192,
                -0.08411390941937635,
                -0.03927510918066887,
                1.0,
            ],
        ),
        "explicit xy metadata",
    );
}

#[test]
fn radiance_arbitrary_primaries_and_non_d65_white_survive_swap_and_metal() {
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    for name in ["custom-white", "ap1-white", "ap0-white"] {
        let bytes = std::fs::read(fixtures.join(format!("{name}.linear-srgb.f32"))).unwrap();
        let values = bytes
            .chunks_exact(4)
            .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
            .collect::<Vec<_>>();
        qualify_declared_linear_gamut(&format!("{name}.hdr"), 0, frame(4, values));
        let source = rrrah_decode::decode_raster(&rrrah_decode::DecodeRequest::new(
            fixtures.join(format!("{name}.hdr")),
        ))
        .unwrap();
        assert!(matches!(source.color_space(), RasterColorSpace::LinearRgb(_)));
        let RasterPixels::Rgba32Float(values) = source.pixels() else {
            panic!()
        };
        assert_eq!(
            &values[..],
            &[2., 0., 0., 1., 0., 2., 0., 1., 0., 0., 2., 1., 2., 2., 2., 1.]
        );
    }
    eprintln!(
        "Radiance arbitrary color: 3 non-D65 RGBW sources; independent full matrix oracle; native samples unchanged; Metal SDR/HDR and swap retry exact"
    );
}

#[test]
fn malformed_radiance_primaries_release_decode_admission() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("malformed.hdr");
    for declaration in [
        "PRIMARIES= .64 .33 .64 .33 .64 .33 .3127 .3290",
        "PRIMARIES= NaN .33 .30 .60 .15 .06 .3127 .3290",
        "PRIMARIES= .64 .33 .30 .60 .15 .06 .3127 0",
        "PRIMARIES= .64 .33 .30 .60 .15 .06 .3127 .3290\nPRIMARIES= .64 .33 .30 .60 .15 .06 .3127 .3290",
    ] {
        let mut bytes =
            format!("#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n{declaration}\n\n-Y 1 +X 1\n").into_bytes();
        bytes.extend([128, 0, 0, 130]);
        std::fs::write(&path, bytes).unwrap();
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(&path);
        request.memory_budget = Some(root.clone());
        assert!(matches!(
            rrrah_decode::decode_raster(&request),
            Err(rrrah_decode::RasterDecodeError::InvalidHdrColor(_))
        ));
        assert_eq!(root.used(), 0);
    }
}

#[test]
fn exr_arbitrary_primaries_and_aces_ids_preserve_color_through_metal_swap() {
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("exr-arbitrary-color.json")).unwrap()).unwrap();
    let native_bytes = std::fs::read(fixtures.join("raster/float-none.exr.f32")).unwrap();
    let native = native_bytes
        .chunks_exact(4)
        .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
        .collect::<Vec<_>>();
    let mut counts = [0usize; 3];
    for case in manifest["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let mut request = rrrah_decode::DecodeRequest::new(fixtures.join("raster").join(name));
        request.memory_budget = Some(root.clone());
        match case["outcome"].as_str().unwrap() {
            "accepted" => {
                let bytes =
                    std::fs::read(fixtures.join("raster").join(case["oracle"].as_str().unwrap())).unwrap();
                let values = bytes
                    .chunks_exact(4)
                    .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                    .collect::<Vec<_>>();
                let golden = DecodedRaster::new(
                    3,
                    2,
                    RasterPixels::Rgba32Float(Arc::new(values).into()),
                    RasterColorSpace::LinearSrgb,
                )
                .unwrap();
                qualify_declared_linear_gamut(name, 0, golden);
                let source = rrrah_decode::decode_raster(&request).unwrap();
                let RasterColorSpace::LinearRgb(chroma) = source.color_space() else {
                    panic!()
                };
                for (actual, reference) in chroma.xy().iter().zip(case["xy"].as_array().unwrap()) {
                    assert!((actual - reference.as_f64().unwrap()).abs() < 1e-15);
                }
                let expected_neutral = case["adopted_neutral"]
                    .as_array()
                    .map(|v| [v[0].as_f64().unwrap(), v[1].as_f64().unwrap()]);
                assert_eq!(chroma.adopted_neutral().is_some(), expected_neutral.is_some());
                if let (Some(actual), Some(expected)) = (chroma.adopted_neutral(), expected_neutral) {
                    for (actual, expected) in actual.into_iter().zip(expected) {
                        assert!((actual - expected).abs() < 1e-15);
                    }
                }
                let RasterPixels::Rgba32Float(actual) = source.pixels() else {
                    panic!()
                };
                assert!(
                    actual
                        .iter()
                        .zip(native.iter())
                        .all(|(a, b)| a.to_bits() == b.to_bits())
                );
                counts[0] += 1;
            }
            "refused" => {
                assert!(matches!(
                    rrrah_decode::decode_raster(&request),
                    Err(rrrah_decode::RasterDecodeError::InvalidExrColor(_))
                ));
                counts[1] += 1;
            }
            "unknown" => {
                let source = rrrah_decode::decode_raster(&request).unwrap();
                assert_eq!(source.color_space(), &RasterColorSpace::LinearRgbUnspecified);
                assert!(rrrah_decode::prepare_raster_for_display(&source).is_err());
                counts[2] += 1;
            }
            _ => panic!("unexpected outcome"),
        }
        assert_eq!(root.used(), 0);
    }
    assert_eq!(counts, [17, 11, 6]);
    eprintln!(
        "EXR arbitrary color: accepted=17; refused=11; unknown=6; independent full matrix; native/swap/Metal SDR/HDR exact; root=0"
    );
}

#[test]
fn adopted_neutral_metadata_survives_v4_swap_pressure_and_metal() {
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    for (name, xy, neutral) in [
        (
            "rec709",
            [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290],
            [0.3333, 0.3333],
        ),
        (
            "ap1",
            [0.713, 0.293, 0.165, 0.830, 0.128, 0.044, 0.32168, 0.33767],
            [0.34567, 0.35850],
        ),
    ] {
        let bytes = std::fs::read(fixtures.join(format!("adopted-neutral-{name}.linear-srgb.f32"))).unwrap();
        let values = bytes
            .chunks_exact(4)
            .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
            .collect::<Vec<_>>();
        let root = rrrah_core::MemoryBudget::new(1024 * 1024);
        let color = RasterColorSpace::LinearRgb(
            rrrah_core::RgbChromaticities::new(xy)
                .unwrap()
                .with_adopted_neutral(neutral)
                .unwrap(),
        );
        let source = DecodedRaster::new(
            4,
            1,
            RasterPixels::Rgba32Float(
                Arc::new(vec![
                    2., 0., 0., 1., 0., 2., 0., 1., 0., 0., 2., 1., 2., 2., 2., 1.,
                ])
                .into(),
            ),
            color,
        )
        .unwrap()
        .try_manage_pixels(&root)
        .unwrap();
        qualify_linear_source(
            source,
            root,
            0,
            frame(4, values),
            &format!("adopted-neutral-{name}"),
        );
    }
    eprintln!(
        "Adopted neutral metadata: cases=2; independent full matrix; V4 pressure/cancel/retry; Metal SDR/HDR exact; root=0"
    );
}
