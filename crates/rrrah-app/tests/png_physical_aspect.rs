#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "manual PNG physical aspect Metal SDR/HDR and swap qualification"]
fn png_physical_aspect_survives_swap_and_matches_metal_geometry() {
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("PNG aspect adapter: {}", gpu.adapter_name());
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/data/png-physical-aspect.png");
    let root = MemoryBudget::new(1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(path);
    request.memory_budget = Some(root.clone());
    let raster = {
        let decoded = rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!(decoded.pixel_aspect(), Some(0.5));
        decoded.to_linear_srgb_with_budget(Some(&root)).unwrap()
    };
    assert_eq!(raster.pixel_aspect(), Some(0.5));
    let reference = DecodedRaster::new(
        4,
        4,
        RasterPixels::Rgba8(std::sync::Arc::new(vec![255u8; 64]).into()),
        RasterColorSpace::Srgb,
    )
    .unwrap()
    .with_pixel_aspect(Some(0.5))
    .unwrap()
    .to_linear_srgb()
    .unwrap();
    let view = rrrah_gpu::ViewParameters {
        viewport: [64., 64.],
        ..Default::default()
    };
    let sdr = gpu.render_raster(&reference, view, [64, 64]).pixels;
    let hdr = gpu.render_raster_half(&reference, view, [64, 64]);
    let square = reference.clone().with_pixel_aspect(None).unwrap();
    assert_ne!(sdr, gpu.render_raster(&square, view, [64, 64]).pixels);
    assert!(sdr[(32 * 64 + 32) * 4] > 200);
    assert!(sdr[(32 * 64) * 4] < 100);
    assert_eq!(gpu.render_raster(&raster, view, [64, 64]).pixels, sdr);
    assert_eq!(gpu.render_raster_half(&raster, view, [64, 64]), hdr);
    let dir = tempfile::tempdir().unwrap();
    let queue = MemoryBudget::new(256);
    let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
        dir.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits::bytes(4096),
            queue_bytes: 256,
            queue_count: 1,
            restore_bytes: 256,
        },
        queue.clone(),
        root.clone(),
    )
    .unwrap();
    assert_eq!(swap.try_enqueue(1, raster), rrrah_cache::SpillAdmission::Queued);
    swap.wait_idle().unwrap();
    assert_eq!(root.used(), 0);
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!(restored.pixel_aspect(), Some(0.5));
    assert_eq!(gpu.render_raster(&restored, view, [64, 64]).pixels, sdr);
    assert_eq!(gpu.render_raster_half(&restored, view, [64, 64]), hdr);
    drop(restored);
    assert_eq!((root.used(), queue.used()), (0, 0));
    assert_eq!(
        (swap.stats().writes, swap.stats().reads, swap.stats().errors),
        (1, 1, 0)
    );
    eprintln!("PNG physical aspect: width/height=0.5; decoded and swapped SDR/HDR frames exact; final=0");
}

#[test]
#[ignore = "manual APNG selected-frame physical aspect Metal and swap qualification"]
fn apng_selected_frames_preserve_physical_aspect_through_swap_and_metal() {
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("APNG aspect adapter: {}", gpu.adapter_name());
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/data/apng-physical-aspect.png");
    let root = MemoryBudget::new(1024 * 1024);
    let queue = MemoryBudget::new(256);
    let dir = tempfile::tempdir().unwrap();
    let swap: rrrah_cache::RasterSwapCache<usize> = rrrah_cache::ImageSwapCache::new_with_budgets(
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
        queue.clone(),
        root.clone(),
    )
    .unwrap();
    let view = rrrah_gpu::ViewParameters {
        viewport: [64., 64.],
        ..Default::default()
    };
    let mut expected = Vec::new();
    for (index, pixel) in [[1f32; 4], [0., 1., 0., 1.]].into_iter().enumerate() {
        let mut request = rrrah_decode::DecodeRequest::new(&path);
        request.image_index = index;
        request.memory_budget = Some(root.clone());
        let raster = rrrah_decode::decode_raster(&request).unwrap();
        assert_eq!(raster.pixel_aspect(), Some(0.5));
        assert_eq!((raster.image_index(), raster.image_count()), (index, 2));
        let reference = DecodedRaster::new(
            4,
            4,
            RasterPixels::Rgba32Float(std::sync::Arc::new(pixel.repeat(16)).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .with_pixel_aspect(Some(0.5))
        .unwrap();
        let sdr = gpu.render_raster(&reference, view, [64, 64]).pixels;
        let hdr = gpu.render_raster_half(&reference, view, [64, 64]);
        assert_eq!(gpu.render_raster(&raster, view, [64, 64]).pixels, sdr);
        assert_eq!(gpu.render_raster_half(&raster, view, [64, 64]), hdr);
        assert_eq!(
            swap.try_enqueue(index, raster),
            rrrah_cache::SpillAdmission::Queued
        );
        swap.wait_idle().unwrap();
        assert_eq!((root.used(), queue.used()), (0, 0));
        expected.push((sdr, hdr));
    }
    assert_ne!(expected[0], expected[1]);
    for index in [1usize, 0] {
        let restored = swap.try_get(&index, || false).unwrap().unwrap();
        assert_eq!(restored.pixel_aspect(), Some(0.5));
        assert_eq!((restored.image_index(), restored.image_count()), (index, 2));
        assert_eq!(
            gpu.render_raster(&restored, view, [64, 64]).pixels,
            expected[index].0
        );
        assert_eq!(
            gpu.render_raster_half(&restored, view, [64, 64]),
            expected[index].1
        );
    }
    assert_eq!((root.used(), queue.used()), (0, 0));
    assert_eq!(
        (swap.stats().writes, swap.stats().reads, swap.stats().errors),
        (2, 2, 0)
    );
    eprintln!("APNG physical aspect: two selected frames; reverse swap restore; SDR/HDR exact; final=0");
}

#[test]
#[ignore = "manual 16-bit APNG precision Metal SDR/HDR qualification"]
fn apng_sixteen_bit_precision_matches_authored_linear_metal_reference() {
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("APNG precision adapter: {}", gpu.adapter_name());
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/data/apng-16bit-precision.png");
    let root = MemoryBudget::new(1024 * 1024);
    for (index, samples) in [[257u16, 12345, 32769, 65534], [258, 12346, 32770, 65533]]
        .into_iter()
        .enumerate()
    {
        let mut request = rrrah_decode::DecodeRequest::new(&path);
        request.image_index = index;
        request.memory_budget = Some(root.clone());
        let decoded = rrrah_decode::decode_raster(&request).unwrap();
        let linear: Vec<f32> = samples
            .into_iter()
            .enumerate()
            .map(|(channel, sample)| {
                let x = f64::from(sample) / 65535.;
                (if channel == 3 {
                    x
                } else if x <= 0.04045 {
                    x / 12.92
                } else {
                    ((x + 0.055) / 1.055).powf(2.4)
                }) as f32
            })
            .collect();
        let reference = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(std::sync::Arc::new(linear).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let view = rrrah_gpu::ViewParameters {
            viewport: [64., 64.],
            ..Default::default()
        };
        assert_eq!(
            gpu.render_raster(&decoded, view, [64, 64]).pixels,
            gpu.render_raster(&reference, view, [64, 64]).pixels
        );
        assert_eq!(
            gpu.render_raster_half(&decoded, view, [64, 64]),
            gpu.render_raster_half(&reference, view, [64, 64])
        );
        drop(decoded);
        assert_eq!(root.used(), 0);
    }
    eprintln!("APNG 16-bit precision: two selected frames; authored linear SDR/HDR reference exact; final=0");
}

#[test]
#[ignore = "manual packed APNG grayscale/palette Metal SDR/HDR and swap qualification"]
fn apng_packed_samples_and_palette_alpha_survive_metal_and_swap() {
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("APNG packed adapter: {}", gpu.adapter_name());
    let root = MemoryBudget::new(1024 * 1024);
    let queue = MemoryBudget::new(64);
    let dir = tempfile::tempdir().unwrap();
    let swap: rrrah_cache::RasterSwapCache<usize> = rrrah_cache::ImageSwapCache::new_with_budgets(
        dir.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits::bytes(65536),
            queue_bytes: 64,
            queue_count: 1,
            restore_bytes: 64,
        },
        queue.clone(),
        root.clone(),
    )
    .unwrap();
    let cases = [
        (
            1,
            [
                [0u8, 0, 0, 255],
                [255, 255, 255, 255],
                [0, 0, 0, 255],
                [255, 255, 255, 255],
            ],
        ),
        (
            2,
            [
                [255, 0, 0, 0],
                [0, 255, 0, 64],
                [0, 0, 255, 128],
                [255, 255, 255, 255],
            ],
        ),
        (
            4,
            [
                [0, 0, 0, 255],
                [85, 85, 85, 255],
                [170, 170, 170, 255],
                [255, 255, 255, 255],
            ],
        ),
    ];
    let view = rrrah_gpu::ViewParameters {
        viewport: [64., 64.],
        ..Default::default()
    };
    for (case, (depth, samples)) in cases.into_iter().enumerate() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/data/apng-{depth}bit-packed.png"));
        for index in 0..2 {
            let mut request = rrrah_decode::DecodeRequest::new(&path);
            request.image_index = index;
            request.memory_budget = Some(root.clone());
            let decoded = rrrah_decode::decode_raster(&request).unwrap();
            let mut linear = Vec::new();
            for x in 0..4 {
                for (channel, sample) in samples[if index == 0 { x } else { 3 - x }]
                    .into_iter()
                    .enumerate()
                {
                    let e = f64::from(sample) / 255.;
                    linear.push(
                        (if channel == 3 {
                            e
                        } else if e <= 0.04045 {
                            e / 12.92
                        } else {
                            ((e + 0.055) / 1.055).powf(2.4)
                        }) as f32,
                    );
                }
            }
            let reference = DecodedRaster::new(
                4,
                1,
                RasterPixels::Rgba32Float(std::sync::Arc::new(linear).into()),
                RasterColorSpace::LinearSrgb,
            )
            .unwrap();
            let sdr = gpu.render_raster(&reference, view, [64, 64]).pixels;
            let hdr = gpu.render_raster_half(&reference, view, [64, 64]);
            assert_eq!(gpu.render_raster(&decoded, view, [64, 64]).pixels, sdr);
            assert_eq!(gpu.render_raster_half(&decoded, view, [64, 64]), hdr);
            let key = case * 2 + index;
            assert_eq!(
                swap.try_enqueue(key, decoded),
                rrrah_cache::SpillAdmission::Queued
            );
            swap.wait_idle().unwrap();
            assert_eq!(root.used(), 0);
            let restored = swap.try_get(&key, || false).unwrap().unwrap();
            assert_eq!((restored.image_index(), restored.image_count()), (index, 2));
            assert_eq!(gpu.render_raster(&restored, view, [64, 64]).pixels, sdr);
            assert_eq!(gpu.render_raster_half(&restored, view, [64, 64]), hdr);
            drop(restored);
            assert_eq!((root.used(), queue.used()), (0, 0));
        }
    }
    assert_eq!(
        (swap.stats().writes, swap.stats().reads, swap.stats().errors),
        (6, 6, 0)
    );
    eprintln!(
        "APNG packed precision: six selected frames; grayscale/palette alpha; SDR/HDR and swap exact; final=0"
    );
}

#[test]
#[ignore = "manual APNG PREVIOUS patch composition Metal SDR/HDR qualification"]
fn apng_previous_rectangle_matches_independent_composition_frames_on_metal() {
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("APNG previous adapter: {}", gpu.adapter_name());
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/raster");
    let root = MemoryBudget::new(1024 * 1024);
    let view = rrrah_gpu::ViewParameters {
        viewport: [64., 64.],
        ..Default::default()
    };
    for index in [3usize, 1, 2] {
        let mut request = rrrah_decode::DecodeRequest::new(fixtures.join("apng-compose-0-0.apng"));
        request.image_index = index;
        request.memory_budget = Some(root.clone());
        let decoded = rrrah_decode::decode_raster(&request).unwrap();
        let bytes =
            std::fs::read(fixtures.join(format!("apng-compose-0-0.apng.frame{index}.rgba32f"))).unwrap();
        let values: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let reference = DecodedRaster::new(
            5,
            3,
            RasterPixels::Rgba32Float(std::sync::Arc::new(values).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        assert_eq!(
            gpu.render_raster(&decoded, view, [64, 64]).pixels,
            gpu.render_raster(&reference, view, [64, 64]).pixels
        );
        assert_eq!(
            gpu.render_raster_half(&decoded, view, [64, 64]),
            gpu.render_raster_half(&reference, view, [64, 64])
        );
        drop(decoded);
        assert_eq!(root.used(), 0);
    }
    eprintln!(
        "APNG previous rectangle: three selected disposal frames; independent SDR/HDR oracle exact; final=0"
    );
}
