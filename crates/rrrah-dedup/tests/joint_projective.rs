use rrrah_dedup::geometry::{
    Correspondence, GeometryError, GeometryPolicy, JointProjectivePolicy, ProjectiveSamplingPolicy,
    verify_projective_sampled_for_domains, verify_projective_sampled_joint_for_domains,
};
mod data_204100 {
    include!("fixtures/joint-geometry-204100.bits.rs");
    pub fn points() -> &'static [[u64; 4]] {
        POINTS
    }
    pub fn legacy() -> ([[u64; 3]; 3], &'static [usize], u64) {
        (LEGACY_TRANSFORM, LEGACY_INLIERS, LEGACY_ERROR_BITS)
    }
}
mod data_211900 {
    include!("fixtures/joint-geometry-211900.bits.rs");
    pub fn points() -> &'static [[u64; 4]] {
        POINTS
    }
    pub fn legacy() -> ([[u64; 3]; 3], &'static [usize], u64) {
        (LEGACY_TRANSFORM, LEGACY_INLIERS, LEGACY_ERROR_BITS)
    }
}
fn points(group: u32) -> Vec<Correspondence> {
    let raw = if group == 204100 {
        data_204100::points()
    } else {
        data_211900::points()
    };
    raw.iter()
        .map(|p| Correspondence {
            source: [f64::from_bits(p[0]), f64::from_bits(p[1])],
            target: [f64::from_bits(p[2]), f64::from_bits(p[3])],
        })
        .collect()
}
fn legacy(group: u32) -> ([[u64; 3]; 3], &'static [usize], u64) {
    if group == 204100 {
        data_204100::legacy()
    } else {
        data_211900::legacy()
    }
}
fn policy(n: usize) -> (JointProjectivePolicy, ProjectiveSamplingPolicy) {
    let sampling = ProjectiveSamplingPolicy {
        trials: 4096,
        seed: 0x1234abcd,
    };
    (
        JointProjectivePolicy {
            target: GeometryPolicy {
                tolerance: 2.,
                min_inliers: 10,
                max_points: n,
                max_hypotheses: 4096,
            },
            source_tolerance: 0.5,
            maximum_projection_checks: 2 * n as u64 * (sampling.trials + 1),
        },
        sampling,
    )
}
fn residual(h: rrrah_dedup::geometry::ProjectiveTransform, a: [f64; 2], b: [f64; 2]) -> f64 {
    h.apply(a)
        .map_or(f64::INFINITY, |p| (p[0] - b[0]).hypot(p[1] - b[1]))
}
#[test]
fn real_reverse_points_select_joint_support_instead_of_target_only_winner() {
    for (group, minimum) in [(204100, 14), (211900, 11)] {
        let points = points(group);
        let (p, s) = policy(points.len());
        let old =
            verify_projective_sampled_for_domains(&points, p.target, s, (640, 480), (2560, 1920), || false)
                .unwrap()
                .unwrap();
        let (matrix, indices, error) = legacy(group);
        assert_eq!(old.transform.matrix.map(|r| r.map(f64::to_bits)), matrix);
        assert_eq!(old.inliers, indices);
        assert_eq!(old.squared_error.to_bits(), error);
        let inv = old.transform.inverse().unwrap();
        let joint = points
            .iter()
            .filter(|q| {
                residual(old.transform, q.source, q.target) <= 2. && residual(inv, q.target, q.source) <= 0.5
            })
            .count();
        assert!(joint < 10, "baseline {group}: {joint}");
        let e =
            verify_projective_sampled_joint_for_domains(&points, p, s, (640, 480), (2560, 1920), || false)
                .unwrap()
                .unwrap();
        assert!(e.inliers.len() >= minimum, "{group}: {}", e.inliers.len());
        assert_eq!(e.hypotheses, 4096);
        let inv = e.transform.inverse().unwrap();
        let expected: Vec<_> = points
            .iter()
            .enumerate()
            .filter_map(|(i, q)| {
                (residual(e.transform, q.source, q.target) <= 2. && residual(inv, q.target, q.source) <= 0.5)
                    .then_some(i)
            })
            .collect();
        assert_eq!(e.inliers, expected);
        assert!(e.normalized_squared_error.is_finite());
    }
}
#[test]
fn joint_projection_caps_overflow_invalid_and_cancellation_refuse() {
    let points = points(204100);
    let (p, s) = policy(points.len());
    let run =
        |p, s| verify_projective_sampled_joint_for_domains(&points, p, s, (640, 480), (2560, 1920), || false);
    let mut low = p;
    low.maximum_projection_checks -= 1;
    assert!(matches!(run(low, s), Err(GeometryError::Budget)));
    let mut huge = p;
    huge.target.max_points = usize::MAX;
    assert!(matches!(run(huge, s), Err(GeometryError::Budget)));
    let mut huge_s = s;
    huge_s.trials = u64::MAX;
    assert!(matches!(run(p, huge_s), Err(GeometryError::Budget)));
    for tolerance in [0., -1., f64::NAN, f64::INFINITY, 1e-300, 1e300] {
        let mut invalid = p;
        invalid.source_tolerance = tolerance;
        assert!(matches!(run(invalid, s), Err(GeometryError::Invalid)));
    }
    assert!(matches!(
        verify_projective_sampled_joint_for_domains(&points, low, huge_s, (0, 0), (0, 0), || true),
        Err(GeometryError::Cancelled)
    ));
    assert!(matches!(
        verify_projective_sampled_joint_for_domains(&points, p, s, (0, 480), (2560, 1920), || false),
        Err(GeometryError::Invalid)
    ));
    let mut duplicate = points.clone();
    duplicate[1].source = duplicate[0].source;
    assert!(matches!(
        verify_projective_sampled_joint_for_domains(&duplicate, p, s, (640, 480), (2560, 1920), || false),
        Err(GeometryError::Invalid)
    ));
}
#[test]
fn joint_first_middle_last_cancellation_and_retry_are_deterministic() {
    let points = points(204100);
    let (p, s) = policy(points.len());
    let count = std::cell::Cell::new(0);
    let expected =
        verify_projective_sampled_joint_for_domains(&points, p, s, (640, 480), (2560, 1920), || {
            count.set(count.get() + 1);
            false
        })
        .unwrap()
        .unwrap();
    let last = count.get();
    for at in [1, last / 2, last] {
        count.set(0);
        assert!(matches!(
            verify_projective_sampled_joint_for_domains(&points, p, s, (640, 480), (2560, 1920), || {
                count.set(count.get() + 1);
                count.get() == at
            }),
            Err(GeometryError::Cancelled)
        ));
    }
    let retry =
        verify_projective_sampled_joint_for_domains(&points, p, s, (640, 480), (2560, 1920), || false)
            .unwrap()
            .unwrap();
    assert_eq!(retry.transform.matrix, expected.transform.matrix);
    assert_eq!(retry.inliers, expected.inliers);
    assert_eq!(retry.normalized_squared_error, expected.normalized_squared_error);
}

#[test]
fn real_207700_recovers_with_explicit_larger_sampling_and_projection_admission() {
    mod fixture {
        include!("fixtures/joint-geometry-207700.bits.rs");
    }
    let points: Vec<_> = fixture::POINTS
        .iter()
        .map(|p| Correspondence {
            source: [f64::from_bits(p[0]), f64::from_bits(p[1])],
            target: [f64::from_bits(p[2]), f64::from_bits(p[3])],
        })
        .collect();
    let (mut p, mut sampling) = policy(points.len());
    assert!(
        verify_projective_sampled_joint_for_domains(&points, p, sampling, (640, 480), (2560, 1920), || false)
            .unwrap()
            .is_none()
    );
    sampling.trials = 1_000_000;
    p.target.max_hypotheses = sampling.trials;
    assert!(matches!(
        verify_projective_sampled_joint_for_domains(&points, p, sampling, (640, 480), (2560, 1920), || false),
        Err(GeometryError::Budget)
    ));
    p.maximum_projection_checks = 2 * points.len() as u64 * (sampling.trials + 1);
    let e =
        verify_projective_sampled_joint_for_domains(&points, p, sampling, (640, 480), (2560, 1920), || false)
            .unwrap()
            .unwrap();
    assert_eq!(e.inliers.len(), 11);
    let inverse = e.transform.inverse().unwrap();
    for i in e.inliers {
        let q = points[i];
        assert!(residual(e.transform, q.source, q.target) <= 2.);
        assert!(residual(inverse, q.target, q.source) <= 0.5);
    }
}
