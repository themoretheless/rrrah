use rrrah_dedup::geometry::{
    Correspondence, GeometryError, GeometryPolicy, ProjectiveTransform, verify_projective,
    verify_projective_for_domains,
};
use std::cell::Cell;
fn policy(n: usize) -> GeometryPolicy {
    GeometryPolicy {
        tolerance: 1e-6,
        min_inliers: n,
        max_points: 32,
        max_hypotheses: 10000,
    }
}
fn corners_valid(t: ProjectiveTransform) -> bool {
    [t, t.inverse().unwrap()].iter().all(|t| {
        let d = [[0., 0.], [63., 0.], [0., 63.], [63., 63.]]
            .map(|[x, y]| t.matrix[2][0] * x + t.matrix[2][1] * y + t.matrix[2][2]);
        d.iter()
            .all(|v| v.is_finite() && *v != 0. && v.is_sign_positive() == d[0].is_sign_positive())
    })
}
fn identity() -> Vec<Correspondence> {
    [[2., 2.], [20., 2.], [2., 20.], [20., 20.]]
        .into_iter()
        .map(|p| Correspondence { source: p, target: p })
        .collect()
}
#[test]
fn weaker_eligible_model_is_not_hidden_by_stronger_horizon_model() {
    let mut points: Vec<_> = [
        [2., 2.],
        [4., 8.],
        [6., 3.],
        [8., 12.],
        [10., 5.],
        [12., 9.],
        [5., 11.],
        [11., 2.],
    ]
    .into_iter()
    .map(|[x, y]| Correspondence {
        source: [x, y],
        target: [x / (1. - x / 20.), y / (1. - x / 20.)],
    })
    .collect();
    points.extend(
        [
            [30., 30.],
            [45., 30.],
            [30., 45.],
            [45., 45.],
            [35., 40.],
            [42., 36.],
        ]
        .into_iter()
        .map(|p| Correspondence { source: p, target: p }),
    );
    let unconstrained = verify_projective(&points, policy(6), || false).unwrap().unwrap();
    assert!(unconstrained.inliers.len() >= 8);
    assert!(!corners_valid(unconstrained.transform));
    let admitted = verify_projective_for_domains(&points, policy(6), (64, 64), (64, 64), || false)
        .unwrap()
        .unwrap();
    assert!(corners_valid(admitted.transform));
    assert_eq!(admitted.inliers, (8..14).collect::<Vec<_>>());
    for p in &points[8..] {
        let q = admitted.transform.apply(p.source).unwrap();
        assert!((q[0] - p.target[0]).abs() < 1e-6 && (q[1] - p.target[1]).abs() < 1e-6);
    }
}
#[test]
fn every_cancel_and_one_short_work_limit_refuse_partial_results() {
    let p = identity();
    let calls = Cell::new(0);
    verify_projective_for_domains(&p, policy(4), (64, 64), (64, 64), || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap()
    .unwrap();
    for checkpoint in 1..=calls.get() {
        let c = Cell::new(0);
        assert!(matches!(
            verify_projective_for_domains(&p, policy(4), (64, 64), (64, 64), || {
                c.set(c.get() + 1);
                c.get() == checkpoint
            }),
            Err(GeometryError::Cancelled)
        ));
    }
    let mut p = p;
    p.push(Correspondence {
        source: [12., 13.],
        target: [12., 13.],
    });
    let mut limit = policy(4);
    limit.max_hypotheses = 4;
    assert!(matches!(
        verify_projective_for_domains(&p, limit, (64, 64), (64, 64), || false),
        Err(GeometryError::Budget)
    ));
}
#[test]
fn horizon_only_points_and_zero_domains_are_explicit() {
    let points = [[2., 2.], [4., 8.], [6., 3.], [8., 12.]].map(|[x, y]| Correspondence {
        source: [x, y],
        target: [x / (1. - x / 20.), y / (1. - x / 20.)],
    });
    assert!(
        verify_projective_for_domains(&points, policy(4), (64, 64), (64, 64), || false)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        verify_projective_for_domains(&identity(), policy(4), (0, 64), (64, 64), || false),
        Err(GeometryError::Invalid)
    ));
}
