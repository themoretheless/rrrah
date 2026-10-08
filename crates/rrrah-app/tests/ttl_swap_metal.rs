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

fn qualify_live_policy_cycle(gpu: &common::GpuReadback, width: u32, height: u32, output: [u32; 2]) {
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
                let x = (index % width as usize) as f32;
                let y = (index / width as usize) as f32;
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
    drop(lease);
    assert!(ram.set_limits_and_spill(tight));
    ram.swap().unwrap().wait_idle().unwrap();
    assert_eq!((ram.len(), root.used()), (1, payload_bytes));
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
    drop(restored);
    assert_eq!(root.used(), payload_bytes);
    let replacement = make(16.0);
    let replacement_bits = match replacement.pixels() {
        RasterPixels::Rgba32Float(v) => v.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        _ => panic!(),
    };
    let replacement_frame = gpu.render_raster(&replacement, params, output).pixels;
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
    drop(final_frame);
    drop(ram);
    assert_eq!(root.used(), 0);
}
