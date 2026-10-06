use rrrah_dedup::{
    geometry::Transform,
    linear::LinearRgbaView,
    warp::{WarpError, WarpPolicy, verify_pixels},
};
const IDENTITY: Transform = Transform {
    a: 1.0,
    b: 0.0,
    translation: [0.0; 2],
};
const POLICY: WarpPolicy = WarpPolicy {
    tolerance: 1e-6,
    max_source_pixels: 100,
};
#[test]
fn crop_overlap_and_single_pixel_edits_are_explicit() {
    let source: Vec<f32> = (0_u8..12).flat_map(|i| [f32::from(i), 0.0, 0.0, 1.0]).collect();
    let target: Vec<f32> = [5_usize, 6, 9, 10]
        .into_iter()
        .flat_map(|i| source[i * 4..i * 4 + 4].iter().copied())
        .collect();
    let a = LinearRgbaView::new(4, 3, &source, 12, || false).unwrap();
    let b = LinearRgbaView::new(2, 2, &target, 4, || false).unwrap();
    let transform = Transform {
        translation: [-1.0, -1.0],
        ..IDENTITY
    };
    let evidence = verify_pixels(&a, &b, transform, POLICY, || false).unwrap();
    assert_eq!(
        (
            evidence.source_pixels,
            evidence.compared_pixels,
            evidence.matched_pixels
        ),
        (12, 4, 4)
    );
    let mut changed = target;
    changed[0] += 0.1;
    let b = LinearRgbaView::new(2, 2, &changed, 4, || false).unwrap();
    assert_eq!(
        verify_pixels(&a, &b, transform, POLICY, || false)
            .unwrap()
            .matched_pixels,
        3
    );
    assert_eq!(
        verify_pixels(
            &a,
            &b,
            transform,
            WarpPolicy {
                max_source_pixels: 1,
                ..POLICY
            },
            || false
        ),
        Err(WarpError::Budget)
    );
    assert_eq!(
        verify_pixels(&a, &b, transform, POLICY, || true),
        Err(WarpError::Cancelled)
    );
}
#[test]
fn bilinear_sampling_keeps_alpha_and_invisible_rgb_separate() {
    let source = LinearRgbaView::new(1, 1, &[1.0, 0.0, 0.0, 0.5], 1, || false).unwrap();
    let target = LinearRgbaView::new(2, 1, &[1.0, 0.0, 0.0, 1.0, 999.0, f32::NAN, 99.0, 0.0], 2, || {
        false
    })
    .unwrap();
    let evidence = verify_pixels(
        &source,
        &target,
        Transform {
            translation: [0.5, 0.0],
            ..IDENTITY
        },
        POLICY,
        || false,
    )
    .unwrap();
    assert_eq!(evidence.matched_pixels, 1);
    assert!(evidence.maximum_channel_error < 1e-9);
}

#[test]
fn reverse_verification_catches_edits_between_scaled_sample_centers() {
    use rrrah_dedup::warp::verify_bidirectional;
    let small = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
    let large = [0.0, 0.0, 0.0, 1.0, 0.5, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
    let a = LinearRgbaView::new(2, 1, &small, 2, || false).unwrap();
    let b = LinearRgbaView::new(3, 1, &large, 3, || false).unwrap();
    let transform = Transform { a: 2.0, ..IDENTITY };
    let result = verify_bidirectional(&a, &b, transform, POLICY, || false).unwrap();
    assert_eq!(
        (result.forward.matched_pixels, result.reverse.matched_pixels),
        (2, 3)
    );
    let mut changed = large;
    changed[4] = 0.9;
    let c = LinearRgbaView::new(3, 1, &changed, 3, || false).unwrap();
    let result = verify_bidirectional(&a, &c, transform, POLICY, || false).unwrap();
    assert_eq!(
        (result.forward.matched_pixels, result.reverse.matched_pixels),
        (2, 2)
    );
}

#[test]
fn reflected_grids_preserve_hdr_alpha_and_inverse_with_atomic_cancel() {
    use rrrah_dedup::warp::verify_reflected_bidirectional;
    let pixels = [
        [9.0_f32, -2.0, 0.0, 1.0],
        [0.2, 3.0, 0.0, 0.5],
        [77.0, -99.0, 0.0, 0.0],
        [-3.0, 0.0, 1.0, 1.0],
        [2.0, 0.1, 0.0, 0.25],
        [0.4, 0.6, 0.8, 1.0],
    ];
    let source = pixels.into_iter().flatten().collect::<Vec<_>>();
    let a = LinearRgbaView::new(3, 2, &source, 6, || false).unwrap();
    for (width, height, order, transform) in [
        (
            3,
            2,
            [2, 1, 0, 5, 4, 3],
            Transform {
                a: 1.0,
                b: 0.0,
                translation: [2.0, 0.0],
            },
        ),
        (
            2,
            3,
            [0, 3, 1, 4, 2, 5],
            Transform {
                a: 0.0,
                b: -1.0,
                translation: [0.0, 0.0],
            },
        ),
    ] {
        let mut target = order.into_iter().flat_map(|i| pixels[i]).collect::<Vec<_>>();
        // Invisible RGB is irrelevant even when the hidden values differ.
        let hidden = order.iter().position(|&i| i == 2).unwrap() * 4;
        target[hidden] = -1234.0;
        let b = LinearRgbaView::new(width, height, &target, 6, || false).unwrap();
        let calls = std::cell::Cell::new(0);
        let expected = verify_reflected_bidirectional(&a, &b, transform, POLICY, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        for result in [&expected.forward, &expected.reverse] {
            assert_eq!((result.compared_pixels, result.matched_pixels), (6, 6));
            assert_eq!(result.squared_error, 0.0);
        }
        for checkpoint in 1..=calls.get() {
            let current = std::cell::Cell::new(0);
            assert_eq!(
                verify_reflected_bidirectional(&a, &b, transform, POLICY, || {
                    current.set(current.get() + 1);
                    current.get() == checkpoint
                }),
                Err(WarpError::Cancelled)
            );
        }
        assert_eq!(
            verify_reflected_bidirectional(&a, &b, transform, POLICY, || false).unwrap(),
            expected
        );
        let visible = order.iter().position(|&i| i == 0).unwrap() * 4;
        target[visible] += 1.0;
        let b = LinearRgbaView::new(width, height, &target, 6, || false).unwrap();
        let changed = verify_reflected_bidirectional(&a, &b, transform, POLICY, || false).unwrap();
        assert!(changed.forward.matched_pixels < 6);
        assert!(changed.reverse.matched_pixels < 6);
        assert_eq!(
            verify_reflected_bidirectional(
                &a,
                &b,
                transform,
                WarpPolicy {
                    max_source_pixels: 5,
                    ..POLICY
                },
                || false
            ),
            Err(WarpError::Budget)
        );
    }
}
