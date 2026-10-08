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

#[test]
fn unsupported_pixels_uninformative_regions_and_overflow_refuse_explicitly() {
    use rrrah_dedup::rank_region::RankRegionError;
    let data = image(false);
    let view = LinearRgbaView::new(32, 32, &data, 1024, || false).unwrap();
    let pts = points();
    let rect = [6, 6, 16, 16];
    let flat = vec![0.5_f32; 32 * 32 * 4];
    let mut flat = flat;
    for p in flat.chunks_exact_mut(4) {
        p[3] = 1.;
    }
    let flat_view = LinearRgbaView::new(32, 32, &flat, 1024, || false).unwrap();
    assert_eq!(
        compare_local_rank_region(
            &flat_view,
            &flat_view,
            model(),
            &pts,
            rect,
            rect,
            policy(),
            || false
        ),
        Err(LocalRankError::Rank(RankRegionError::Uninformative))
    );
    let mut translucent = data.clone();
    translucent[(8 * 32 + 8) * 4 + 3] = 0.5;
    let alpha = LinearRgbaView::new(32, 32, &translucent, 1024, || false).unwrap();
    assert_eq!(
        compare_local_rank_region(&alpha, &view, model(), &pts, rect, rect, policy(), || false),
        Err(LocalRankError::Rank(RankRegionError::Invalid))
    );
    let mut huge = policy();
    huge.filter_radius = u32::MAX;
    huge.rank.maximum_pixel_reads = u64::MAX;
    assert_eq!(
        compare_local_rank_region(&view, &view, model(), &pts, rect, rect, huge, || false),
        Err(LocalRankError::Budget)
    );
    assert_eq!(
        compare_local_rank_region(
            &view,
            &view,
            model(),
            &pts,
            [u32::MAX, 0, 2, 2],
            rect,
            policy(),
            || false
        ),
        Err(LocalRankError::Invalid)
    );
    let horizon = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [1., 0., -16.]],
    };
    assert_eq!(
        compare_local_rank_region(&view, &view, horizon, &pts, rect, rect, policy(), || false),
        Err(LocalRankError::Invalid)
    );
    let mut invalid = policy();
    invalid.source_tolerance = f64::NAN;
    assert_eq!(
        compare_local_rank_region(&view, &view, model(), &pts, rect, rect, invalid, || false),
        Err(LocalRankError::Invalid)
    );
}

#[test]
fn points_outside_selected_regions_do_not_count_and_aliases_are_not_hidden() {
    let data = image(false);
    let view = LinearRgbaView::new(32, 32, &data, 1024, || false).unwrap();
    let pts = points();
    assert_eq!(
        compare_local_rank_region(
            &view,
            &view,
            model(),
            &pts,
            [20, 20, 8, 8],
            [20, 20, 8, 8],
            policy(),
            || false
        ),
        Err(LocalRankError::InsufficientGeometry)
    );
    let mut pts = pts;
    pts.extend(
        [Correspondence {
            source: [28., 28.],
            target: [28., 28.],
        }; 2],
    );
    let mut p = policy();
    p.maximum_point_checks = 78;
    assert_eq!(
        compare_local_rank_region(
            &view,
            &view,
            model(),
            &pts,
            [6, 6, 16, 16],
            [6, 6, 16, 16],
            p,
            || false
        ),
        Err(LocalRankError::Invalid)
    );
}
