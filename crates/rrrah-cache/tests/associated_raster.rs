use rrrah_core::{
    DecodedRaster, MemoryBudget, RasterAlphaAssociation as Alpha, RasterAnimationTiming, RasterColorSpace,
    RasterPixels,
};
fn frame(root: &MemoryBudget) -> DecodedRaster {
    let values = vec![2., 0.5, 0.25, 0., -0.25, 4., 0.5, 0.5];
    DecodedRaster::new(
        2,
        1,
        RasterPixels::Rgba32Float(root.try_reserve(32).unwrap().try_adopt(values).unwrap().into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap()
    .with_alpha_association(Alpha::Associated)
    .unwrap()
    .with_pixel_aspect(Some(1.25))
    .unwrap()
    .with_hotspot(Some((1, 0)))
    .unwrap()
    .with_animation_timing(Some(RasterAnimationTiming::new(1001, 24, 2).unwrap()))
    .with_image_selection(2, 3)
    .unwrap()
}
#[test]
fn associated_payload_v6_and_legacy_straight_roundtrip_and_refusal() {
    let root = MemoryBudget::new(4096);
    let original = frame(&root);
    let mut bytes = Vec::new();
    rrrah_cache::write_raster_payload(&mut bytes, &original).unwrap();
    assert_eq!(&bytes[..8], b"RRRAST6\0");
    let read = rrrah_cache::read_raster_payload(&mut std::io::Cursor::new(&bytes), &root).unwrap();
    assert_eq!(read.alpha_association(), Alpha::Associated);
    assert_eq!(read.hotspot(), original.hotspot());
    assert_eq!(read.pixel_aspect(), original.pixel_aspect());
    assert_eq!(read.animation_timing(), original.animation_timing());
    assert_eq!(read.image_index(), 2);
    let RasterPixels::Rgba32Float(v) = read.pixels() else {
        panic!()
    };
    assert_eq!(v.as_ref(), &[2., 0.5, 0.25, 0., -0.25, 4., 0.5, 0.5]);
    for offset in [18, 19] {
        let mut bad = bytes.clone();
        bad[offset] = 0;
        assert!(rrrah_cache::read_raster_payload(&mut std::io::Cursor::new(bad), &root).is_err());
    }
    let mut nonlinear = bytes.clone();
    nonlinear[17] = 0;
    let empty = MemoryBudget::new(0);
    assert!(matches!(
        rrrah_cache::read_raster_payload(&mut std::io::Cursor::new(nonlinear), &empty),
        Err(rrrah_cache::RasterPayloadError::Invalid(
            "association requires float linear RGB"
        ))
    ));
    assert_eq!(empty.peak(), 0);
    let short = MemoryBudget::new(31);
    assert!(rrrah_cache::read_raster_payload(&mut std::io::Cursor::new(&bytes), &short).is_err());
    assert_eq!(short.used(), 0);
    let straight = original.clone().with_alpha_association(Alpha::Straight).unwrap();
    let mut legacy = Vec::new();
    rrrah_cache::write_raster_payload(&mut legacy, &straight).unwrap();
    assert_eq!(&legacy[..8], b"RRRAST5\0");
    let old = rrrah_cache::read_raster_payload(&mut std::io::Cursor::new(legacy), &root).unwrap();
    assert_eq!(old.alpha_association(), Alpha::Straight);
    drop((old, straight, read, original));
    assert_eq!(root.used(), 0);
}
#[test]
fn associated_ram_and_swapped_lod_keep_emission_and_crop_metadata() {
    let root = MemoryBudget::new(8192);
    let source = frame(&root);
    let mut ram = rrrah_cache::RasterRamCache::new(rrrah_memory::CacheLimits::bytes(32));
    assert!(ram.insert_visible(1u8, source.clone()));
    assert!(ram.get_visible_lease_for(&source).is_some());
    assert!(
        ram.get_visible_lease_for(&source.clone().with_alpha_association(Alpha::Straight).unwrap())
            .is_none()
    );
    drop(ram);

    let expected = source
        .downsample_linear_with_budget_and_cancel([1, 1], &root, || false)
        .unwrap();
    assert_eq!(expected.alpha_association(), Alpha::Associated);
    let RasterPixels::Rgba32Float(v) = expected.pixels() else {
        panic!()
    };
    assert_eq!(v.as_ref(), &[0.875, 2.25, 0.375, 0.25]);
    let tmp = tempfile::tempdir().unwrap();
    let store = rrrah_swap::SwapStore::new(
        tmp.path(),
        rrrah_swap::SwapLimits {
            max_bytes: 1024 * 1024,
            max_objects: Some(16),
        },
    )
    .unwrap();
    let index = MemoryBudget::new(8192);
    let swapped = rrrah_cache::SwappedRaster::write(&store, &source, &index, || false).unwrap();
    let lod = swapped
        .downsample_linear([1, 1], &root, &MemoryBudget::new(8192), || false)
        .unwrap();
    assert_eq!(lod.alpha_association(), Alpha::Associated);
    let RasterPixels::Rgba32Float(v) = lod.pixels() else {
        panic!()
    };
    assert_eq!(v.as_ref(), &[0.875, 2.25, 0.375, 0.25]);
    let crop = swapped
        .restore_region([0, 0, 1, 1], &root, &MemoryBudget::new(8192), || false)
        .unwrap();
    assert_eq!(crop.frame.alpha_association(), Alpha::Associated);
    assert_eq!(crop.frame.hotspot(), None);
    drop((crop, lod, swapped, expected, source));
    assert_eq!(root.used(), 0);
}

#[test]
fn associated_overflow_fixture_survives_swap_and_both_lod_paths() {
    let native =
        include_bytes!("../../../tests/fixtures/raster/associated-overflow-rec709.exr.native-rgba32le");
    let values: Vec<f32> = native
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let root = MemoryBudget::new(16384);
    let source = DecodedRaster::new(
        4,
        1,
        RasterPixels::Rgba32Float(root.try_reserve(64).unwrap().try_adopt(values).unwrap().into()),
        RasterColorSpace::LinearSrgb,
    )
    .unwrap()
    .with_alpha_association(Alpha::Associated)
    .unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let store = rrrah_swap::SwapStore::new(
        tmp.path(),
        rrrah_swap::SwapLimits {
            max_bytes: 1024 * 1024,
            max_objects: Some(8),
        },
    )
    .unwrap();
    let index = MemoryBudget::new(8192);
    let swapped = rrrah_cache::SwappedRaster::write(&store, &source, &index, || false).unwrap();
    let restored = swapped
        .restore_region([0, 0, 4, 1], &root, &MemoryBudget::new(8192), || false)
        .unwrap();
    assert_eq!(restored.frame.alpha_association(), Alpha::Associated);
    let RasterPixels::Rgba32Float(pixels) = restored.frame.pixels() else {
        panic!()
    };
    let bits: Vec<u8> = pixels.iter().flat_map(|v| v.to_le_bytes()).collect();
    assert_eq!(bits.as_slice(), native);
    let cpu = source
        .downsample_linear_with_budget_and_cancel([1, 1], &root, || false)
        .unwrap();
    let disk = swapped
        .downsample_linear([1, 1], &root, &MemoryBudget::new(8192), || false)
        .unwrap();
    for frame in [&cpu, &disk] {
        assert_eq!(frame.alpha_association(), Alpha::Associated);
        let RasterPixels::Rgba32Float(v) = frame.pixels() else {
            panic!()
        };
        let expected = [f32::MAX / 4., -f32::MAX / 4., 1.25, 0.375];
        assert_eq!(
            v.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected.map(f32::to_bits)
        );
    }
    drop((cpu, disk, restored, swapped, source));
    assert_eq!(root.used(), 0);
    assert_eq!(index.used(), 0);
}

#[test]
fn shared_segmented_quota_refuses_spill_without_corrupting_visible_and_retries() {
    let root = MemoryBudget::new(8192);
    let original = frame(&root);
    let directory = tempfile::tempdir().unwrap();
    let queue = MemoryBudget::new(64);
    let swap = rrrah_cache::RasterSwapCache::<u8>::new_with_budgets(
        directory.path(),
        rrrah_cache::ImageSwapConfig {
            limits: rrrah_cache::CacheLimits {
                max_bytes: 4096,
                max_entries: Some(1),
                ttl: None,
            },
            queue_bytes: 64,
            queue_count: 1,
            restore_bytes: 4096,
        },
        queue.clone(),
        root.clone(),
    )
    .unwrap();
    let index = MemoryBudget::new(8192);
    let visible =
        rrrah_cache::SwappedRaster::write(&swap.region_store(), &original, &index, || false).unwrap();
    assert_eq!(swap.disk_usage().unwrap().objects, 1);
    assert_eq!(
        swap.try_enqueue(1, original.clone()),
        rrrah_cache::SpillAdmission::Queued
    );
    swap.wait_idle().unwrap();
    assert_eq!(
        swap.stats().errors,
        0,
        "expected shared quota refusal is not an I/O failure"
    );
    assert_eq!(swap.stats().dropped, 1);
    assert_eq!(queue.used(), 0);
    assert!(swap.try_get(&1, || false).unwrap().is_none());
    assert_eq!(swap.disk_usage().unwrap().objects, 1);
    let restored = visible
        .restore_region([0, 0, 2, 1], &root, &MemoryBudget::new(8192), || false)
        .unwrap();
    assert_eq!(restored.frame.alpha_association(), Alpha::Associated);
    let RasterPixels::Rgba32Float(p) = restored.frame.pixels() else {
        panic!()
    };
    assert_eq!(p.as_ref(), &[2., 0.5, 0.25, 0., -0.25, 4., 0.5, 0.5]);
    drop((restored, visible));
    assert_eq!(index.used(), 0);
    assert_eq!(swap.disk_usage().unwrap().objects, 0);
    assert_eq!(
        swap.try_enqueue(1, original.clone()),
        rrrah_cache::SpillAdmission::Queued
    );
    swap.wait_idle().unwrap();
    assert_eq!(swap.stats().writes, 1);
    assert_eq!(swap.stats().dropped, 1);
    assert_eq!(swap.stats().errors, 0);
    let retry = swap.try_get(&1, || false).unwrap().unwrap();
    assert_eq!(retry.alpha_association(), Alpha::Associated);
    let RasterPixels::Rgba32Float(p) = retry.pixels() else {
        panic!()
    };
    assert_eq!(p.as_ref(), &[2., 0.5, 0.25, 0., -0.25, 4., 0.5, 0.5]);
    drop((retry, original));
    swap.shutdown();
    drop(swap);
    assert_eq!(root.used(), 0);
    assert_eq!(queue.used(), 0);
}
