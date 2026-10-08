#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    anchor_rank::{AnchorRankPolicy, compare_anchor_rank},
    geometry::{Correspondence, ProjectiveTransform},
    linear::LinearRgbaView,
    local_rank::{LocalRankError, LocalRankPolicy},
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
            let p = [8. + f64::from(i % 5) * 8., 8. + f64::from(i / 5) * 16.];
            Correspondence { source: p, target: p }
        })
        .collect()
}
fn policy() -> AnchorRankPolicy {
    AnchorRankPolicy {
        local: LocalRankPolicy {
            rank: RankRegionPolicy {
                radius: 2,
                minimum_contrast: 0.001,
                minimum_pairs: 100,
                maximum_sites: 980,
                maximum_pixel_reads: 4900,
            },
            filter_radius: 0,
            minimum_witnesses: 10,
            maximum_points: 10,
            maximum_point_checks: 55,
            minimum_point_separation: 2.,
            target_tolerance: 0.01,
            source_tolerance: 0.01,
            minimum_coverage: 0.3,
            minimum_agreement: 0.9,
        },
        window_radius: 1,
        maximum_selection_checks: 145,
    }
}
fn image(invert: bool) -> Vec<f32> {
    (0..64)
        .flat_map(|y| {
            (0..64).flat_map(move |x| {
                let v = 0.2 + ((x * 13 + y * 7) % 29) as f32 / 50.;
                let v = if invert { 1. - v } else { v };
                [v, v, v, 1.]
            })
        })
        .collect()
}
#[test]
fn geometry_selected_windows_aggregate_information_and_reject_inverted_or_flat_pixels() {
    let a = image(false);
    let b = image(true);
    let av = LinearRgbaView::new(64, 64, &a, 4096, || false).unwrap();
    let bv = LinearRgbaView::new(64, 64, &b, 4096, || false).unwrap();
    let budget = MemoryBudget::new(100000);
    let pts = points();
    let e = compare_anchor_rank(&av, &av, model(), &pts, policy(), &budget, || false).unwrap();
    assert!(e.supported);
    assert_eq!((e.forward_anchors, e.reverse_anchors), (10, 10));
    assert_eq!(e.forward.sites, 90);
    assert_eq!(e.forward.valid_sites, 90);
    assert_eq!(e.forward.agreeing_pairs, e.forward.informative_pairs);
    assert_eq!(e.forward.pixel_reads + e.reverse.pixel_reads, 4900);
    let opposite = compare_anchor_rank(&av, &bv, model(), &pts, policy(), &budget, || false).unwrap();
    assert!(!opposite.supported);
    assert_eq!(opposite.forward.agreeing_pairs, 0);
    let flat: Vec<_> = (0..4096).flat_map(|_| [0.5, 0.5, 0.5, 1.]).collect();
    let fv = LinearRgbaView::new(64, 64, &flat, 4096, || false).unwrap();
    let e = compare_anchor_rank(&fv, &fv, model(), &pts, policy(), &budget, || false).unwrap();
    assert!(!e.supported);
    assert_eq!(e.forward.informative_pairs, 0);
    assert_eq!(e.forward.valid_sites, 90);
    assert_eq!(budget.used(), 0);
}
#[test]
fn overlap_bounds_aliases_and_inconsistent_geometry_are_not_hidden() {
    let a = image(false);
    let av = LinearRgbaView::new(64, 64, &a, 4096, || false).unwrap();
    let budget = MemoryBudget::new(100000);
    let mut p = policy();
    p.window_radius = 2;
    p.local.rank.maximum_sites = 1620;
    p.local.rank.maximum_pixel_reads = 8100;
    let mut pts = points();
    pts[1] = Correspondence {
        source: [11., 8.],
        target: [11., 8.],
    };
    assert_eq!(
        compare_anchor_rank(&av, &av, model(), &pts, p, &budget, || false),
        Err(LocalRankError::InsufficientGeometry)
    );
    let mut pts = points();
    pts[0] = Correspondence {
        source: [1., 8.],
        target: [1., 8.],
    };
    assert_eq!(
        compare_anchor_rank(&av, &av, model(), &pts, p, &budget, || false),
        Err(LocalRankError::InsufficientGeometry)
    );
    let mut pts = points();
    pts[9] = pts[0];
    assert_eq!(
        compare_anchor_rank(&av, &av, model(), &pts, p, &budget, || false),
        Err(LocalRankError::Invalid)
    );
    let mut pts = points();
    pts[0].target[0] += 1.;
    assert_eq!(
        compare_anchor_rank(&av, &av, model(), &pts, p, &budget, || false),
        Err(LocalRankError::InsufficientGeometry)
    );
    assert_eq!(budget.used(), 0);
}
#[test]
fn exact_aggregate_limits_and_every_observed_cancellation_release_credits() {
    let a = image(false);
    let av = LinearRgbaView::new(64, 64, &a, 4096, || false).unwrap();
    let budget = MemoryBudget::new(100000);
    let pts = points();
    let calls = Cell::new(0);
    compare_anchor_rank(&av, &av, model(), &pts, policy(), &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let n = Cell::new(0);
        assert!(
            compare_anchor_rank(&av, &av, model(), &pts, policy(), &budget, || {
                n.set(n.get() + 1);
                n.get() == stop
            })
            .is_err()
        );
        assert_eq!(budget.used(), 0);
    }
    for kind in 0..4 {
        let mut p = policy();
        match kind {
            0 => p.maximum_selection_checks -= 1,
            1 => p.local.maximum_point_checks -= 1,
            2 => p.local.rank.maximum_sites -= 1,
            _ => p.local.rank.maximum_pixel_reads -= 1,
        };
        assert_eq!(
            compare_anchor_rank(&av, &av, model(), &pts, p, &budget, || false),
            Err(LocalRankError::Budget)
        );
    }
    let tiny = MemoryBudget::new(1);
    assert_eq!(
        compare_anchor_rank(&av, &av, model(), &pts, policy(), &tiny, || false),
        Err(LocalRankError::Budget)
    );
    assert_eq!(tiny.used(), 0);
}

#[test]
fn independent_textures_reject_even_with_exact_bidirectional_geometry() {
    fn texture(mut state: u32) -> Vec<f32> {
        (0..128 * 128)
            .flat_map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                let v = (state & 65535) as f32 / 65535.;
                [v, v, v, 1.]
            })
            .collect()
    }
    let points: Vec<_> = (0..20)
        .map(|i| {
            let p = [24. + f64::from(i % 5) * 20., 24. + f64::from(i / 5) * 20.];
            Correspondence { source: p, target: p }
        })
        .collect();
    let source = texture(173);
    let av = LinearRgbaView::new(128, 128, &source, 16384, || false).unwrap();
    let budget = MemoryBudget::new(1000000);
    for seed in [19, 31, 53, 97, 211, 307, 503, 701] {
        let target = texture(seed);
        let bv = LinearRgbaView::new(128, 128, &target, 16384, || false).unwrap();
        for filter_radius in [0, 8] {
            let mut p = policy();
            p.window_radius = 2;
            p.local.filter_radius = filter_radius;
            p.local.rank.radius = 8;
            p.local.rank.minimum_contrast = 0.005;
            p.local.rank.minimum_pairs = 1000;
            p.local.rank.maximum_sites = 54760;
            p.local.rank.maximum_pixel_reads = 273800;
            p.local.maximum_points = 20;
            p.local.maximum_point_checks = 210;
            p.maximum_selection_checks = 590;
            let e = compare_anchor_rank(&av, &bv, model(), &points, p, &budget, || false).unwrap();
            assert_eq!((e.forward_anchors, e.reverse_anchors), (20, 20));
            assert_eq!((e.forward.valid_sites, e.reverse.valid_sites), (500, 500));
            assert!(e.forward.informative_pairs >= 1000 && e.reverse.informative_pairs >= 1000);
            assert!(!e.supported, "seed={seed}, filter={filter_radius}: {e:?}");
            assert_eq!(budget.used(), 0);
        }
    }
}
