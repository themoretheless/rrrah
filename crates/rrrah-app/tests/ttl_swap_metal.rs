#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_cache::{CacheLimits, ImageSwapConfig, MemoryBudget};
use std::time::Duration;

#[test]
fn expired_hdr_lease_and_swap_restore_preserve_metal_frame_and_float_bits() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("TTL HDR adapter: {}", gpu.adapter_name());
    let budget = MemoryBudget::new(4096);
    let samples = vec![-0.0f32, 2.0, 0.5, 1.0, 8.0, 0.25, -0.5, 1.0];
    let expected_bits: Vec<_> = samples.iter().map(|v| v.to_bits()).collect();
    let raster = rrrah_core::DecodedRaster::new(
        2,
        1,
        rrrah_core::RasterPixels::Rgba32Float(std::sync::Arc::new(samples).into()),
        rrrah_core::RasterColorSpace::LinearSrgb,
    )
    .unwrap()
    .try_manage_pixels(&budget)
    .unwrap();
    let params = rrrah_gpu::ViewParameters {
        viewport: [96., 64.],
        zoom: 1.,
        ..Default::default()
    };
    let expected = gpu.render_raster(&raster, params, [96, 64]).pixels;
    let weight = raster.capacity_bytes();
    let directory = tempfile::tempdir().unwrap();
    let mut ram = rrrah_cache::RasterRamCache::new(CacheLimits {
        max_bytes: weight,
        max_entries: Some(1),
        ttl: Some(Duration::from_secs(1)),
    });
    ram.enable_swap(
        rrrah_cache::RasterSwapCache::new_with_budgets(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(4096),
                queue_bytes: weight,
                queue_count: 1,
                restore_bytes: weight,
            },
            MemoryBudget::new(weight),
            budget.clone(),
        )
        .unwrap(),
    );
    assert!(ram.insert(1u8, raster));
    let lease = ram.get_lease(&1).unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    assert!(ram.get_lease(&1).is_none());
    assert_eq!(ram.spill_expired(), 0);
    assert_eq!(ram.swap().unwrap().stats().writes, 0);
    assert_eq!(gpu.render_raster(&lease, params, [96, 64]).pixels, expected);
    drop(lease);
    assert_eq!(ram.spill_expired(), 1);
    ram.swap().unwrap().wait_idle().unwrap();
    assert!(ram.is_empty());
    assert_eq!(budget.used(), 0);
    let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
    assert!(ram.get(&1).is_none());
    assert_eq!(ram.swap().unwrap().stats().errors, 0);
    drop(pressure);
    let restored = ram.get(&1).unwrap();
    let rrrah_core::RasterPixels::Rgba32Float(values) = restored.pixels() else {
        panic!()
    };
    assert_eq!(
        values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected_bits
    );
    assert_eq!(gpu.render_raster(&restored, params, [96, 64]).pixels, expected);
    drop(restored);
    drop(ram);
    assert_eq!(budget.used(), 0);
}

#[test]
fn expired_real_cr3_lease_and_swap_restore_preserve_metal_frame() {
    raw_ttl_swap_frame(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3"), None);
}

#[test]
#[ignore = "requires authored EIP and actual Metal GPU"]
fn expired_eip_lease_and_swap_restore_match_original_cr3_metal_frame() {
    raw_ttl_swap_frame(std::path::PathBuf::from(std::env::var("RRRAH_EIP_APP_FIRST").unwrap()), Some(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/IMG_9043.CR3")));
}

fn raw_ttl_swap_frame(source: std::path::PathBuf, reference: Option<std::path::PathBuf>) {
    use rrrah_decode::RawDecoder;
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("TTL RAW adapter: {}", gpu.adapter_name());
    let budget = MemoryBudget::new(256 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(source);
    request.memory_budget = Some(budget.clone());
    let native = rrrah_decode::NativeRawDecoder.decode(&request).unwrap().mosaic;
    let metadata = native.metadata.clone();
    let expected_samples = native.pixels.to_vec();
    let expected = gpu.render(&native, [128, 96]).pixels;
    if let Some(path) = reference {
        let mut original_request = rrrah_decode::DecodeRequest::new(path);
        original_request.memory_budget = Some(budget.clone());
        let original = rrrah_decode::NativeRawDecoder.decode(&original_request).unwrap().mosaic;
        assert_eq!(native.metadata, original.metadata);
        assert_eq!(native.pixels, original.pixels);
        assert_eq!(expected, gpu.render(&original, [128, 96]).pixels);
        drop(original);
    }
    let weight = native.pixels.capacity_bytes();
    let key = rrrah_cache::CacheKey::for_mosaic_recipe(
        &rrrah_cache::SourceFingerprint::from_path(&request.path).unwrap(),
        0,
        rrrah_decode::NativeRawDecoder.mosaic_recipe(&request).unwrap(),
    );
    let directory = tempfile::tempdir().unwrap();
    let mut ram = rrrah_cache::MosaicRamCache::with_limits(CacheLimits {
        max_bytes: weight,
        max_entries: Some(1),
        ttl: Some(Duration::from_secs(1)),
    });
    ram.enable_swap(
        rrrah_cache::MosaicSwapCache::new_with_budgets(
            directory.path(),
            rrrah_cache::MosaicSwapConfig {
                limits: CacheLimits::bytes(weight + 65536),
                queue_bytes: weight,
                queue_count: 1,
                restore_bytes: weight,
            },
            MemoryBudget::new(weight),
            budget.clone(),
        )
        .unwrap(),
    );
    assert!(ram.insert(key, native));
    let lease = ram.get_lease(&key).unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    assert!(ram.get_lease(&key).is_none());
    assert_eq!(ram.spill_expired(), 0);
    assert_eq!(gpu.render(&lease, [128, 96]).pixels, expected);
    drop(lease);
    assert_eq!(ram.spill_expired(), 1);
    ram.swap().unwrap().wait_idle().unwrap();
    assert_eq!(budget.used(), 0);
    let pressure = budget.try_reserve(budget.available_bytes()).unwrap();
    assert!(ram.get(&key).is_none());
    assert_eq!(ram.swap().unwrap().stats().errors, 0);
    drop(pressure);
    let restored = ram.get(&key).unwrap();
    assert_eq!(restored.metadata, metadata);
    assert_eq!(&*restored.pixels, expected_samples.as_slice());
    assert_eq!(gpu.render(&restored, [128, 96]).pixels, expected);
    assert_eq!(ram.swap().unwrap().stats().writes, 1);
    assert_eq!(ram.swap().unwrap().stats().reads, 1);
    eprintln!("RAW TTL/swap: {} samples and 128x96 Metal frame exact; one write/read; pressure refusal recovers; final=0", expected_samples.len());
    drop(restored);
    drop(ram);
    assert_eq!(budget.used(), 0);
}

#[test]
#[ignore = "manual hardware qualification"]
fn live_count_size_and_ttl_changes_preserve_hdr_frames_through_swap() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    let cycles = std::env::var("RRRAH_METAL_POLICY_CYCLES")
        .map(|value| value.parse::<usize>().expect("valid cycle count")).unwrap_or(1);
    assert!((1..=10000).contains(&cycles));
    eprintln!("live policy adapter: {}; cycles={cycles}", gpu.adapter_name());
    for _ in 0..cycles { qualify_live_policy_cycle(&gpu, 2, 1, [96, 64]); }
    eprintln!("live policy cycles={cycles}: exact Metal frames and float bits; final managed bytes=0");
}

#[test]
#[ignore = "manual 12 MP Metal and disk-swap qualification"]
fn large_hdr_live_policy_preserves_every_component_and_metal_pixel() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("large live policy adapter: {}; source=4000x3000", gpu.adapter_name());
    qualify_live_policy_cycle(&gpu, 4000, 3000, [4000, 3000]);
    eprintln!("12 MP live policy: every float bit and Metal pixel exact; final managed bytes=0");
}

#[test]
#[ignore = "manual sustained 12 MP Metal and disk-swap qualification"]
fn sustained_large_hdr_live_policy_preserves_every_component_and_metal_pixel() {
    let gpu = common::qualification_gpu().expect("actual GPU required");
    let cycles = std::env::var("RRRAH_METAL_LARGE_POLICY_CYCLES")
        .map(|value| value.parse::<usize>().expect("valid large cycle count")).unwrap_or(30);
    assert!((1..=1000).contains(&cycles));
    let started = std::time::Instant::now();
    eprintln!("sustained large policy adapter: {}; cycles={cycles}; source=4000x3000", gpu.adapter_name());
    for cycle in 0..cycles {
        qualify_live_policy_cycle_seeded(&gpu, 4000, 3000, [4000, 3000], cycle);
        eprintln!("large policy cycle={}/{cycles}; elapsed={:.3}s; final managed bytes=0", cycle + 1, started.elapsed().as_secs_f64());
    }
}

fn qualify_live_policy_cycle(gpu: &common::GpuReadback, width: u32, height: u32, output: [u32; 2]) {
    qualify_live_policy_cycle_seeded(gpu, width, height, output, 0);
}

fn qualify_live_policy_cycle_seeded(gpu: &common::GpuReadback, width: u32, height: u32, output: [u32; 2], seed: usize) {
    use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
    let pixels = width as usize * height as usize;
    let payload_bytes = u64::try_from(pixels * 4 * std::mem::size_of::<f32>()).unwrap();
    let root = MemoryBudget::new(payload_bytes * 4);
    let make = |red: f32| {
        let mut values = Vec::with_capacity(pixels * 4);
        for index in 0..pixels {
            if pixels == 2 {
                let pixel = if index == 0 {
                    [red, -0.0, 0.5, 1.0]
                } else {
                    [8.0, 0.25, -0.5, 1.0]
                };
                values.extend_from_slice(&pixel);
            } else {
                // Spatially distinct binary fractions exercise every retained component.
                let x = ((index % width as usize + seed * 31) % width as usize) as f32;
                let y = ((index / width as usize + seed * 17) % height as usize) as f32;
                values.extend_from_slice(&[
                    red + x / 4096.0,
                    if index % 17 == 0 { -0.0 } else { y / 4096.0 },
                    -0.5 + (index % 1024) as f32 / 256.0,
                    (index % 255 + 1) as f32 / 256.0,
                ]);
            }
        }
        DecodedRaster::new(
            width, height,
            RasterPixels::Rgba32Float(std::sync::Arc::new(values).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap()
        .try_manage_pixels(&root)
        .unwrap()
    };
    let params = rrrah_gpu::ViewParameters {
        viewport: [output[0] as f32, output[1] as f32],
        ..Default::default()
    };
    let directory = tempfile::tempdir().unwrap();
    let mut ram = rrrah_cache::RasterRamCache::new(CacheLimits {
        max_bytes: payload_bytes * 2,
        max_entries: Some(2),
        ttl: None,
    });
    ram.enable_swap(
        rrrah_cache::RasterSwapCache::new_with_budgets(
            directory.path(),
            ImageSwapConfig {
                limits: CacheLimits::bytes(payload_bytes * 4 + 4096),
                queue_bytes: payload_bytes * 2,
                queue_count: 2,
                restore_bytes: payload_bytes,
            },
            MemoryBudget::new(payload_bytes * 2),
            root.clone(),
        )
        .unwrap(),
    );
    let first = make(2.0);
    let first_bits = match first.pixels() {
        RasterPixels::Rgba32Float(values) => values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        _ => panic!(),
    };
    let expected = gpu.render_raster(&first, params, output).pixels;
    let expected_hdr = gpu.render_raster_half(&first, params, output);
    assert!(ram.insert(1u8, first));
    let lease = ram.get_lease(&1).unwrap();
    assert!(ram.insert_visible(2, make(4.0)));
    let tight = CacheLimits {
        max_bytes: payload_bytes,
        max_entries: Some(1),
        ttl: Some(Duration::ZERO),
    };
    assert!(!ram.set_limits_and_spill(tight));
    assert_eq!((ram.len(), ram.resident_bytes(), root.used()), (2, payload_bytes * 2, payload_bytes * 2));
    assert!(ram.get(&1).is_some());
    assert_eq!(gpu.render_raster(&lease, params, output).pixels, expected);
    assert_eq!(gpu.render_raster_half(&lease, params, output), expected_hdr);
    drop(lease);
    assert!(ram.set_limits_and_spill(tight));
    ram.swap().unwrap().wait_idle().unwrap();
    assert_eq!((ram.len(), root.used()), (1, payload_bytes));
    let pressure = root.try_reserve(root.available_bytes()).unwrap();
    assert!(ram.get_background_with_cancel(&1, || false).is_none());
    assert_eq!(ram.swap().unwrap().stats().errors, 0);
    drop(pressure);
    assert!(ram.get_background_with_cancel(&1, || true).is_none());
    assert_eq!(root.used(), payload_bytes);
    let restored = ram.get_background_with_cancel(&1, || false).unwrap();
    assert_eq!(ram.len(), 1, "background restore cannot displace visible frame");
    let RasterPixels::Rgba32Float(values) = restored.pixels() else {
        panic!()
    };
    assert_eq!(
        values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        first_bits,
        "swap preserves every HDR component, including signed zero and negative values"
    );
    assert_eq!(gpu.render_raster(&restored, params, output).pixels, expected);
    assert_eq!(gpu.render_raster_half(&restored, params, output), expected_hdr);
    drop(restored);
    assert_eq!(root.used(), payload_bytes);
    let replacement = make(16.0);
    let replacement_bits = match replacement.pixels() {
        RasterPixels::Rgba32Float(v) => v.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        _ => panic!(),
    };
    let replacement_frame = gpu.render_raster(&replacement, params, output).pixels;
    let replacement_hdr = gpu.render_raster_half(&replacement, params, output);
    assert!(ram.insert_visible(2, replacement.clone()));
    assert!(ram.get(&2).is_none(), "replacement takes the new zero TTL");
    assert_eq!(ram.spill_expired(), 0, "visible expired frame remains protected");
    ram.mark_visible(3);
    assert_eq!(ram.spill_expired(), 1);
    ram.swap().unwrap().wait_idle().unwrap();
    assert_eq!(root.used(), payload_bytes, "external display owner retains its reservation");
    assert_eq!(
        gpu.render_raster(&replacement, params, output).pixels,
        replacement_frame
    );
    drop(replacement);
    assert_eq!(root.used(), 0);
    let final_frame = ram.get(&2).unwrap();
    let RasterPixels::Rgba32Float(values) = final_frame.pixels() else {
        panic!()
    };
    assert_eq!(
        values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        replacement_bits
    );
    assert_eq!(
        gpu.render_raster(&final_frame, params, output).pixels,
        replacement_frame
    );
    assert_eq!(gpu.render_raster_half(&final_frame, params, output), replacement_hdr);
    drop(final_frame);
    drop(ram);
    assert_eq!(root.used(), 0);
}

#[test]
#[ignore = "manual long-lived 12 MP collection cache and Metal qualification"]
fn long_lived_large_collection_cache_preserves_swap_and_metal_frames() {
    use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
    const WIDTH: u32 = 4000;
    const HEIGHT: u32 = 3000;
    const COUNT: usize = 8;
    const BYTES: u64 = WIDTH as u64 * HEIGHT as u64 * 16;
    let seconds = std::env::var("RRRAH_METAL_COLLECTION_SECONDS")
        .map(|v| v.parse::<u64>().expect("valid duration")).unwrap_or(60);
    assert!((10..=86400).contains(&seconds));
    let hdr_output = if std::env::var("RRRAH_METAL_COLLECTION_FULL_HDR").as_deref() == Ok("1") {
        [WIDTH, HEIGHT]
    } else {
        [63, 65]
    };
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("long-lived collection adapter: {}; duration={seconds}s; frames={COUNT}; payload={BYTES}; hdr_output={hdr_output:?}", gpu.adapter_name());
    let root = MemoryBudget::new(BYTES * 3);
    let queue = MemoryBudget::new(BYTES);
    let directory = tempfile::tempdir().unwrap();
    let limits = |ttl| CacheLimits { max_bytes: BYTES, max_entries: Some(1), ttl };
    let mut ram = rrrah_cache::RasterRamCache::new(limits(Some(Duration::ZERO)));
    ram.enable_swap(rrrah_cache::RasterSwapCache::new_with_budgets(
        directory.path(), ImageSwapConfig {
            limits: CacheLimits::bytes(BYTES * COUNT as u64 + 1024 * 1024),
            queue_bytes: BYTES, queue_count: 1, restore_bytes: BYTES,
        }, queue.clone(), root.clone()).unwrap());
    let components = |key: usize, index: usize| [
        key as f32 / 8.0 + (index % WIDTH as usize) as f32 / 4096.0,
        if index % 17 == 0 { -0.0 } else { (index / WIDTH as usize) as f32 / 4096.0 },
        -0.5 + ((index + key * 31) % 1024) as f32 / 256.0,
        (index % 255 + 1) as f32 / 256.0,
    ];
    let params = rrrah_gpu::ViewParameters {
        viewport: [WIDTH as f32, HEIGHT as f32], ..Default::default()
    };
    // Pre-spill GPU frame references are outside the managed cache budget.
    let mut expected = Vec::new();
    let mut expected_half = Vec::new();
    for key in 0..COUNT {
        let mut data = Vec::with_capacity((BYTES / 4) as usize);
        for index in 0..(WIDTH as usize * HEIGHT as usize) {
            data.extend_from_slice(&components(key, index));
        }
        let raster = DecodedRaster::new(WIDTH, HEIGHT,
            RasterPixels::Rgba32Float(std::sync::Arc::new(data).into()),
            RasterColorSpace::LinearSrgb).unwrap().try_manage_pixels(&root).unwrap();
        expected.push(gpu.render_raster(&raster, params, [WIDTH, HEIGHT]).pixels);
        expected_half.push(gpu.render_raster_half(&raster, params, hdr_output));
        assert!(ram.insert_visible(key, raster));
        ram.swap().unwrap().wait_idle().unwrap();
        assert_eq!(queue.used(), 0);
        assert_eq!(root.used(), BYTES);
    }
    ram.mark_visible(COUNT);
    assert!(ram.set_limits_and_spill(limits(Some(Duration::ZERO))));
    ram.spill_expired();
    ram.swap().unwrap().wait_idle().unwrap();
    assert_eq!(root.used(), 0);
    let started = std::time::Instant::now();
    let mut cycles = 0;
    let mut accelerated = 0;
    let mut mid_batch_cancelled = 0;
    let parallel_available = std::thread::available_parallelism().is_ok_and(|n| n.get() >= 2);
    while started.elapsed() < Duration::from_secs(seconds) || cycles < COUNT {
        let key = (cycles * 3) % COUNT;
        assert!(ram.set_limits_and_spill(limits(None)));
        let pressure = root.try_reserve(root.available_bytes()).unwrap();
        assert!(ram.get_background_with_cancel(&key, || false).is_none());
        drop(pressure);
        assert!(ram.get_background_with_cancel(&key, || true).is_none());
        assert_eq!(root.used(), 0);
        if parallel_available {
            let mut saw_scratch = false;
            assert!(ram.get_background_with_cancel(&key, || {
                saw_scratch |= root.used() > BYTES;
                saw_scratch
            }).is_none(), "mid-batch cancellation must not publish pixels");
            assert!(saw_scratch, "large-frame cancellation must exercise admitted scratch");
            assert_eq!(root.used(), 0);
            mid_batch_cancelled += 1;
        }
        let mut saw_scratch = false;
        let frame = ram.get_with_cancel(&key, || {
            saw_scratch |= root.used() > BYTES;
            false
        }).expect("previously persisted collection frame");
        if parallel_available {
            assert!(saw_scratch, "successful large restore must exercise parallel path");
            accelerated += 1;
        }
        let RasterPixels::Rgba32Float(values) = frame.pixels() else { panic!() };
        for (index, actual) in values.chunks_exact(4).enumerate() {
            let reference = components(key, index);
            for channel in 0..4 {
                assert_eq!(actual[channel].to_bits(), reference[channel].to_bits(),
                    "key={key} pixel={index} channel={channel}");
            }
        }
        assert_eq!(gpu.render_raster(&frame, params, [WIDTH, HEIGHT]).pixels, expected[key]);
        assert_eq!(gpu.render_raster_half(&frame, params, hdr_output), expected_half[key]);
        assert!(ram.set_limits_and_spill(limits(Some(Duration::ZERO))));
        // Policy TTL applies to new/replaced membership, not existing insertion timestamps.
        assert!(ram.insert_visible(key, frame.clone()));
        assert_eq!(ram.spill_expired(), 0, "visible frame remains protected");
        ram.mark_visible(COUNT);
        assert_eq!(ram.spill_expired(), 1);
        drop(frame);
        ram.swap().unwrap().wait_idle().unwrap();
        assert_eq!((root.used(), queue.used(), ram.resident_bytes()), (0, 0, 0));
        let stats = ram.swap().unwrap().stats();
        assert_eq!((stats.errors, stats.dropped, stats.queued_bytes), (0, 0, 0));
        cycles += 1;
        if cycles % COUNT == 0 {
            eprintln!("long-lived collection cycles={cycles}; elapsed={:.3}s; root=0; queue=0; reads={}; writes={}", started.elapsed().as_secs_f64(), stats.reads, stats.writes);
        }
    }
    let stats = ram.swap().unwrap().stats();
    assert_eq!(stats.reads, cycles as u64);
    assert_eq!(stats.writes, COUNT as u64, "unchanged persisted frames need no redundant writes");
    drop(ram);
    assert_eq!((root.used(), queue.used()), (0, 0));
    eprintln!("long-lived collection complete: cycles={cycles}; elapsed={:.3}s; reads={}; writes={}; root=0; queue=0", started.elapsed().as_secs_f64(), stats.reads, stats.writes);
    eprintln!("parallel collection: accelerated={accelerated}; mid_batch_cancelled={mid_batch_cancelled}; exact_sdr_full_frames={cycles}; exact_hdr_half_frames={cycles}; hdr_output={hdr_output:?}; root=0; queue=0");
}
