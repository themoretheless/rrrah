#![cfg(feature = "raster")]
use rrrah_dedup::{
    geometry::{Correspondence, ProjectiveTransform},
    linear::LinearRgbaView,
    local_rank::{LocalRankError, LocalRankPolicy, compare_local_rank_region},
    rank_region::RankRegionPolicy,
};
use std::cell::Cell;
fn model() -> ProjectiveTransform {
    ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    }
}
fn points() -> Vec<Correspondence> {
    (0..10)
        .map(|i| {
            let p = [8. + f64::from(i % 5) * 3., 8. + f64::from(i / 5) * 5.];
            Correspondence { source: p, target: p }
        })
        .collect()
}
fn policy() -> LocalRankPolicy {
    LocalRankPolicy {
        rank: RankRegionPolicy {
            radius: 2,
            minimum_contrast: 0.001,
            minimum_pairs: 10,
            maximum_sites: 512,
            maximum_pixel_reads: 23040,
        },
        filter_radius: 0,
        minimum_witnesses: 10,
        maximum_points: 20,
        maximum_point_checks: 55,
        minimum_point_separation: 2.,
        target_tolerance: 2.,
        source_tolerance: 2.,
        minimum_coverage: 0.3,
        minimum_agreement: 0.9,
    }
}
fn image(invert: bool) -> Vec<f32> {
    (0..32)
        .flat_map(|y| {
            (0..32).flat_map(move |x| {
                let v = 0.2 + ((x * 13 + y * 7) % 29) as f32 / 50.;
                let v = if invert { 1. - v } else { v };
                [v, v, v, 1.]
            })
        })
        .collect()
}
#[test]
fn bidirectional_support_requires_geometry_and_pixel_order() {
    let a = image(false);
    let b = image(true);
    let av = LinearRgbaView::new(32, 32, &a, 1024, || false).unwrap();
    let bv = LinearRgbaView::new(32, 32, &b, 1024, || false).unwrap();
    let pts = points();
    let rect = [6, 6, 16, 16];
    let same = compare_local_rank_region(&av, &av, model(), &pts, rect, rect, policy(), || false).unwrap();
    assert!(same.supported);
    assert_eq!(same.regional_witnesses, 10);
    assert_eq!(same.forward.agreeing_pairs, same.forward.informative_pairs);
    let opposite =
        compare_local_rank_region(&av, &bv, model(), &pts, rect, rect, policy(), || false).unwrap();
    assert!(!opposite.supported);
    assert_eq!(opposite.forward.agreeing_pairs, 0);
    assert_eq!(
        compare_local_rank_region(&av, &av, model(), &pts[..9], rect, rect, policy(), || false),
        Err(LocalRankError::InsufficientGeometry)
    );
    let mut duplicates = pts.clone();
    duplicates[9] = duplicates[0];
    assert_eq!(
        compare_local_rank_region(&av, &av, model(), &duplicates, rect, rect, policy(), || false),
        Err(LocalRankError::Invalid)
    );
}
#[test]
fn aggregate_work_and_each_cancellation_refuse() {
    let a = image(false);
    let av = LinearRgbaView::new(32, 32, &a, 1024, || false).unwrap();
    let pts = points();
    let rect = [6, 6, 16, 16];
    let calls = Cell::new(0);
    compare_local_rank_region(&av, &av, model(), &pts, rect, rect, policy(), || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let n = Cell::new(0);
        assert!(
            compare_local_rank_region(&av, &av, model(), &pts, rect, rect, policy(), || {
                n.set(n.get() + 1);
                n.get() == stop
            })
            .is_err()
        );
    }
    for kind in 0..3 {
        let mut p = policy();
        match kind {
            0 => p.rank.maximum_sites -= 1,
            1 => p.rank.maximum_pixel_reads -= 1,
            _ => p.maximum_point_checks -= 1,
        };
        assert_eq!(
            compare_local_rank_region(&av, &av, model(), &pts, rect, rect, p, || false),
            Err(LocalRankError::Budget)
        );
    }
}
#[test]
fn inverse_residual_is_required_even_when_forward_error_is_small() {
    let a = image(false);
    let av = LinearRgbaView::new(32, 32, &a, 1024, || false).unwrap();
    let mut pts = points();
    for p in &mut pts {
        p.target = [p.source[0] * 0.1 + 0.5, p.source[1] * 0.1];
    }
    let mut p = policy();
    p.minimum_point_separation = 0.1;
    let m = ProjectiveTransform {
        matrix: [[0.1, 0., 0.], [0., 0.1, 0.], [0., 0., 1.]],
    };
    assert_eq!(
        compare_local_rank_region(
            &av,
            &av,
            m,
            &pts,
            [0, 0, 32, 32],
            [0, 0, 32, 32],
            {
                p.rank.maximum_sites = 2048;
                p.rank.maximum_pixel_reads = 92160;
                p
            },
            || false
        ),
        Err(LocalRankError::InsufficientGeometry)
    );
}
