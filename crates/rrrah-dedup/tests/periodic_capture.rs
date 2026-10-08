use rrrah_dedup::{
    geometry::Transform,
    linear::LinearRgbaView,
    warp::{
        FilterPolicy, PhotometricPolicy, WarpError, WarpPolicy, verify_filtered_photometric_bidirectional,
        verify_photometric_bidirectional,
    },
};

// Analytic ramps plus zero-mean periodic capture bands. This isolates the
// pixel stage with supplied identity geometry; it is not a camera-copy oracle.
#[test]
fn periodic_capture_bands_keep_strict_evidence_and_reject_broad_content_changes() {
    const SIDE: usize = 65;
    let base: Vec<f32> = (0..SIDE)
        .flat_map(|y| {
            (0..SIDE).flat_map(move |x| {
                [
                    0.2 + x as f32 / 160.,
                    0.2 + y as f32 / 160.,
                    0.2 + (x + y) as f32 / 320.,
                    1.,
                ]
            })
        })
        .collect();
    let source = LinearRgbaView::new(65, 65, &base, 4225, || false).unwrap();
    let geometry = Transform {
        a: 1.,
        b: 0.,
        translation: [0.; 2],
    };
    let policy = PhotometricPolicy {
        residual: WarpPolicy {
            tolerance: 0.03,
            max_source_pixels: 8450,
        },
        minimum_samples: 1000,
        minimum_variance: 1e-5,
        minimum_gain: 0.2,
        maximum_gain: 5.,
        maximum_offset: 0.1,
    };
    for period in [3, 5, 7] {
        for phase in 0..period {
            for horizontal in [false, true] {
                let mut captured = base.clone();
                for y in 0..SIDE {
                    for x in 0..SIDE {
                        let coordinate = if horizontal { y } else { x };
                        let noise = if (coordinate + phase) % period == 0 {
                            0.12
                        } else {
                            -0.12 / (period - 1) as f32
                        };
                        for channel in 0..3 {
                            captured[(y * SIDE + x) * 4 + channel] += noise;
                        }
                    }
                }
                let target = LinearRgbaView::new(65, 65, &captured, 4225, || false).unwrap();
                match verify_photometric_bidirectional(&source, &target, geometry, policy, || false) {
                    Ok(strict) => assert!(
                        strict.forward.pixels.matched_pixels * 10 < strict.forward.pixels.compared_pixels * 9
                    ),
                    Err(WarpError::Fit(_)) => {} // Explicit bounded-fit refusal is retained.
                    Err(error) => panic!("unexpected strict-stage error: {error:?}"),
                }
                // A period-wide box contains each phase exactly once, making
                // the band sum zero independently of the fitting algorithm.
                let filter = FilterPolicy {
                    radius: (period / 2) as u32,
                    max_sample_pairs: 8_000_000,
                };
                let evidence = verify_filtered_photometric_bidirectional(
                    &source,
                    &target,
                    geometry,
                    policy,
                    filter,
                    || false,
                )
                .unwrap();
                for direction in [&evidence.evidence.forward, &evidence.evidence.reverse] {
                    assert_eq!(direction.pixels.matched_pixels, direction.pixels.compared_pixels);
                    assert!(direction.pixels.compared_pixels >= 3481);
                    for channel in 0..3 {
                        assert!((direction.gain[channel] - 1.).abs() < 1e-5);
                        assert!(direction.offset[channel].abs() < 1e-5);
                    }
                }
                // The same 15-wide window for every period avoids knowing
                // the band period. For period 7, its remaining zero-mean
                // band amplitude is bounded by 0.12/15 before fitting.
                let fixed = FilterPolicy {
                    radius: 7,
                    max_sample_pairs: 8_000_000,
                };
                let fixed_evidence = verify_filtered_photometric_bidirectional(
                    &source,
                    &target,
                    geometry,
                    policy,
                    fixed,
                    || false,
                )
                .unwrap();
                for direction in [&fixed_evidence.evidence.forward, &fixed_evidence.evidence.reverse] {
                    assert_eq!(direction.pixels.compared_pixels, 2601);
                    assert_eq!(direction.pixels.matched_pixels, 2601);
                    assert!(direction.pixels.compared_pixels >= 1000);
                    assert!(
                        direction.pixels.compared_pixels as f64 / direction.pixels.source_pixels as f64
                            >= 0.3
                    );
                }
                // A broad opaque edit survives the same filter; periodic
                // suppression must not erase the decision for this negative.
                for y in 0..SIDE {
                    for x in 0..SIDE / 2 {
                        captured[(y * SIDE + x) * 4 + 2] += 0.3;
                    }
                }
                let edited = LinearRgbaView::new(65, 65, &captured, 4225, || false).unwrap();
                for negative_filter in [filter, fixed] {
                    match verify_filtered_photometric_bidirectional(
                        &source,
                        &edited,
                        geometry,
                        policy,
                        negative_filter,
                        || false,
                    ) {
                        Ok(negative) => {
                            for direction in [&negative.evidence.forward, &negative.evidence.reverse] {
                                assert!(
                                    direction.pixels.matched_pixels * 10
                                        < direction.pixels.compared_pixels * 9
                                );
                            }
                        }
                        Err(WarpError::Fit(_)) => {}
                        Err(error) => panic!("unexpected negative-stage error: {error:?}"),
                    }
                }
            }
        }
    }
}
