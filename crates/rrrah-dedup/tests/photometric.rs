use rrrah_dedup::{
    geometry::Transform,
    linear::LinearRgbaView,
    warp::{PhotometricPolicy, WarpError, WarpPolicy, verify_photometric_bidirectional},
};
const ID: Transform = Transform {
    a: 1.,
    b: 0.,
    translation: [0.; 2],
};
const POLICY: PhotometricPolicy = PhotometricPolicy {
    residual: WarpPolicy {
        tolerance: 1e-6,
        max_source_pixels: 100,
    },
    minimum_samples: 8,
    minimum_variance: 0.0001,
    minimum_gain: 0.2,
    maximum_gain: 5.,
    maximum_offset: 0.2,
};
fn data() -> Vec<f32> {
    (0_u8..20)
        .flat_map(|i| {
            let v = f32::from(i) / 20.;
            [v, v * v, 1. - v, 1.]
        })
        .collect()
}
#[test]
fn known_channel_gains_offsets_and_spatial_edit_are_separate_evidence() {
    let a = data();
    let mut b: Vec<f32> = a
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[0] * 0.5 + 0.03, p[1] * 0.8 + 0.02, p[2] * 1.2 - 0.01, p[3]])
        .collect();
    let av = LinearRgbaView::new(5, 4, &a, 20, || false).unwrap();
    let bv = LinearRgbaView::new(5, 4, &b, 20, || false).unwrap();
    let e = verify_photometric_bidirectional(&av, &bv, ID, POLICY, || false).unwrap();
    assert_eq!(
        (e.forward.pixels.matched_pixels, e.reverse.pixels.matched_pixels),
        (20, 20)
    );
    for (a, b) in e.forward.gain.into_iter().zip([0.5, 0.8, 1.2]) {
        assert!((a - b).abs() < 1e-6);
    }
    for (a, b) in e.forward.offset.into_iter().zip([0.03, 0.02, -0.01]) {
        assert!((a - b).abs() < 1e-6);
    }
    assert!(!av.same_pixels(&bv, || false).unwrap());
    b[24] += 0.2;
    let bv = LinearRgbaView::new(5, 4, &b, 20, || false).unwrap();
    let e = verify_photometric_bidirectional(&av, &bv, ID, POLICY, || false).unwrap();
    assert!(e.forward.pixels.matched_pixels < 20 && e.reverse.pixels.matched_pixels < 20);
}
#[test]
fn flat_content_alpha_edits_limits_and_cancellation_do_not_disappear() {
    let a = data();
    let av = LinearRgbaView::new(5, 4, &a, 20, || false).unwrap();
    let flat = [0.5, 0.5, 0.5, 1.0].repeat(20);
    let fv = LinearRgbaView::new(5, 4, &flat, 20, || false).unwrap();
    assert_eq!(
        verify_photometric_bidirectional(&fv, &fv, ID, POLICY, || false),
        Err(WarpError::Fit(
            rrrah_dedup::warp::PhotometricFitFailure::LowVariance { channel: 0 }
        ))
    );
    assert_eq!(
        verify_photometric_bidirectional(&av, &av, ID, POLICY, || true),
        Err(WarpError::Cancelled)
    );
    assert_eq!(
        verify_photometric_bidirectional(
            &av,
            &av,
            ID,
            PhotometricPolicy {
                residual: WarpPolicy {
                    max_source_pixels: 1,
                    ..POLICY.residual
                },
                ..POLICY
            },
            || false
        ),
        Err(WarpError::Budget)
    );
    let mut b = a.clone();
    b[3] = 0.;
    let bv = LinearRgbaView::new(5, 4, &b, 20, || false).unwrap();
    let e = verify_photometric_bidirectional(&av, &bv, ID, POLICY, || false).unwrap();
    assert_eq!(
        (e.forward.pixels.matched_pixels, e.reverse.pixels.matched_pixels),
        (19, 19)
    );
    assert_eq!(
        verify_photometric_bidirectional(
            &av,
            &av,
            ID,
            PhotometricPolicy {
                minimum_variance: f64::NAN,
                ..POLICY
            },
            || false
        ),
        Err(WarpError::Invalid)
    );
}

#[test]
fn incomplete_fits_have_specific_reasons_instead_of_invalid_pixel_errors() {
    use rrrah_dedup::warp::PhotometricFitFailure;
    let a = data();
    let av = LinearRgbaView::new(5, 4, &a, 20, || false).unwrap();
    let invisible = [0., 0., 0., 0.].repeat(20);
    let bv = LinearRgbaView::new(5, 4, &invisible, 20, || false).unwrap();
    assert_eq!(
        verify_photometric_bidirectional(&av, &bv, ID, POLICY, || false),
        Err(WarpError::Fit(PhotometricFitFailure::InsufficientSamples {
            observed: 0,
            required: 8
        }))
    );
    let bright: Vec<f32> = a
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[0] * 9., p[1] * 9., p[2] * 9., p[3]])
        .collect();
    let bv = LinearRgbaView::new(5, 4, &bright, 20, || false).unwrap();
    assert_eq!(
        verify_photometric_bidirectional(&av, &bv, ID, POLICY, || false),
        Err(WarpError::Fit(PhotometricFitFailure::OutsidePolicy {
            channel: 0
        }))
    );
    let mut a = data();
    for p in a.as_chunks_mut::<4>().0 {
        p[1] = 0.;
    }
    let av = LinearRgbaView::new(5, 4, &a, 20, || false).unwrap();
    assert_eq!(
        verify_photometric_bidirectional(&av, &av, ID, POLICY, || false),
        Err(WarpError::Fit(PhotometricFitFailure::LowVariance { channel: 1 }))
    );
}

#[test]
fn bounded_fit_excludes_saturation_from_model_but_never_from_residuals() {
    use rrrah_dedup::warp::{FitSampleRange, verify_photometric_bidirectional_in_range};
    let mut a = data();
    a[0] = 4.0;
    let mut b = a.clone();
    b[0] = 1.0;
    let av = LinearRgbaView::new(5, 4, &a, 20, || false).unwrap();
    let bv = LinearRgbaView::new(5, 4, &b, 20, || false).unwrap();
    let range = FitSampleRange {
        minimum: 0.0,
        maximum: 1.0,
    };
    let e = verify_photometric_bidirectional_in_range(&av, &bv, ID, POLICY, range, || false).unwrap();
    assert_eq!(e.range, range);
    for side in [&e.evidence.forward, &e.evidence.reverse] {
        assert_eq!(side.fitted_samples, 19);
        assert_eq!(side.pixels.compared_pixels, 20);
        assert_eq!(side.pixels.matched_pixels, 19);
        assert!(side.gain.iter().all(|v| (v - 1.0).abs() < 1e-10));
        assert!(side.offset.iter().all(|v| v.abs() < 1e-10));
        assert!((side.pixels.maximum_channel_error - 3.0).abs() < 1e-10);
    }
    b[12] += 0.15;
    b[27] = 0.2;
    let bv = LinearRgbaView::new(5, 4, &b, 20, || false).unwrap();
    let edited = verify_photometric_bidirectional_in_range(&av, &bv, ID, POLICY, range, || false).unwrap();
    assert!(edited.evidence.forward.pixels.matched_pixels < 19);
    assert!(edited.evidence.reverse.pixels.matched_pixels < 19);
    assert!(matches!(
        verify_photometric_bidirectional_in_range(
            &av,
            &bv,
            ID,
            POLICY,
            FitSampleRange {
                minimum: 1.0,
                maximum: 0.0
            },
            || false
        ),
        Err(WarpError::Invalid)
    ));
    assert!(matches!(
        verify_photometric_bidirectional_in_range(&av, &bv, ID, POLICY, range, || true),
        Err(WarpError::Cancelled)
    ));
    assert!(matches!(
        verify_photometric_bidirectional_in_range(
            &av,
            &bv,
            ID,
            POLICY,
            FitSampleRange {
                minimum: 0.4,
                maximum: 0.401
            },
            || false
        ),
        Err(WarpError::Fit(_))
    ));
}

#[test]
fn explicit_display_projection_retains_hdr_disagreement_and_alpha_edits() {
    use rrrah_dedup::warp::{FitSampleRange, verify_photometric_display_projection};
    let range = FitSampleRange {
        minimum: 0.0,
        maximum: 1.0,
    };
    let mut source = data();
    source[0] = 4.0;
    let mut displayed = source.clone();
    displayed[0] = 1.0;
    let a = LinearRgbaView::new(5, 4, &source, 20, || false).unwrap();
    let b = LinearRgbaView::new(5, 4, &displayed, 20, || false).unwrap();
    let e = verify_photometric_display_projection(&a, &b, ID, POLICY, range, || false).unwrap();
    assert_eq!(e.projected.evidence.forward.fitted_samples, 19);
    assert_eq!(e.projected.evidence.reverse.fitted_samples, 19);
    assert_eq!(e.projected.evidence.forward.pixels.matched_pixels, 20);
    assert_eq!(e.projected.evidence.reverse.pixels.matched_pixels, 20);
    assert_eq!(e.strict.forward.matched_pixels, 19);
    assert_eq!(e.strict.reverse.matched_pixels, 19);
    assert_eq!(
        e.strict.forward.maximum_channel_error.to_bits(),
        3.0_f64.to_bits()
    );
    assert!(!a.same_pixels(&b, || false).unwrap());
    assert_eq!(
        verify_photometric_display_projection(&a, &b, ID, POLICY, range, || true),
        Err(WarpError::Cancelled)
    );
    displayed[79] = 0.2;
    let edited = LinearRgbaView::new(5, 4, &displayed, 20, || false).unwrap();
    let e = verify_photometric_display_projection(&a, &edited, ID, POLICY, range, || false).unwrap();
    assert!(e.projected.evidence.forward.pixels.matched_pixels < 20);
    assert!(e.projected.evidence.reverse.pixels.matched_pixels < 20);
    assert_eq!(source[0].to_bits(), 4.0_f32.to_bits());
    let saturated = [4.0, 4.0, 4.0, 1.0].repeat(20);
    let flat = LinearRgbaView::new(5, 4, &saturated, 20, || false).unwrap();
    assert!(matches!(
        verify_photometric_display_projection(&flat, &flat, ID, POLICY, range, || false),
        Err(WarpError::Fit(_))
    ));
}

#[test]
fn reflected_gain_offset_preserves_strict_evidence_and_cancel_atomicity() {
    use rrrah_dedup::warp::{verify_reflected_bidirectional, verify_reflected_photometric_bidirectional};
    let source = data();
    let mut target = (0..4)
        .flat_map(|y| (0..5).rev().map(move |x| y * 5 + x))
        .flat_map(|i| {
            let p = &source[i * 4..i * 4 + 4];
            [p[0] * 0.5 + 0.03, p[1] * 0.8 + 0.02, p[2] * 1.2 - 0.01, p[3]]
        })
        .collect::<Vec<_>>();
    let a = LinearRgbaView::new(5, 4, &source, 20, || false).unwrap();
    let b = LinearRgbaView::new(5, 4, &target, 20, || false).unwrap();
    let transform = Transform {
        translation: [4., 0.],
        ..ID
    };
    let strict = verify_reflected_bidirectional(&a, &b, transform, POLICY.residual, || false).unwrap();
    assert!(strict.forward.matched_pixels < 20 && strict.reverse.matched_pixels < 20);
    let calls = std::cell::Cell::new(0);
    let expected = verify_reflected_photometric_bidirectional(&a, &b, transform, POLICY, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(expected.forward.pixels.matched_pixels, 20);
    assert_eq!(expected.reverse.pixels.matched_pixels, 20);
    for (actual, wanted) in expected.forward.gain.into_iter().zip([0.5, 0.8, 1.2]) {
        assert!((actual - wanted).abs() < 1e-6);
    }
    for checkpoint in 1..=calls.get() {
        let current = std::cell::Cell::new(0);
        assert_eq!(
            verify_reflected_photometric_bidirectional(&a, &b, transform, POLICY, || {
                current.set(current.get() + 1);
                current.get() == checkpoint
            }),
            Err(WarpError::Cancelled)
        );
    }
    assert_eq!(
        verify_reflected_photometric_bidirectional(&a, &b, transform, POLICY, || false).unwrap(),
        expected
    );
    target[0] += 0.3;
    let b = LinearRgbaView::new(5, 4, &target, 20, || false).unwrap();
    let edited = verify_reflected_photometric_bidirectional(&a, &b, transform, POLICY, || false).unwrap();
    assert!(edited.forward.pixels.matched_pixels < 20 && edited.reverse.pixels.matched_pixels < 20);
    assert_eq!(
        verify_reflected_photometric_bidirectional(
            &a,
            &b,
            transform,
            PhotometricPolicy {
                residual: WarpPolicy {
                    max_source_pixels: 19,
                    ..POLICY.residual
                },
                ..POLICY
            },
            || false
        ),
        Err(WarpError::Budget)
    );
}

#[test]
fn reflected_range_and_display_keep_hdr_alpha_inverse_and_cancellation_evidence() {
    use rrrah_dedup::warp::{
        FitSampleRange, verify_reflected_photometric_bidirectional_in_range,
        verify_reflected_photometric_display_projection,
    };
    let range = FitSampleRange {
        minimum: 0.,
        maximum: 1.,
    };
    let mut source = data();
    source[0] = 4.;
    // Independent authored mapping: (x,y) -> (3-y,4-x), target 4 by 5.
    let mut target = vec![0.; 80];
    for y in 0..4 {
        for x in 0..5 {
            let from = (y * 5 + x) * 4;
            let to = ((4 - x) * 4 + (3 - y)) * 4;
            target[to..to + 4].copy_from_slice(&source[from..from + 4]);
        }
    }
    target[76] = 1.;
    let a = LinearRgbaView::new(5, 4, &source, 20, || false).unwrap();
    let b = LinearRgbaView::new(4, 5, &target, 20, || false).unwrap();
    let transform = Transform {
        a: 0.,
        b: 1.,
        translation: [3., 4.],
    };
    let raw = verify_reflected_photometric_bidirectional_in_range(&a, &b, transform, POLICY, range, || false)
        .unwrap();
    assert_eq!(raw.evidence.forward.fitted_samples, 19);
    assert_eq!(raw.evidence.reverse.fitted_samples, 19);
    assert_eq!(raw.evidence.forward.pixels.matched_pixels, 19);
    assert_eq!(raw.evidence.reverse.pixels.matched_pixels, 19);
    let calls = std::cell::Cell::new(0);
    let projected = verify_reflected_photometric_display_projection(&a, &b, transform, POLICY, range, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(projected.projected.evidence.forward.pixels.matched_pixels, 20);
    assert_eq!(projected.projected.evidence.reverse.pixels.matched_pixels, 20);
    assert_eq!(projected.strict.forward.matched_pixels, 19);
    assert_eq!(projected.strict.reverse.matched_pixels, 19);
    assert!((projected.strict.forward.maximum_channel_error - 3.).abs() < 1e-9);
    for checkpoint in 1..=calls.get() {
        let count = std::cell::Cell::new(0);
        assert_eq!(
            verify_reflected_photometric_display_projection(&a, &b, transform, POLICY, range, || {
                count.set(count.get() + 1);
                count.get() == checkpoint
            }),
            Err(WarpError::Cancelled)
        );
    }
    assert_eq!(
        verify_reflected_photometric_display_projection(&a, &b, transform, POLICY, range, || false).unwrap(),
        projected
    );
    let mut altered = target.clone();
    altered[3] = 0.2;
    let edited = LinearRgbaView::new(4, 5, &altered, 20, || false).unwrap();
    let result =
        verify_reflected_photometric_display_projection(&a, &edited, transform, POLICY, range, || false)
            .unwrap();
    assert!(result.projected.evidence.forward.pixels.matched_pixels < 20);
    assert!(result.projected.evidence.reverse.pixels.matched_pixels < 20);
    let invalid = FitSampleRange {
        minimum: 1.,
        maximum: 0.,
    };
    assert_eq!(
        verify_reflected_photometric_display_projection(&a, &b, transform, POLICY, invalid, || false),
        Err(WarpError::Invalid)
    );
    let small = PhotometricPolicy {
        residual: WarpPolicy {
            max_source_pixels: 19,
            ..POLICY.residual
        },
        ..POLICY
    };
    assert_eq!(
        verify_reflected_photometric_display_projection(&a, &b, transform, small, range, || false),
        Err(WarpError::Budget)
    );
}
