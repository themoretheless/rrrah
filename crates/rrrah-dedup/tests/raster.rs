#![cfg(feature = "raster")]
use rrrah_core::{DecodedRaster, MemoryBudget, RasterColorSpace, RasterPixels};
use rrrah_dedup::raster::NormalizedRaster;
use std::sync::Arc;

#[test]
fn storage_precision_and_frame_metadata_are_preserved() {
    let a = DecodedRaster::new(
        1,
        1,
        RasterPixels::Rgba8(Arc::new(vec![255, 0, 0, 255]).into()),
        RasterColorSpace::Srgb,
    )
    .unwrap()
    .with_image_selection(2, 3)
    .unwrap();
    let b = DecodedRaster::new(
        1,
        1,
        RasterPixels::Rgba16(Arc::new(vec![65535, 0, 0, 65535]).into()),
        RasterColorSpace::Srgb,
    )
    .unwrap();
    let budget = MemoryBudget::new(1024);
    let a = NormalizedRaster::new(&a, 1, &budget, || false).unwrap();
    let b = NormalizedRaster::new(&b, 1, &budget, || false).unwrap();
    assert!(a.same_selected_frame(&b, || false).unwrap());
    assert_eq!((a.image_index(), a.image_count()), (2, 3));
}

#[test]
fn unqualified_color_and_insufficient_budget_fail() {
    let make =
        |color| DecodedRaster::new(1, 1, RasterPixels::Rgba8(Arc::new(vec![255; 4]).into()), color).unwrap();
    let budget = MemoryBudget::new(1024);
    for color in [
        RasterColorSpace::Icc(vec![1]),
        RasterColorSpace::Unspecified,
        RasterColorSpace::LinearRgbUnspecified,
    ] {
        assert!(NormalizedRaster::new(&make(color), 1, &budget, || false).is_err());
    }
    let source = make(RasterColorSpace::Srgb);
    assert!(NormalizedRaster::new(&source, 1, &MemoryBudget::new(0), || false).is_err());
    assert!(NormalizedRaster::new(&source, 0, &budget, || false).is_err());
    assert!(NormalizedRaster::new(&source, 1, &budget, || true).is_err());
}

#[test]
fn cancellation_interrupts_each_pixel_representation() {
    use rrrah_core::RasterError;
    use rrrah_dedup::raster::AdapterError;
    use std::cell::Cell;
    for pixels in [
        RasterPixels::Rgba8(Arc::new(vec![255; 4 * 16384]).into()),
        RasterPixels::Rgba16(Arc::new(vec![65535; 4 * 16384]).into()),
        RasterPixels::Rgba32Float(Arc::new(vec![1.0; 4 * 16384]).into()),
    ] {
        for color in [RasterColorSpace::Srgb, RasterColorSpace::LinearSrgb] {
            let source = DecodedRaster::new(16384, 1, pixels.clone(), color).unwrap();
            let calls = Cell::new(0);
            let budget = MemoryBudget::new(4 * 4 * 16384);
            let result = NormalizedRaster::new(&source, 16384, &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == 4
            });
            assert!(matches!(
                result,
                Err(AdapterError::Raster(RasterError::Cancelled))
            ));
            // The failed conversion must release the entire output reservation.
            assert!(budget.try_reserve(4 * 4 * 16384).is_ok());
            assert_eq!(calls.get(), 4);
        }
    }
}

#[test]
fn selected_frame_key_tracks_equality_metadata_but_excludes_container_selection() {
    let source = DecodedRaster::new(
        2,
        1,
        RasterPixels::Rgba8(Arc::new(vec![255; 8]).into()),
        RasterColorSpace::Srgb,
    )
    .unwrap();
    let budget = MemoryBudget::new(4096);
    let base = NormalizedRaster::new(&source, 2, &budget, || false).unwrap();
    let selected = NormalizedRaster::new(
        &source.clone().with_image_selection(1, 3).unwrap(),
        2,
        &budget,
        || false,
    )
    .unwrap();
    assert!(base.same_selected_frame(&selected, || false).unwrap());
    assert_eq!(
        base.selected_frame_digest(|| false).unwrap(),
        selected.selected_frame_digest(|| false).unwrap()
    );
    for variant in [
        source.clone().with_sample_scale(2.0).unwrap(),
        source.with_hotspot(Some((1, 0))).unwrap(),
    ] {
        let other = NormalizedRaster::new(&variant, 2, &budget, || false).unwrap();
        assert!(!base.same_selected_frame(&other, || false).unwrap());
        assert_ne!(
            base.selected_frame_digest(|| false).unwrap(),
            other.selected_frame_digest(|| false).unwrap()
        );
    }
    assert!(base.selected_frame_digest(|| true).is_err());
}

#[test]
fn normalized_keys_and_direct_equality_agree_with_independent_hdr_alpha_precision_labels() {
    // Equal-class labels are authored from canonical sample semantics, not from hashes.
    let fixtures = [
        (0, [2.0, -0.1, 0.1234, 1.0]),
        (0, [2.0, -0.1, 0.1234, 1.0]),
        (1, [2.0, -0.1, 0.1235, 1.0]),
        (2, [2.1, -0.1, 0.1234, 1.0]),
        (3, [2.0, -0.1, 0.1234, 0.5]),
        (4, [-0.0, 0.0, 1.0, 1.0]),
        (4, [0.0, -0.0, 1.0, 1.0]),
        (5, [99.0, -20.0, 5.0, 0.0]),
        (5, [-5.0, 13.0, 7.0, -0.0]),
    ];
    let budget = MemoryBudget::new(1024 * 1024);
    let mut frames = Vec::new();
    for (class, samples) in fixtures {
        let source = DecodedRaster::new(
            1,
            1,
            RasterPixels::Rgba32Float(Arc::new(samples.to_vec()).into()),
            RasterColorSpace::LinearSrgb,
        )
        .unwrap();
        let frame = NormalizedRaster::new(&source, 1, &budget, || false).unwrap();
        let known = rrrah_dedup::linear::LinearRgbaView::new(1, 1, &samples, 1, || false).unwrap();
        assert!(
            frame
                .view(|| false)
                .unwrap()
                .same_pixels(&known, || false)
                .unwrap()
        );
        frames.push((class, frame));
    }
    for pixels in [
        RasterPixels::Rgba8(Arc::new(vec![255, 0, 0, 255]).into()),
        RasterPixels::Rgba16(Arc::new(vec![65535, 0, 0, 65535]).into()),
    ] {
        let source = DecodedRaster::new(1, 1, pixels, RasterColorSpace::Srgb).unwrap();
        frames.push((6, NormalizedRaster::new(&source, 1, &budget, || false).unwrap()));
    }
    let keys = frames
        .iter()
        .map(|(_, frame)| frame.selected_frame_digest(|| false).unwrap())
        .collect::<Vec<_>>();
    for (i, (left_class, left)) in frames.iter().enumerate() {
        for (j, (right_class, right)) in frames.iter().enumerate() {
            let expected = left_class == right_class;
            assert_eq!(
                left.same_selected_frame(right, || false).unwrap(),
                expected,
                "direct {i} {j}"
            );
            assert_eq!(keys[i] == keys[j], expected, "key {i} {j}");
        }
    }
    drop(frames);
    assert_eq!(budget.used(), 0);
}
