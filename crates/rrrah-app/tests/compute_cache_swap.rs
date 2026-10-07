#[path = "../../rrrah-gpu/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "manual Metal compute-to-cache pipeline qualification"]
fn compute_hdr_result_preserves_float_bits_ram_swap_and_rendering() {
    use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
    let gpu = common::qualification_gpu().expect("actual GPU required");
    eprintln!("compute cache adapter: {}", gpu.adapter_name());
    let count = 8200usize;
    let bytes = count as u64 * 16;
    let root = MemoryBudget::new(bytes * 2);
    let gpu_root = MemoryBudget::new(bytes * 3 + 16);
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
        164,
        50,
        RasterPixels::Rgba32Float(computed),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let oracle = DecodedRaster::new(
        164,
        50,
        RasterPixels::Rgba32Float(std::sync::Arc::new(expected).into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap();
    let view = rrrah_gpu::ViewParameters {
        viewport: [128., 96.],
        ..Default::default()
    };
    let expected_frame = gpu.render_raster(&oracle, view, [128, 96]).pixels;
    assert!(expected_frame.chunks_exact(4).any(|pixel| pixel[0] > 100 || pixel[2] > 100));
    assert_eq!(gpu.render_raster(&raster, view, [128, 96]).pixels, expected_frame);
    let mut ram = rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits::bytes(bytes));
    assert!(ram.insert(1u8, raster.clone()));
    let lease = ram.get_lease(&1).unwrap();
    assert!(ram.set_limits(rrrah_cache::CacheLimits::bytes(0)).is_none());
    assert_eq!(gpu.render_raster(&lease, view, [128, 96]).pixels, expected_frame);
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
        MemoryBudget::new(bytes),
        root.clone(),
    )
    .unwrap();
    assert_eq!(swap.try_enqueue(1, raster), rrrah_cache::SpillAdmission::Queued);
    swap.wait_idle().unwrap();
    assert_eq!(root.used(), 0);
    let restored = swap.try_get(&1, || false).unwrap().unwrap();
    let RasterPixels::Rgba32Float(values) = restored.pixels() else {
        panic!()
    };
    assert_eq!(
        values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected_bits
    );
    assert_eq!(
        gpu.render_raster(&restored, view, [128, 96]).pixels,
        expected_frame
    );
    drop(restored);
    assert_eq!(root.used(), 0);
    assert_eq!(
        (swap.stats().writes, swap.stats().reads, swap.stats().errors),
        (1, 1, 0)
    );
}
