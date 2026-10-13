#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "actual Metal selected ICO image RAM/TTL/swap qualification"]
fn selected_ico_images_keep_quality_order_alpha_and_metal_frames() {
    qualify_selected_images(false, 0);
}

#[test]
#[ignore = "actual Metal selected CUR hotspot/alpha RAM/TTL/swap qualification"]
fn selected_cursor_images_keep_matching_hotspots_and_metal_frames() {
    qualify_selected_images(true, 0);
}

#[test]
#[ignore = "actual Metal selected DIB CUR mask/hotspot RAM/TTL/swap qualification"]
fn selected_dib_cursor_images_keep_masks_hotspots_and_metal_frames() {
    qualify_selected_images(true, 24);
}

#[test]
#[ignore = "actual Metal indexed and BGRA CUR RAM/TTL/swap qualification"]
fn selected_indexed_and_bgra_cursor_images_keep_masks_and_metal_frames() {
    for depth in [1, 4, 8, 16, 32] {
        qualify_selected_images(true, depth);
    }
}

fn qualify_selected_images(cursor: bool, depth: u16) {
    let dib = depth != 0;
    use rrrah_cache::{CacheLimits, ImageSwapConfig, RasterRamCache, RasterSwapCache};
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
    use std::{sync::Arc, time::Duration};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!(
        "{} pages adapter: {}",
        if depth == 24 {
            "CUR DIB"
        } else if dib {
            "CUR DIB depths"
        } else if cursor {
            "CUR"
        } else {
            "ICO"
        },
        gpu.adapter_name()
    );
    let root = MemoryBudget::new(1024 * 1024);
    let queue = MemoryBudget::new(256);
    let directory = tempfile::tempdir().unwrap();
    let mut ram = RasterRamCache::new(CacheLimits {
        max_bytes: 64,
        max_entries: Some(1),
        ttl: None,
    });
    ram.enable_swap(
        RasterSwapCache::new_with_budgets(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(65536),
                queue_bytes: 256,
                queue_count: 1,
                restore_bytes: 256,
            },
            queue.clone(),
            root.clone(),
        )
        .unwrap(),
    );
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/data")
        .join(if depth == 24 {
            "cur-selected-dib.cur".to_owned()
        } else if dib {
            format!("cur-selected-dib-{depth}.cur")
        } else if cursor {
            "cur-selected-images.cur".to_owned()
        } else {
            "ico-selected-images.ico".to_owned()
        });
    let view = rrrah_gpu::ViewParameters {
        viewport: [63., 65.],
        ..Default::default()
    };
    let mut expected = Vec::new();
    let images: Vec<(usize, u32, u32, [u8; 4], Option<(u32, u32)>)> = if depth == 32 {
        vec![
            (0, 2, 2, [255, 0, 0, 128], Some((0, 1))),
            (1, 2, 2, [0, 255, 0, 64], Some((1, 0))),
            (2, 1, 1, [0, 0, 255, 255], Some((0, 0))),
        ]
    } else if dib {
        vec![
            (0, 2, 2, [255, 0, 0, 255], Some((0, 1))),
            (1, 2, 2, [0, 255, 0, 255], Some((1, 0))),
            (2, 1, 1, [0, 0, 255, 0], Some((0, 0))),
        ]
    } else if cursor {
        vec![
            (0, 2, 2, [255, 0, 0, 128], Some((0, 1))),
            (1, 2, 2, [0, 255, 0, 64], Some((1, 0))),
            (2, 1, 1, [0, 0, 255, 255], Some((0, 0))),
        ]
    } else {
        vec![
            (0, 2, 2, [255, 0, 0, 128], None),
            (1, 1, 3, [0, 0, 255, 255], None),
            (2, 3, 1, [0, 255, 0, 64], None),
        ]
    };
    for (index, width, height, rgba, hotspot) in images {
        assert!(ram.set_limits_and_spill(CacheLimits {
            max_bytes: 64,
            max_entries: Some(1),
            ttl: None
        }));
        let mut request = rrrah_decode::DecodeRequest::new(&path);
        request.image_index = index;
        request.memory_budget = Some(root.clone());
        let native = rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!(
            (
                native.width(),
                native.height(),
                native.image_index(),
                native.image_count()
            ),
            (width, height, index, 3)
        );
        assert_eq!(native.hotspot(), hotspot);
        let RasterPixels::Rgba8(pixels) = native.pixels() else {
            panic!()
        };
        assert!(pixels.chunks_exact(4).all(|pixel| pixel == rgba));
        let frame = rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&root)).unwrap();
        drop(native);
        let reference = DecodedRaster::new(
            width,
            height,
            RasterPixels::Rgba8(Arc::new(rgba.repeat((width * height) as usize)).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .with_hotspot(hotspot)
        .unwrap()
        .to_linear_srgb()
        .unwrap();
        let sdr = gpu.render_raster(&reference, view, [63, 65]).pixels;
        let hdr = gpu.render_raster_half(&reference, view, [63, 65]);
        assert_eq!(gpu.render_raster(&frame, view, [63, 65]).pixels, sdr);
        assert_eq!(gpu.render_raster_half(&frame, view, [63, 65]), hdr);
        assert!(ram.insert_visible(index, frame));
        let pressure = root.try_reserve(root.available_bytes()).unwrap();
        let hit = ram.get(&index).unwrap();
        assert_eq!((hit.image_index(), hit.image_count()), (index, 3));
        drop(pressure);
        assert!(ram.set_limits_and_spill(CacheLimits {
            max_bytes: 64,
            max_entries: Some(1),
            ttl: Some(Duration::ZERO)
        }));
        assert!(ram.insert_visible(index, hit));
        assert_eq!(ram.spill_expired(), 0);
        ram.mark_visible(99);
        assert_eq!(ram.spill_expired(), 1);
        ram.swap().unwrap().wait_idle().unwrap();
        assert_eq!((root.used(), queue.used()), (0, 0));
        expected.push((sdr, hdr, hotspot));
    }
    assert_ne!(expected[0], expected[1]);
    assert_ne!(expected[1], expected[2]);
    for index in [2usize, 0, 1, 2] {
        let frame = ram.get(&index).unwrap();
        assert_eq!((frame.image_index(), frame.image_count()), (index, 3));
        assert_eq!(frame.hotspot(), expected[index].2);
        assert_eq!(
            gpu.render_raster(&frame, view, [63, 65]).pixels,
            expected[index].0
        );
        assert_eq!(gpu.render_raster_half(&frame, view, [63, 65]), expected[index].1);
        drop(frame);
        ram.mark_visible(99);
        ram.spill_expired();
        ram.swap().unwrap().wait_idle().unwrap();
        assert_eq!((root.used(), queue.used()), (0, 0));
    }
    assert_eq!(
        (
            ram.swap().unwrap().stats().writes,
            ram.swap().unwrap().stats().reads,
            ram.swap().unwrap().stats().errors
        ),
        (3, 4, 0)
    );
    if dib && depth != 24 {
        eprintln!(
            "CUR DIB depth{depth}: palette/alpha/mask; matching hotspots; RAM TTL reverse swap; SDR/HDR exact; final=0"
        );
    } else if dib {
        eprintln!(
            "CUR DIB pages: padded24bit AND masks; matching hotspots; RAM TTL reverse swap; SDR/HDR exact; final=0"
        );
    } else if cursor {
        eprintln!(
            "CUR pages: quality order3; matching hotspots; exact RGBA alpha; count1/64byte RAM; full-pressure hit; visible TTL; reverse swap; SDR/HDR exact; final=0"
        );
    } else {
        eprintln!(
            "ICO pages: quality order3; exact RGBA alpha; count1/64byte RAM; full-pressure hit; visible TTL; reverse swap; SDR/HDR exact; final=0"
        );
    }
}

#[test]
#[ignore = "actual Metal mixed packed palette and per-pixel mask qualification"]
fn patterned_cursor_dib_pixels_survive_swap_and_metal() {
    use rrrah_cache::{CacheLimits, ImageSwapConfig, RasterRamCache, RasterSwapCache};
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
    use std::{sync::Arc, time::Duration};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("CUR patterned adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/data");
    for depth in [1, 4, 8, 16, 24, 32] {
        let root = MemoryBudget::new(1024 * 1024);
        let queue = MemoryBudget::new(4096);
        let directory = tempfile::tempdir().unwrap();
        let mut ram = RasterRamCache::new(CacheLimits {
            max_bytes: 1024,
            max_entries: Some(1),
            ttl: None,
        });
        ram.enable_swap(
            RasterSwapCache::new_with_budgets(
                directory.path(),
                ImageSwapConfig {
                    limits: CacheLimits::bytes(65536),
                    queue_bytes: 4096,
                    queue_count: 1,
                    restore_bytes: 4096,
                },
                queue.clone(),
                root.clone(),
            )
            .unwrap(),
        );
        let path = fixtures.join(format!("cur-dib-pattern-{depth}.cur"));
        let pixels = std::fs::read(path.with_extension("rgba8")).unwrap();
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.memory_budget = Some(root.clone());
        let native = rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!(
            (native.width(), native.height(), native.hotspot()),
            (9, 3, Some((8, 2)))
        );
        let RasterPixels::Rgba8(actual) = native.pixels() else {
            panic!()
        };
        assert_eq!(actual.as_ref(), pixels.as_slice(), "depth{depth}");
        let frame = rrrah_decode::prepare_raster_for_display_with_budget(&native, Some(&root)).unwrap();
        drop(native);
        let reference = DecodedRaster::new(
            9,
            3,
            RasterPixels::Rgba8(Arc::new(pixels).into()),
            RasterColorSpace::Srgb,
        )
        .unwrap()
        .to_linear_srgb()
        .unwrap();
        let view = rrrah_gpu::ViewParameters {
            viewport: [63., 65.],
            ..Default::default()
        };
        let sdr = gpu.render_raster(&reference, view, [63, 65]).pixels;
        let hdr = gpu.render_raster_half(&reference, view, [63, 65]);
        assert_eq!(gpu.render_raster(&frame, view, [63, 65]).pixels, sdr);
        assert_eq!(gpu.render_raster_half(&frame, view, [63, 65]), hdr);
        assert!(ram.insert_visible(0usize, frame));
        let pressure = root.try_reserve(root.available_bytes()).unwrap();
        let hit = ram.get(&0).unwrap();
        drop(pressure);
        assert!(ram.set_limits_and_spill(CacheLimits {
            max_bytes: 1024,
            max_entries: Some(1),
            ttl: Some(Duration::ZERO)
        }));
        assert!(ram.insert_visible(0, hit));
        ram.mark_visible(1);
        assert_eq!(ram.spill_expired(), 1);
        ram.swap().unwrap().wait_idle().unwrap();
        assert_eq!((root.used(), queue.used()), (0, 0));
        let restored = ram.get(&0).unwrap();
        assert_eq!(restored.hotspot(), Some((8, 2)));
        assert_eq!(gpu.render_raster(&restored, view, [63, 65]).pixels, sdr);
        assert_eq!(gpu.render_raster_half(&restored, view, [63, 65]), hdr);
        drop(restored);
        ram.mark_visible(1);
        ram.spill_expired();
        ram.swap().unwrap().wait_idle().unwrap();
        drop(ram);
        assert_eq!((root.used(), queue.used()), (0, 0));
        eprintln!(
            "CUR patterned depth{depth}: 9x3 packed rows/mixed mask/alpha; native oracle and SDR/HDR swap exact; final=0"
        );
    }
}
