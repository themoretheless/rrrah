use rrrah_dedup::{
    geometry::Transform,
    linear::LinearRgbaView,
    warp::{
        FilterPolicy, PhotometricPolicy, WarpError, WarpPolicy, verify_filtered_photometric_bidirectional,
        verify_photometric_bidirectional,
    },
};
const ID: Transform = Transform {
    a: 1.,
    b: 0.,
    translation: [0.; 2],
};
const PHOTO: PhotometricPolicy = PhotometricPolicy {
    residual: WarpPolicy {
        tolerance: 0.03,
        max_source_pixels: 100,
    },
    minimum_samples: 8,
    minimum_variance: 1e-5,
    minimum_gain: 0.2,
    maximum_gain: 5.,
    maximum_offset: 1.,
};
const FILTER: FilterPolicy = FilterPolicy {
    radius: 1,
    max_sample_pairs: 2916,
};
fn signal() -> Vec<f32> {
    (0_u8..9)
        .flat_map(|y| {
            (0_u8..9).flat_map(move |x| [f32::from(x) / 8., f32::from(y) / 8., f32::from(x + y) / 16., 1.])
        })
        .collect()
}
#[test]
fn independently_known_box_mean_retains_strict_high_frequency_differences() {
    let a = signal();
    let mut b = a.clone();
    for y in 0..9 {
        for x in 0..9 {
            let sign = if (x + y) % 2 == 0 { 1. } else { -1. };
            for (c, amp) in [0.1, 0.08, -0.06].into_iter().enumerate() {
                b[(y * 9 + x) * 4 + c] += amp * sign;
            }
        }
    }
    let av = LinearRgbaView::new(9, 9, &a, 81, || false).unwrap();
    let bv = LinearRgbaView::new(9, 9, &b, 81, || false).unwrap();
    let raw = verify_photometric_bidirectional(&av, &bv, ID, PHOTO, || false).unwrap();
    assert!(raw.forward.pixels.matched_pixels < raw.forward.pixels.compared_pixels);
    let e = verify_filtered_photometric_bidirectional(&av, &bv, ID, PHOTO, FILTER, || false).unwrap();
    assert_eq!(e.filter, FILTER);
    for p in [&e.evidence.forward, &e.evidence.reverse] {
        assert_eq!(
            (
                p.fitted_samples,
                p.pixels.source_pixels,
                p.pixels.compared_pixels,
                p.pixels.matched_pixels
            ),
            (49, 81, 49, 49)
        );
    }
    // A 3x3 box sums the checker to one signed sample. On the 7x7 interior,
    // the remaining noise mean is amplitude/(9*49), orthogonal to the gradients.
    for (c, amp) in [0.1, 0.08, -0.06].into_iter().enumerate() {
        assert!((e.evidence.forward.gain[c] - 1.).abs() < 1e-6);
        assert!((e.evidence.forward.offset[c] - amp / (9. * 49.)).abs() < 1e-6);
    }
}
#[test]
fn broad_content_edits_and_alpha_changes_remain_visible_after_filtering() {
    let a = signal();
    let av = LinearRgbaView::new(9, 9, &a, 81, || false).unwrap();
    let mut b = a.clone();
    for y in 2..7 {
        for x in 2..7 {
            for c in 0..3 {
                b[(y * 9 + x) * 4 + c] += 0.4;
            }
        }
    }
    let bv = LinearRgbaView::new(9, 9, &b, 81, || false).unwrap();
    let e = verify_filtered_photometric_bidirectional(&av, &bv, ID, PHOTO, FILTER, || false).unwrap();
    assert!(e.evidence.forward.pixels.matched_pixels < 44);
    assert!(e.evidence.reverse.pixels.matched_pixels < 44);
    let mut b = a.clone();
    b[(4 * 9 + 4) * 4 + 3] = 0.;
    b[(4 * 9 + 4) * 4] = f32::NAN;
    let bv = LinearRgbaView::new(9, 9, &b, 81, || false).unwrap();
    let e = verify_filtered_photometric_bidirectional(&av, &bv, ID, PHOTO, FILTER, || false).unwrap();
    assert_eq!(
        (
            e.evidence.forward.pixels.matched_pixels,
            e.evidence.reverse.pixels.matched_pixels
        ),
        (40, 40)
    );
}
#[test]
fn complete_work_is_admitted_before_sampling_and_cancellation_has_no_partial_evidence() {
    let a = signal();
    let av = LinearRgbaView::new(9, 9, &a, 81, || false).unwrap();
    let calls = std::cell::Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        false
    };
    assert_eq!(
        verify_filtered_photometric_bidirectional(
            &av,
            &av,
            ID,
            PHOTO,
            FilterPolicy {
                max_sample_pairs: 2915,
                ..FILTER
            },
            cancel
        ),
        Err(WarpError::Budget)
    );
    assert_eq!(calls.get(), 0);
    let large = [0.5, 0.5, 0.5, 1.0].repeat(100);
    let target = LinearRgbaView::new(10, 10, &large, 100, || false).unwrap();
    assert_eq!(
        verify_filtered_photometric_bidirectional(
            &av,
            &target,
            ID,
            PhotometricPolicy {
                residual: WarpPolicy {
                    max_source_pixels: 81,
                    ..PHOTO.residual
                },
                ..PHOTO
            },
            FilterPolicy {
                max_sample_pairs: 5000,
                ..FILTER
            },
            cancel
        ),
        Err(WarpError::Budget)
    );
    assert_eq!(calls.get(), 0);

    assert_eq!(
        verify_filtered_photometric_bidirectional(
            &av,
            &av,
            ID,
            PHOTO,
            FilterPolicy {
                radius: u32::MAX,
                ..FILTER
            },
            || false
        ),
        Err(WarpError::Invalid)
    );
    assert_eq!(
        verify_filtered_photometric_bidirectional(&av, &av, ID, PHOTO, FILTER, || true),
        Err(WarpError::Cancelled)
    );
    let calls = std::cell::Cell::new(0);
    assert_eq!(
        verify_filtered_photometric_bidirectional(&av, &av, ID, PHOTO, FILTER, || {
            calls.set(calls.get() + 1);
            calls.get() == 200
        }),
        Err(WarpError::Cancelled)
    );
    let flat = [0.5, 0.5, 0.5, 1.0].repeat(81);
    let fv = LinearRgbaView::new(9, 9, &flat, 81, || false).unwrap();
    assert_eq!(
        verify_filtered_photometric_bidirectional(&fv, &fv, ID, PHOTO, FILTER, || false),
        Err(WarpError::Fit(
            rrrah_dedup::warp::PhotometricFitFailure::LowVariance { channel: 0 }
        ))
    );
}

#[test]
fn reflected_filter_retains_color_space_alpha_work_and_cancel_policy() {
    use rrrah_dedup::warp::{
        ColorFilterPolicy, FilterColorSpace, verify_reflected_filtered_photometric,
        verify_reflected_filtered_photometric_constrained,
    };
    let source = signal();
    let mut target = (0..9)
        .flat_map(|y| {
            (0..9)
                .rev()
                .flat_map(move |x| (y * 9 + x) * 4..(y * 9 + x) * 4 + 4)
        })
        .map(|i| source[i])
        .collect::<Vec<_>>();
    let a = LinearRgbaView::new(9, 9, &source, 81, || false).unwrap();
    let transform = Transform {
        translation: [8., 0.],
        ..ID
    };
    for color_space in [FilterColorSpace::LinearSrgb, FilterColorSpace::EncodedSrgb] {
        let policy = ColorFilterPolicy {
            filter: FILTER,
            color_space,
        };
        let b = LinearRgbaView::new(9, 9, &target, 81, || false).unwrap();
        let calls = std::cell::Cell::new(0);
        let expected = verify_reflected_filtered_photometric(&a, &b, transform, PHOTO, policy, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        assert_eq!(expected.color_space, color_space);
        for evidence in [&expected.evidence.forward, &expected.evidence.reverse] {
            assert_eq!(
                (evidence.pixels.compared_pixels, evidence.pixels.matched_pixels),
                (49, 49)
            );
        }
        let constrained =
            verify_reflected_filtered_photometric_constrained(&a, &b, transform, PHOTO, policy, || false)
                .unwrap();
        assert_eq!(constrained.evidence.forward.pixels.matched_pixels, 49);
        assert_eq!(constrained.evidence.reverse.pixels.matched_pixels, 49);
        for checkpoint in [1, 30, 100, calls.get()] {
            let current = std::cell::Cell::new(0);
            assert_eq!(
                verify_reflected_filtered_photometric(&a, &b, transform, PHOTO, policy, || {
                    current.set(current.get() + 1);
                    current.get() == checkpoint
                }),
                Err(WarpError::Cancelled)
            );
        }
        assert_eq!(
            verify_reflected_filtered_photometric(&a, &b, transform, PHOTO, policy, || false).unwrap(),
            expected
        );
        assert_eq!(
            verify_reflected_filtered_photometric(
                &a,
                &b,
                transform,
                PHOTO,
                ColorFilterPolicy {
                    filter: FilterPolicy {
                        max_sample_pairs: FILTER.max_sample_pairs - 1,
                        ..FILTER
                    },
                    ..policy
                },
                || false
            ),
            Err(WarpError::Budget)
        );
    }
    target[(4 * 9 + 4) * 4 + 3] = 0.5;
    let b = LinearRgbaView::new(9, 9, &target, 81, || false).unwrap();
    let result = verify_reflected_filtered_photometric(
        &a,
        &b,
        transform,
        PHOTO,
        ColorFilterPolicy {
            filter: FILTER,
            color_space: FilterColorSpace::LinearSrgb,
        },
        || false,
    )
    .unwrap();
    assert!(
        result.evidence.forward.pixels.matched_pixels < 49
            && result.evidence.reverse.pixels.matched_pixels < 49
    );
}
