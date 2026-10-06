use rrrah_dedup::geometry::{Correspondence, GeometryError, GeometryPolicy, verify_similarity};
const POLICY: GeometryPolicy = GeometryPolicy {
    tolerance: 0.01,
    min_inliers: 4,
    max_points: 100,
    max_hypotheses: 1000,
};
#[test]
fn one_shot_cancel_never_publishes_partial_geometry_and_retry_recovers_support() {
    let points = [[0., 0.], [3., 0.], [0., 2.], [3., 2.]].map(|source| Correspondence {
        source,
        target: [
            1.2 * source[0] - 0.3 * source[1] + 7.,
            0.3 * source[0] + 1.2 * source[1] - 4.,
        ],
    });
    let checks = std::cell::Cell::new(0);
    let expected = verify_similarity(&points, POLICY, || {
        checks.set(checks.get() + 1);
        false
    })
    .unwrap()
    .unwrap();
    for checkpoint in 1..=checks.get() {
        let current = std::cell::Cell::new(0);
        assert_eq!(
            verify_similarity(&points, POLICY, || {
                current.set(current.get() + 1);
                current.get() == checkpoint
            }),
            Err(GeometryError::Cancelled)
        );
    }
    assert_eq!(
        verify_similarity(&points, POLICY, || false).unwrap(),
        Some(expected)
    );
}
#[test]
fn scaled_rotated_crop_translation_rejects_watermark_outliers() {
    let mut points: Vec<_> = [[0.0, 0.0], [4.0, 0.0], [0.0, 3.0], [4.0, 3.0]]
        .map(|source| Correspondence {
            source,
            target: [10.0 - source[1] * 2.0, 20.0 + source[0] * 2.0],
        })
        .to_vec();
    points.push(Correspondence {
        source: [2.0, 1.0],
        target: [999.0, 999.0],
    });
    let evidence = verify_similarity(&points, POLICY, || false).unwrap().unwrap();
    assert_eq!(evidence.inliers, [0, 1, 2, 3]);
    let p = evidence.transform.apply([1.0, 2.0]);
    assert!((p[0] - 6.0).abs() < 1e-9 && (p[1] - 22.0).abs() < 1e-9);
    assert_eq!(
        verify_similarity(
            &points,
            GeometryPolicy {
                max_hypotheses: 0,
                ..POLICY
            },
            || false
        ),
        Err(GeometryError::Budget)
    );
    assert_eq!(
        verify_similarity(&points, POLICY, || true),
        Err(GeometryError::Cancelled)
    );
}
#[test]
fn duplicate_and_collinear_support_cannot_inflate_evidence() {
    let point = Correspondence {
        source: [0.0, 0.0],
        target: [1.0, 1.0],
    };
    assert_eq!(
        verify_similarity(&[point, point], POLICY, || false),
        Err(GeometryError::Invalid)
    );
    let points = (0_u8..4)
        .map(|x| Correspondence {
            source: [f64::from(x), 0.0],
            target: [f64::from(x) * 2.0, 1.0],
        })
        .collect::<Vec<_>>();
    assert!(verify_similarity(&points, POLICY, || false).unwrap().is_none());
}

#[test]
fn noisy_corners_refine_to_independently_known_least_squares_solution() {
    // The xy perturbation is orthogonal to constant, x and y on this symmetric grid.
    // Hence its closed-form optimum is the authored transform, residual 4*(.01+.04).
    let points = [[-1., -1.], [1., -1.], [-1., 1.], [1., 1.]].map(|source| {
        let noise = source[0] * source[1];
        Correspondence {
            source,
            target: [
                1.2 * source[0] - 0.3 * source[1] + 2. + 0.1 * noise,
                0.3 * source[0] + 1.2 * source[1] - 4. - 0.2 * noise,
            ],
        }
    });
    let e = verify_similarity(
        &points,
        GeometryPolicy {
            tolerance: 0.5,
            max_hypotheses: 6,
            ..POLICY
        },
        || false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(e.inliers, [0, 1, 2, 3]);
    for (a, b) in [
        e.transform.a,
        e.transform.b,
        e.transform.translation[0],
        e.transform.translation[1],
        e.squared_error,
    ]
    .into_iter()
    .zip([1.2, 0.3, 2., -4., 0.2])
    {
        assert!((a - b).abs() < 1e-12);
    }
}

#[test]
fn degenerate_least_squares_refinement_preserves_prior_hypothesis() {
    let points = [[-1., -1.], [1., -1.], [-1., 1.], [1., 1.]].map(|source| Correspondence {
        source,
        target: [-source[0], source[1]],
    });
    // A loose threshold admits this reflected support to two-point hypotheses;
    // its centered similarity least-squares model has zero scale and is unusable.
    let e = verify_similarity(
        &points,
        GeometryPolicy {
            tolerance: 3.,
            ..POLICY
        },
        || false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(e.inliers.len(), 4);
    assert!(e.transform.a.hypot(e.transform.b) > 0.);
}

#[test]
fn reflected_geometry_keeps_support_original_coordinates_and_cancel_atomicity() {
    use rrrah_dedup::geometry::verify_reflected_similarity;
    let mut points = [[0., 0.], [3., 0.], [0., 2.], [3., 2.]]
        .map(|source| Correspondence {
            source,
            target: [
                -1.2 * source[0] - 0.3 * source[1] + 7.,
                -0.3 * source[0] + 1.2 * source[1] - 4.,
            ],
        })
        .to_vec();
    points.push(Correspondence {
        source: [1., 1.],
        target: [99., 99.],
    });
    assert!(verify_similarity(&points, POLICY, || false).unwrap().is_none());
    let calls = std::cell::Cell::new(0);
    let expected = verify_reflected_similarity(&points, POLICY, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap()
    .unwrap();
    assert_eq!(expected.similarity.inliers, [0, 1, 2, 3]);
    for point in &points[..4] {
        let actual = expected.apply(point.source);
        for (a, b) in actual.into_iter().zip(point.target) {
            assert!((a - b).abs() < 1e-10);
        }
    }
    for checkpoint in 1..=calls.get() {
        let current = std::cell::Cell::new(0);
        assert_eq!(
            verify_reflected_similarity(&points, POLICY, || {
                current.set(current.get() + 1);
                current.get() == checkpoint
            }),
            Err(GeometryError::Cancelled)
        );
    }
    assert_eq!(
        verify_reflected_similarity(&points, POLICY, || false).unwrap(),
        Some(expected)
    );
    assert_eq!(
        verify_reflected_similarity(
            &points,
            GeometryPolicy {
                max_hypotheses: 0,
                ..POLICY
            },
            || false
        ),
        Err(GeometryError::Budget)
    );
}

fn marginal_extra_inlier_points(reflected: bool) -> Vec<Correspondence> {
    let sign = if reflected { -1. } else { 1. };
    let map = |source: [f64; 2]| [100. + sign * source[0], source[1] + 7.];
    let mut points = (0..5)
        .flat_map(|y| (0..5).map(move |x| [f64::from(x) * 10., f64::from(y) * 10.]))
        .map(|source| {
            let mut target = map(source);
            // Four corner localization errors generate a count-winning model
            // that admits the marginal outlier while distorting the exact core.
            if [0., 40.].contains(&source[0]) && [0., 40.].contains(&source[1]) {
                target[0] += sign * if source[0] == 0. { 1. } else { -1. };
            }
            Correspondence { source, target }
        })
        .collect::<Vec<_>>();
    let source = [5., 5.];
    let mut target = map(source);
    target[0] += sign;
    target[1] += 2.;
    points.push(Correspondence { source, target });
    points
}

#[test]
fn capped_residual_candidate_preserves_exact_majority_beside_count_winner() {
    use rrrah_dedup::geometry::{verify_reflected_similarity_candidates, verify_similarity_candidates};
    let policy = GeometryPolicy {
        tolerance: 2.,
        ..POLICY
    };
    for reflected in [false, true] {
        let points = marginal_extra_inlier_points(reflected);
        let candidates = if reflected {
            verify_reflected_similarity_candidates(&points, policy, || false)
                .unwrap()
                .map(|model| model.map(|model| model.similarity))
        } else {
            verify_similarity_candidates(&points, policy, || false).unwrap()
        };
        let primary = candidates[0].as_ref().unwrap();
        let robust = candidates[1].as_ref().expect("a distinct exact-majority model");
        assert_eq!(primary.inliers.len(), 26);
        assert_eq!(robust.inliers.len(), 25);
        // Independent centered normal equations: denominator 10000 and
        // corner x-error covariance -80 yield a=.992 and residual 3.36.
        let expected_x = if reflected { 99.84 } else { 100.16 };
        for (actual, expected) in [
            (robust.squared_error, 3.36),
            (robust.transform.a, 0.992),
            (robust.transform.b, 0.),
            (robust.transform.translation[0], expected_x),
            (robust.transform.translation[1], 7.16),
        ] {
            assert!((actual - expected).abs() < 1e-10, "{actual} versus {expected}");
        }
        if !reflected {
            assert_eq!(
                Some(primary.clone()),
                verify_similarity(&points, policy, || false).unwrap()
            );
        }
    }
}

#[test]
fn alternative_geometry_budget_and_cancellation_never_publish_partial_models() {
    use rrrah_dedup::geometry::{verify_reflected_similarity_candidates, verify_similarity_candidates};
    let points = marginal_extra_inlier_points(false);
    let count = points.len() * (points.len() - 1) / 2;
    let policy = GeometryPolicy {
        tolerance: 2.,
        max_hypotheses: count as u64,
        ..POLICY
    };
    let checks = std::cell::Cell::new(0);
    let expected = verify_similarity_candidates(&points, policy, || {
        checks.set(checks.get() + 1);
        false
    })
    .unwrap();
    for checkpoint in [1, 30, 100, checks.get() / 2, checks.get() - 1, checks.get()] {
        let current = std::cell::Cell::new(0);
        assert_eq!(
            verify_similarity_candidates(&points, policy, || {
                current.set(current.get() + 1);
                current.get() == checkpoint
            }),
            Err(GeometryError::Cancelled)
        );
    }
    assert_eq!(
        verify_similarity_candidates(&points, policy, || false).unwrap(),
        expected
    );
    let too_small = GeometryPolicy {
        max_hypotheses: count as u64 - 1,
        ..policy
    };
    assert_eq!(
        verify_similarity_candidates(&points, too_small, || false),
        Err(GeometryError::Budget)
    );
    assert_eq!(
        verify_reflected_similarity_candidates(&points, too_small, || false),
        Err(GeometryError::Budget)
    );
    let reflected = marginal_extra_inlier_points(true);
    let checks = std::cell::Cell::new(0);
    let expected = verify_reflected_similarity_candidates(&reflected, policy, || {
        checks.set(checks.get() + 1);
        false
    })
    .unwrap();
    for checkpoint in [1, 30, 100, checks.get() / 2, checks.get() - 1, checks.get()] {
        let current = std::cell::Cell::new(0);
        assert_eq!(
            verify_reflected_similarity_candidates(&reflected, policy, || {
                current.set(current.get() + 1);
                current.get() == checkpoint
            }),
            Err(GeometryError::Cancelled)
        );
    }
    assert_eq!(
        verify_reflected_similarity_candidates(&reflected, policy, || false).unwrap(),
        expected
    );
}

#[test]
fn alternative_geometry_keeps_degeneracy_guards_and_deduplicates_identical_models() {
    use rrrah_dedup::geometry::{verify_reflected_similarity_candidates, verify_similarity_candidates};
    let collinear = (0..5)
        .map(|x| Correspondence {
            source: [f64::from(x), 0.],
            target: [f64::from(x) + 7., 3.],
        })
        .collect::<Vec<_>>();
    assert_eq!(
        verify_similarity_candidates(&collinear, POLICY, || false).unwrap(),
        [None, None]
    );
    assert_eq!(
        verify_reflected_similarity_candidates(&collinear, POLICY, || false).unwrap(),
        [None, None]
    );
    let duplicate = [collinear[0], collinear[0]];
    assert_eq!(
        verify_similarity_candidates(&duplicate, POLICY, || false),
        Err(GeometryError::Invalid)
    );
    assert_eq!(
        verify_reflected_similarity_candidates(&duplicate, POLICY, || false),
        Err(GeometryError::Invalid)
    );
    let square = [[0., 0.], [2., 0.], [0., 2.], [2., 2.]].map(|source| Correspondence {
        source,
        target: [source[0] + 7., source[1] - 3.],
    });
    let models = verify_similarity_candidates(&square, POLICY, || false).unwrap();
    assert!(models[0].is_some());
    assert!(models[1].is_none());
}
