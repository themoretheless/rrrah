use rrrah_dedup::geometry::{
    Correspondence, GeometryPolicy, ProjectiveSamplingPolicy, ProjectiveTransform, verify_projective_sampled,
    verify_projective_sampled_for_domains,
};
fn policies() -> (GeometryPolicy, ProjectiveSamplingPolicy) {
    (
        GeometryPolicy {
            tolerance: 0.000001,
            min_inliers: 8,
            max_points: 32,
            max_hypotheses: 512,
        },
        ProjectiveSamplingPolicy {
            trials: 512,
            seed: 0x1234abcd,
        },
    )
}
#[test]
fn supported_horizon_model_is_excluded_from_whole_domains() {
    let transform = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0.01, 0., -0.5]],
    };
    let points: Vec<_> = [60., 70., 80., 90.]
        .into_iter()
        .flat_map(|x| {
            [10., 20., 30.].into_iter().map(move |y| {
                let source = [x, y];
                Correspondence {
                    source,
                    target: transform.apply(source).unwrap(),
                }
            })
        })
        .collect();
    let (p, s) = policies();
    assert!(
        verify_projective_sampled(&points, p, s, || false)
            .unwrap()
            .is_some()
    );
    assert!(
        verify_projective_sampled_for_domains(&points, p, s, (100, 100), (1000, 1000), || false)
            .unwrap()
            .is_none()
    );
}
#[test]
fn admissible_model_matches_original_and_cancellation_is_atomic() {
    let points: Vec<_> = [10., 30., 50., 70.]
        .into_iter()
        .flat_map(|x| {
            [10., 30., 50.].into_iter().map(move |y| Correspondence {
                source: [x, y],
                target: [x + 3., y + 4.],
            })
        })
        .collect();
    let (p, s) = policies();
    let expected = verify_projective_sampled(&points, p, s, || false).unwrap();
    let calls = std::cell::Cell::new(0);
    let actual = verify_projective_sampled_for_domains(&points, p, s, (100, 100), (100, 100), || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(actual, expected);
    let total = calls.get();
    for stop in [0, total / 2, total - 1] {
        calls.set(0);
        assert!(matches!(
            verify_projective_sampled_for_domains(&points, p, s, (100, 100), (100, 100), || {
                let n = calls.get();
                calls.set(n + 1);
                n == stop
            }),
            Err(rrrah_dedup::geometry::GeometryError::Cancelled)
        ));
    }
}

#[test]
fn inverse_horizon_is_rejected_even_when_forward_domain_is_valid() {
    let transform = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0.01, 0., 1.]],
    };
    let points: Vec<_> = [10., 30., 50., 70.]
        .into_iter()
        .flat_map(|x| {
            [10., 30., 50.].into_iter().map(move |y| {
                let source = [x, y];
                Correspondence {
                    source,
                    target: transform.apply(source).unwrap(),
                }
            })
        })
        .collect();
    let (p, s) = policies();
    assert!(
        verify_projective_sampled(&points, p, s, || false)
            .unwrap()
            .is_some()
    );
    assert!(
        verify_projective_sampled_for_domains(&points, p, s, (100, 100), (1000, 1000), || false)
            .unwrap()
            .is_none()
    );
}
#[test]
fn valid_alternative_survives_a_stronger_horizon_consensus() {
    let horizon = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0.01, 0., -0.5]],
    };
    let mut points: Vec<_> = [60., 70., 80., 90.]
        .into_iter()
        .flat_map(|x| {
            [10., 20., 30.].into_iter().map(move |y| {
                let source = [x, y];
                Correspondence {
                    source,
                    target: horizon.apply(source).unwrap(),
                }
            })
        })
        .collect();
    points.extend([10., 20., 30., 40.].into_iter().flat_map(|x| {
        [60., 80.].into_iter().map(move |y| Correspondence {
            source: [x, y],
            target: [x + 5., y + 7.],
        })
    }));
    let (mut p, mut s) = policies();
    p.max_hypotheses = 4096;
    s.trials = 4096;
    let original = verify_projective_sampled(&points, p, s, || false)
        .unwrap()
        .unwrap();
    assert!(original.inliers.len() >= 12);
    let qualified = verify_projective_sampled_for_domains(&points, p, s, (100, 100), (1000, 1000), || false)
        .unwrap()
        .unwrap();
    assert_eq!(qualified.inliers.len(), 8);
    for source in [[10., 60.], [40., 80.], [0., 0.], [99., 99.]] {
        let target = qualified.transform.apply(source).unwrap();
        assert!((target[0] - source[0] - 5.).abs() < 1e-7);
        assert!((target[1] - source[1] - 7.).abs() < 1e-7);
    }
}
