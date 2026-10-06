#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
use std::sync::Arc;

#[test]
fn sti_magic_routes_subimages_even_with_a_raw_suffix() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("misleading.mrw");
    std::fs::write(
        &path,
        include_bytes!("../../../tests/fixtures/sti/etrle-two-images.sti"),
    )
    .unwrap();
    let budget = rrrah_core::MemoryBudget::new(4096);
    let mut request = rrrah_decode::DecodeRequest::new(&path);
    request.image_index = 1;
    request.memory_budget = Some(budget.clone());
    let rrrah_decode::DecodedImage::Raster(image) = rrrah_decode::decode_image(&request).unwrap() else {
        panic!("STCI must route as raster")
    };
    assert_eq!((image.image_index(), image.image_count()), (1, 2));
    assert!(rrrah_decode::is_supported_image_path(std::path::Path::new(
        "image.STI"
    )));
    drop(image);
    assert_eq!(budget.used(), 0);
}

fn assert_equal(a: &DecodedRaster, b: &DecodedRaster) {
    assert_eq!(
        (a.width(), a.height(), a.image_index(), a.image_count()),
        (b.width(), b.height(), b.image_index(), b.image_count())
    );
    assert_eq!(a.color_space(), b.color_space());
    match (a.pixels(), b.pixels()) {
        (RasterPixels::Rgba8(a), RasterPixels::Rgba8(b)) => {
            assert_eq!(a.len(), b.len());
            assert_eq!(
                a.iter().zip(b.iter()).position(|(a, b)| a != b),
                None,
                "first mismatching RGBA8 sample"
            );
        }
        (RasterPixels::Rgba16(a), RasterPixels::Rgba16(b)) => assert_eq!(a, b),
        (RasterPixels::Rgba32Float(a), RasterPixels::Rgba32Float(b)) => assert_eq!(a, b),
        _ => panic!("precision changed"),
    }
}

#[test]
fn sti_images_preserve_native_and_prepared_samples_ram_swap_and_metal() {
    let gpu = common::qualification_gpu().expect("actual GPU required for STI qualification");
    eprintln!("STI cache adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/sti");
    for (name, count, bits) in [
        ("indexed.sti", 1, 8),
        ("indexed-zlib.sti", 1, 8),
        ("etrle-two-images.sti", 2, 8),
        ("rgb16.sti", 1, 16),
        ("rgb16-zlib.sti", 1, 16),
        ("rgb24.sti", 1, 16),
        ("rgb24-zlib.sti", 1, 16),
        ("rgb32.sti", 1, 16),
        ("rgb32-zlib.sti", 1, 16),
    ] {
        for index in 0..count {
            let budget = rrrah_core::MemoryBudget::new(4096);
            let mut request = rrrah_decode::DecodeRequest::new(fixtures.join(name));
            request.memory_budget = Some(budget.clone());
            request.image_index = index;
            let native = rrrah_decode::decode_raster(&request).unwrap();
            let raw = std::fs::read(fixtures.join(format!("{name}-image-{index}.rgba"))).unwrap();
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
            let golden = DecodedRaster::new(3, 2, pixels, RasterColorSpace::AssumedSrgb)
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
#[ignore = "requires pinned external STCI files and oracle outputs"]
fn independent_stci_producer_images_match_external_parser_and_metal() {
    let root = std::path::PathBuf::from(
        std::env::var("RRRAH_STCI_EXTERNAL_DIR").expect("external corpus directory"),
    );
    let gpu = common::qualification_gpu().expect("actual GPU required");
    let mut checked = 0;
    for entry in std::fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "sti") {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap();
        let dimensions = std::fs::read_to_string(root.join(format!("{name}-dimensions.txt"))).unwrap();
        let count = dimensions.lines().count();
        for line in dimensions.lines() {
            let columns: Vec<_> = line.split_whitespace().collect();
            let index: usize = columns[0].parse().unwrap();
            let width: u32 = columns[1].parse().unwrap();
            let height: u32 = columns[2].parse().unwrap();
            let budget = rrrah_core::MemoryBudget::new(4 * 1024 * 1024);
            let mut request = rrrah_decode::DecodeRequest::new(&path);
            request.image_index = index;
            request.memory_budget = Some(budget.clone());
            let source = rrrah_decode::decode_raster(&request).unwrap();
            let raw = std::fs::read(root.join(format!("{name}-image-{index}.rgba"))).unwrap();
            let golden = DecodedRaster::new(
                width,
                height,
                RasterPixels::Rgba8(Arc::new(raw).into()),
                RasterColorSpace::AssumedSrgb,
            )
            .unwrap()
            .with_image_selection(index, count)
            .unwrap();
            eprintln!("comparing {name} image {index}");
            assert_equal(&source, &golden);
            let display =
                rrrah_decode::prepare_raster_for_display_with_budget(&source, Some(&budget)).unwrap();
            let expected = golden.to_linear_srgb().unwrap();
            assert_equal(&display, &expected);
            let parameters = rrrah_gpu::ViewParameters {
                viewport: [96., 64.],
                ..Default::default()
            };
            assert_eq!(
                gpu.render_raster(&display, parameters, [96, 64]).pixels,
                gpu.render_raster(&expected, parameters, [96, 64]).pixels
            );
            let frame = gpu.render_raster(&expected, parameters, [96, 64]).pixels;
            for value in [source, display] {
                let weight = value.capacity_bytes();
                let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits {
                    max_bytes: weight,
                    max_entries: Some(1),
                    ttl: None,
                });
                assert!(ram.insert(index, value.clone()));
                let lease = ram.get_lease(&index).unwrap();
                assert_equal(&lease, &value);
                assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
                drop(lease);
                drop(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).unwrap());
                let directory = tempfile::tempdir().unwrap();
                let swap: rrrah_cache::RasterSwapCache<usize> =
                    rrrah_cache::ImageSwapCache::new_with_budgets(
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
                assert_eq!(
                    swap.try_enqueue(index, value.clone()),
                    rrrah_cache::SpillAdmission::Queued
                );
                swap.wait_idle().unwrap();
                let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
                assert!(swap.try_get(&index, || false).is_err());
                assert_eq!(swap.stats().errors, 0);
                drop(pressure);
                let restored = swap.try_get(&index, || false).unwrap().unwrap();
                assert_equal(&restored, &value);
                let restored_display =
                    rrrah_decode::prepare_raster_for_display_with_budget(&restored, Some(&budget)).unwrap();
                assert_eq!(
                    gpu.render_raster(&restored_display, parameters, [96, 64]).pixels,
                    frame
                );
                drop((restored_display, restored, value, ram));
                assert_eq!(
                    (swap.stats().writes, swap.stats().reads, swap.stats().errors),
                    (1, 1, 0)
                );
            }
            assert_eq!(budget.used(), 0);
            checked += 1;
            eprintln!(
                "external {name} image {index}: native/prepared samples, RAM leases, swap pressure/retry and Metal exact"
            );
        }
    }
    assert_eq!(checked, 20);
}
