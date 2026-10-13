#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "manual Metal compute-to-cache pipeline qualification"]
fn compute_hdr_result_preserves_float_bits_ram_swap_and_rendering() {
    qualify_compute_cache_swap(164, 50, [128, 96]);
}

#[test]
#[ignore = "manual 12 MP Metal compute-to-cache and full-frame readback qualification"]
fn large_compute_hdr_result_preserves_float_bits_ram_swap_and_rendering() {
    qualify_compute_cache_swap(4000, 3000, [4000, 3000]);
}

fn qualify_compute_cache_swap(width: u32, height: u32, output: [u32; 2]) {
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("compute cache adapter: {}", gpu.adapter_name());
    let count = width as usize * height as usize;
    let bytes = count as u64 * 16;
    let root = MemoryBudget::new(bytes * 2);
    // The 12 MP case spans multiple bounded dispatches, each with a 16-byte uniform.
    let gpu_root = MemoryBudget::new(bytes * 3 + 64);
    let queue_root = MemoryBudget::new(bytes);
    let mut source = root.try_buffer(count * 4, 0f32).unwrap();
    for (i, pixel) in source.chunks_exact_mut(4).enumerate() {
        pixel.copy_from_slice(&[
            if i % 2 == 0 { -0. } else { 4. },
            -2.,
            (i % 17) as f32 / 8.,
            (i % 13) as f32 / 16.,
        ]);
    }
    let expected: Vec<_> = source
        .chunks_exact(4)
        .flat_map(|p| [p[0] * 2., p[1] * 2., p[2] * 2., p[3]])
        .collect();
    let expected_bits: Vec<_> = expected.iter().map(|v| v.to_bits()).collect();
    let source = source.freeze();
    let computed = gpu.expose_interleaved(&source, 1., &gpu_root, &root).unwrap();
    assert_eq!(
        computed.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected_bits
    );
    assert_eq!(gpu_root.used(), 0);
    assert_eq!(root.used(), bytes * 2);
    drop(source);
    assert_eq!(root.used(), bytes);
    let raster = DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba32Float(computed),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let oracle = DecodedRaster::new(
        width,
        height,
        RasterPixels::Rgba32Float(std::sync::Arc::new(expected).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let view = rrrah_gpu::ViewParameters {
        viewport: [output[0] as f32, output[1] as f32],
        ..Default::default()
    };
    let expected_frame = gpu.render_raster(&oracle, view, output).pixels;
    assert!(expected_frame.chunks_exact(4).any(|pixel| pixel[0] > 100 || pixel[2] > 100));
    let expected_hdr = gpu.render_raster_half(&oracle, view, output);
    assert!(expected_hdr.iter().all(|v| v & 0x7c00 != 0x7c00));
    assert!(expected_hdr.chunks_exact(4).any(|p| {
        p[..3].iter().any(|v| v & 0x8000 == 0 && *v > 0x3c00)
    }));
    assert!(expected_hdr.chunks_exact(4).any(|p| {
        p[..3].iter().any(|v| v & 0x8000 != 0 && v & 0x7fff != 0)
    }));
    assert_eq!(gpu.render_raster_half(&raster, view, output), expected_hdr);
    assert_eq!(gpu.render_raster(&raster, view, output).pixels, expected_frame);
    let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(bytes));
    assert!(ram.insert(1u8, raster.clone()));
    let lease = ram.get_lease(&1).unwrap();
    assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
    assert_eq!(gpu.render_raster(&lease, view, output).pixels, expected_frame);
    assert_eq!(gpu.render_raster_half(&lease, view, output), expected_hdr);
    drop(lease);
    drop(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).unwrap());
    let directory = tempfile::tempdir().unwrap();
    let swap: rrrah_cache::RasterSwapCache<u8> = rrrah_cache::ImageSwapCache::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits::bytes(bytes + 64),
            queue_bytes: bytes,
            queue_count: 1,
            restore_bytes: bytes,
        },
        queue_root.clone(),
        root.clone(),
    )
    .unwrap();
    assert_eq!(swap.try_enqueue(1, raster), rrrah_cache::SpillAdmission::Queued);
    swap.wait_idle().unwrap();
    assert_eq!((root.used(), queue_root.used()), (0, 0));
    let pressure = root.try_reserve(root.limit()).unwrap();
    assert!(swap.try_get(&1, || false).is_err());
    assert_eq!(root.used(), root.limit());
    drop(pressure);
    let mut cancelled_after_admission = false;
    assert!(swap.try_get(&1, || {
        if root.used() != 0 {
            cancelled_after_admission = true;
            true
        } else {
            false
        }
    }).unwrap().is_none());
    assert!(cancelled_after_admission, "cancellation must observe admitted restore memory");
    assert_eq!((root.used(), queue_root.used()), (0, 0));
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    let RasterPixels::Rgba32Float(values) = restored.pixels() else {
        panic!()
    };
    assert_eq!(
        values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected_bits
    );
    assert_eq!(
        gpu.render_raster(&restored, view, output).pixels,
        expected_frame
    );
    assert_eq!(gpu.render_raster_half(&restored, view, output), expected_hdr);
    drop(restored);
    assert_eq!((root.used(), queue_root.used(), gpu_root.used()), (0, 0, 0));
    assert_eq!(
        (swap.stats().writes, swap.stats().reads, swap.stats().errors),
        (1, 1, 0)
    );
    eprintln!("compute cache recovery: {width}x{height}; full-budget refusal; admitted cancellation; exact SDR-HDR retry; final=0");
    eprintln!("compute cache HDR: {width}x{height}; finite signed HDR; computed-pinned-restored binary16 exact");
    eprintln!("compute cache pipeline: {width}x{height}; all component bits and {}x{} Metal frames exact; root=0; gpu=0; one swap write/read", output[0], output[1]);
}
