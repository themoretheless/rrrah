use rrrah_dedup::{
    geometry::Transform,
    linear::LinearRgbaView,
    warp::{
        PhotometricPolicy, WarpError, WarpPolicy,
        verify_photometric_white_anchored_display_projection as verify,
    },
};
use std::cell::Cell;

const ID: Transform = Transform {
    a: 1.0,
    b: 0.0,
    translation: [0.0; 2],
};
const POLICY: PhotometricPolicy = PhotometricPolicy {
    residual: WarpPolicy {
        tolerance: 1e-6,
        max_source_pixels: 100,
    },
    minimum_samples: 8,
    minimum_variance: 1e-5,
    minimum_gain: 0.8,
    maximum_gain: 1.2,
    maximum_offset: 0.2,
};

fn data() -> Vec<f32> {
    (0_u8..20)
        .flat_map(|i| {
            let v = f32::from(i) / 24.0 + 0.05;
            [v, v * v, 0.9 - v * 0.7, 1.0]
        })
        .collect()
}

#[test]
fn analytic_white_anchor_retains_strict_difference_and_visible_edits() {
    let a = data();
    let gains = [0.98_f32, 1.01, 0.99];
    let mut b: Vec<f32> = a
        .chunks_exact(4)
        .flat_map(|p| {
            [
                gains[0] * p[0] + 1.0 - gains[0],
                gains[1] * p[1] + 1.0 - gains[1],
                gains[2] * p[2] + 1.0 - gains[2],
                1.0,
            ]
        })
        .collect();
    let av = LinearRgbaView::new(5, 4, &a, 20, || false).unwrap();
    let bv = LinearRgbaView::new(5, 4, &b, 20, || false).unwrap();
    let e = verify(&av, &bv, ID, POLICY, || false).unwrap();
    for (model, expected) in [
        (e.projected.evidence.forward, gains.map(f64::from)),
        (e.projected.evidence.reverse, gains.map(|g| 1.0 / f64::from(g))),
    ] {
        assert_eq!(model.pixels.matched_pixels, 20);
        for (channel, gain) in expected.into_iter().enumerate() {
            assert!((model.gain[channel] - gain).abs() < 1e-6);
            assert_eq!(model.offset[channel], 1.0 - model.gain[channel]);
        }
    }
    assert!(e.strict.forward.matched_pixels < 20);
    b[24] += 0.1;
    let bv = LinearRgbaView::new(5, 4, &b, 20, || false).unwrap();
    let edited = verify(&av, &bv, ID, POLICY, || false).unwrap();
    assert!(edited.projected.evidence.forward.pixels.matched_pixels < 20);
    assert!(edited.projected.evidence.reverse.pixels.matched_pixels < 20);
}

#[test]
fn cancellation_at_every_checkpoint_discards_result_and_retry_is_stable() {
    let a = data();
    let av = LinearRgbaView::new(5, 4, &a, 20, || false).unwrap();
    let count = Cell::new(0);
    let baseline = verify(&av, &av, ID, POLICY, || {
        count.set(count.get() + 1);
        false
    })
    .unwrap();
    for stop in 1..=count.get() {
        let calls = Cell::new(0);
        assert_eq!(
            verify(&av, &av, ID, POLICY, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(WarpError::Cancelled),
            "checkpoint {stop}"
        );
    }
    assert_eq!(verify(&av, &av, ID, POLICY, || false).unwrap(), baseline);
}

#[test]
fn flat_work_limits_and_infeasible_anchor_bounds_refuse() {
    let a = data();
    let av = LinearRgbaView::new(5, 4, &a, 20, || false).unwrap();
    let flat = [0.5, 0.5, 0.5, 1.0].repeat(20);
    let fv = LinearRgbaView::new(5, 4, &flat, 20, || false).unwrap();
    assert!(matches!(
        verify(&fv, &fv, ID, POLICY, || false),
        Err(WarpError::Fit(_))
    ));
    assert_eq!(
        verify(
            &av,
            &av,
            ID,
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
    assert!(matches!(
        verify(
            &av,
            &av,
            ID,
            PhotometricPolicy {
                minimum_gain: 1.15,
                maximum_gain: 1.2,
                maximum_offset: 0.01,
                ..POLICY
            },
            || false
        ),
        Err(WarpError::Fit(_))
    ));
}

#[test]
fn highlight_projection_keeps_strict_hdr_and_alpha_evidence() {
    let mut a = data();
    let mut b = a.clone();
    a[76..80].copy_from_slice(&[1.5, 1.5, 1.5, 1.0]);
    b[76..80].copy_from_slice(&[1.0, 1.0, 1.0, 1.0]);
    let av = LinearRgbaView::new(5, 4, &a, 20, || false).unwrap();
    let bv = LinearRgbaView::new(5, 4, &b, 20, || false).unwrap();
    let evidence = verify(&av, &bv, ID, POLICY, || false).unwrap();
    for model in [
        evidence.projected.evidence.forward,
        evidence.projected.evidence.reverse,
    ] {
        assert_eq!(model.fitted_samples, 19);
        assert_eq!(model.pixels.compared_pixels, 20);
        assert_eq!(model.pixels.matched_pixels, 20);
    }
    assert_eq!(evidence.strict.forward.matched_pixels, 19);
    assert_eq!(evidence.strict.reverse.matched_pixels, 19);
    b[79] = 0.5;
    let bv = LinearRgbaView::new(5, 4, &b, 20, || false).unwrap();
    let evidence = verify(&av, &bv, ID, POLICY, || false).unwrap();
    assert_eq!(evidence.projected.evidence.forward.pixels.matched_pixels, 19);
    assert_eq!(evidence.projected.evidence.reverse.pixels.matched_pixels, 19);
}

#[test]
fn feasible_gain_boundary_is_explicit_and_retains_residual_failures() {
    let a = data();
    let av = LinearRgbaView::new(5, 4, &a, 20, || false).unwrap();
    let evidence = verify(
        &av,
        &av,
        ID,
        PhotometricPolicy {
            minimum_gain: 1.05,
            maximum_gain: 1.1,
            maximum_offset: 0.1,
            ..POLICY
        },
        || false,
    )
    .unwrap();
    for model in [
        evidence.projected.evidence.forward,
        evidence.projected.evidence.reverse,
    ] {
        assert_eq!(model.gain, [1.05; 3]);
        assert_eq!(model.constrained_channels, [true; 3]);
        assert!(model.offset.iter().all(|offset| offset.abs() <= 0.1));
        assert_eq!(model.pixels.matched_pixels, 0);
    }
    assert_eq!(evidence.strict.forward.matched_pixels, 20);
}

#[test]
fn reflected_white_anchor_fits_both_directions_retains_edits_and_cancels() {
    use rrrah_dedup::warp::verify_reflected_photometric_white_anchored_display_projection as reflected;
    let flip = Transform {
        translation: [4.0, 0.0],
        ..ID
    };
    let a = data();
    let gains = [0.98_f32, 1.01, 0.99];
    let mut b = vec![0.0_f32; a.len()];
    for y in 0..4 {
        for x in 0..5 {
            let src = (y * 5 + x) * 4;
            let dst = (y * 5 + 4 - x) * 4;
            for channel in 0..3 {
                b[dst + channel] = gains[channel] * a[src + channel] + 1.0 - gains[channel];
            }
            b[dst + 3] = 1.0;
        }
    }
    let av = LinearRgbaView::new(5, 4, &a, 20, || false).unwrap();
    let bv = LinearRgbaView::new(5, 4, &b, 20, || false).unwrap();
    let calls = Cell::new(0);
    let e = reflected(&av, &bv, flip, POLICY, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    for (fit, reverse) in [
        (&e.projected.evidence.forward, false),
        (&e.projected.evidence.reverse, true),
    ] {
        assert_eq!(fit.pixels.matched_pixels, 20);
        for channel in 0..3 {
            let expected = if reverse {
                1.0 / f64::from(gains[channel])
            } else {
                f64::from(gains[channel])
            };
            assert!((fit.gain[channel] - expected).abs() < 1e-6);
        }
    }
    assert!(e.strict.forward.matched_pixels < 20);
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        assert!(matches!(
            reflected(&av, &bv, flip, POLICY, || {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            }),
            Err(WarpError::Cancelled)
        ));
    }
    assert_eq!(reflected(&av, &bv, flip, POLICY, || false).unwrap(), e);
    b[3] = 0.0;
    let edited = LinearRgbaView::new(5, 4, &b, 20, || false).unwrap();
    let changed = reflected(&av, &edited, flip, POLICY, || false).unwrap();
    assert_eq!(changed.projected.evidence.forward.pixels.matched_pixels, 19);
    assert_eq!(changed.projected.evidence.reverse.pixels.matched_pixels, 19);
}
