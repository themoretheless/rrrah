#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
use rrrah_decode::*;

fn exercise_lifecycle() -> (DecodedRaster, MemoryBudget) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native.eps");
    std::fs::write(&path,b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 4 4\n%%EndComments\n1 0 0 setrgbcolor 0 0 moveto 4 0 lineto 4 4 lineto 0 4 lineto closepath fill 0 0 1 setrgbcolor 2 setlinewidth 0 2 moveto 4 2 lineto stroke\n").unwrap();
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
    let (raster, root) = exercise_lifecycle();
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
