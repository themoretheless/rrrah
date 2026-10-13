#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "manual 12MP APNG shared memory, swap and Metal SDR/HDR qualification"]
fn large_apng_admission_retry_swap_and_metal_preserve_all_samples() {
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("large APNG adapter: {}", gpu.adapter_name());
    let root = MemoryBudget::new(512 * 1024 * 1024);
    let mut request = rrrah_decode::DecodeRequest::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/data/apng-12mp-16bit.png"),
    );
    request.image_index = 1;
    let decode_budget = root.child(root.limit());
    request.memory_budget = Some(decode_budget.clone());
    let pressure = root.try_reserve(root.limit()).unwrap();
    assert!(rrrah_decode::decode_raster(&request).is_err());
    assert_eq!(root.used(), root.limit());
    drop(pressure);
    assert_eq!(root.used(), 0);
    let partial_pressure = root.try_reserve(128 * 1024 * 1024).unwrap();
    assert!(rrrah_decode::decode_raster(&request).is_err());
    assert_eq!(root.used(), 128 * 1024 * 1024);
    assert_eq!(decode_budget.used(), 0);
    drop(partial_pressure);
    let decoded = rrrah_decode::decode_raster(&request).expect("12MP output fits admitted working buffers");
    assert_eq!(
        (
            decoded.width(),
            decoded.height(),
            decoded.image_index(),
            decoded.image_count()
        ),
        (4000, 3000, 1, 2)
    );
    let expected: Vec<f32> = [258u16, 12346, 32770, 65535]
        .into_iter()
        .enumerate()
        .map(|(channel, s)| {
            let x = f64::from(s) / 65535.;
            (if channel == 3 {
                x
            } else if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / 1.055).powf(2.4)
            }) as f32
        })
        .collect();
    let RasterPixels::Rgba32Float(samples) = decoded.pixels() else {
        panic!()
    };
    assert!(samples.is_managed());
    assert_eq!(samples.len(), 48_000_000);
    let native_bits: [u32; 4] = std::array::from_fn(|channel| samples[channel].to_bits());
    for pixel in samples.chunks_exact(4) {
        for channel in 0..4 {
            assert!((pixel[channel] - expected[channel]).abs() < 1e-7);
            assert_eq!(pixel[channel].to_bits(), native_bits[channel]);
        }
    }
    assert_eq!(root.used(), 192_000_000);
    // A constant 1x1 reference with equivalent display aspect avoids a second
    // full-sized authored allocation while checking every output pixel.
    let reference = DecodedRaster::new(
        1,
        1,
        RasterPixels::Rgba32Float(std::sync::Arc::new(expected).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap()
    .with_pixel_aspect(Some(4. / 3.))
    .unwrap();
    let view = rrrah_gpu::ViewParameters {
        viewport: [128., 96.],
        ..Default::default()
    };
    let sdr = gpu.render_raster(&reference, view, [128, 96]).pixels;
    let hdr = gpu.render_raster_half(&reference, view, [128, 96]);
    assert_eq!(gpu.render_raster(&decoded, view, [128, 96]).pixels, sdr);
    assert_eq!(gpu.render_raster_half(&decoded, view, [128, 96]), hdr);
    let dir = tempfile::tempdir().unwrap();
    let queue = MemoryBudget::new(192_000_000);
    let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
        dir.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 256 * 1024 * 1024,
                max_entries: Some(1),
                ttl: None,
            },
            queue_bytes: 192_000_000,
            queue_count: 1,
            restore_bytes: 192_000_000,
        },
        queue.clone(),
        root.clone(),
    )
    .unwrap();
    assert_eq!(swap.try_enqueue(1, decoded), rrrah_cache::SpillAdmission::Queued);
    swap.wait_idle().unwrap();
    assert_eq!(root.used(), 0);
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!((restored.image_index(), restored.image_count()), (1, 2));
    let RasterPixels::Rgba32Float(samples) = restored.pixels() else {
        panic!()
    };
    let RasterPixels::Rgba32Float(expected) = reference.pixels() else {
        panic!()
    };
    for pixel in samples.chunks_exact(4) {
        for channel in 0..4 {
            assert!((pixel[channel] - expected[channel]).abs() < 1e-7);
            assert_eq!(pixel[channel].to_bits(), native_bits[channel]);
        }
    }
    assert_eq!(gpu.render_raster(&restored, view, [128, 96]).pixels, sdr);
    assert_eq!(gpu.render_raster_half(&restored, view, [128, 96]), hdr);
    drop(restored);
    assert_eq!((root.used(), queue.used()), (0, 0));
    assert_eq!(
        (swap.stats().writes, swap.stats().reads, swap.stats().errors),
        (1, 1, 0)
    );
    eprintln!("large APNG managed decode peak: {} bytes", decode_budget.peak());
    eprintln!("large APNG: 4000x3000; all48000000 samples; pressure retry; SDR/HDR and swap exact; final=0");
}
