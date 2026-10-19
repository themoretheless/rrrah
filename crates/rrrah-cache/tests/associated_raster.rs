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
