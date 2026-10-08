#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::{Correspondence, ProjectiveTransform},
    linear::LinearRgbaView,
    local_rank::{
        LocalRankError, LocalRankPolicy, compare_local_rank_region, compare_local_rank_region_cached,
    },
    rank_region::{RankRegionEvidence, RankRegionPolicy},
};
use std::cell::Cell;
fn model(perspective: bool) -> ProjectiveTransform {
    ProjectiveTransform {
        matrix: if perspective {
            [[1., 0., 1.], [0., 1., 1.], [0.005, 0., 1.]]
        } else {
            [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
        },
    }
}
fn points(m: ProjectiveTransform) -> Vec<Correspondence> {
    (0..10)
        .map(|i| {
            let source = [8. + f64::from(i % 5) * 3., 8. + f64::from(i / 5) * 5.];
            Correspondence {
                source,
                target: m.apply(source).unwrap(),
            }
        })
        .collect()
}
fn pixels(invert: bool) -> Vec<f32> {
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
fn policy(radius: u32) -> LocalRankPolicy {
    LocalRankPolicy {
        rank: RankRegionPolicy {
            radius: 2,
            minimum_contrast: 0.001,
            minimum_pairs: 10,
            maximum_sites: 2048,
            maximum_pixel_reads: 10000000,
        },
        filter_radius: radius,
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
fn counts(e: RankRegionEvidence) -> (u64, u64, u64, u64) {
    (e.sites, e.valid_sites, e.informative_pairs, e.agreeing_pairs)
}
#[test]
fn cached_bidirectional_local_support_matches_direct_for_identity_and_perspective() {
    let a = pixels(false);
    let av = LinearRgbaView::new(32, 32, &a, 1024, || false).unwrap();
    let budget = MemoryBudget::new(200000);
    for perspective in [false, true] {
        let m = model(perspective);
        let pts = points(m);
        for invert in [false, true] {
            let b = pixels(invert);
            let bv = LinearRgbaView::new(32, 32, &b, 1024, || false).unwrap();
            for radius in [0, 1, 3] {
                let p = policy(radius);
                let direct =
                    compare_local_rank_region(&av, &bv, m, &pts, [6, 6, 16, 16], [6, 6, 16, 16], p, || false)
                        .unwrap();
                let cached = compare_local_rank_region_cached(
                    &av,
                    &bv,
                    m,
                    &pts,
                    [6, 6, 16, 16],
                    [6, 6, 16, 16],
                    p,
                    &budget,
                    || false,
                )
                .unwrap();
                assert_eq!(counts(direct.forward), counts(cached.forward));
                assert_eq!(counts(direct.reverse), counts(cached.reverse));
                assert_eq!(direct.supported, cached.supported);
                assert_eq!(direct.regional_witnesses, cached.regional_witnesses);
                assert_eq!(budget.used(), 0);
                assert!(
                    cached.forward.pixel_reads + cached.reverse.pixel_reads
                        < direct.forward.pixel_reads + direct.reverse.pixel_reads
                );
            }
        }
    }
}
#[test]
fn cached_aggregate_context_limits_and_every_cancel_release_all_credits() {
    let a = pixels(false);
    let av = LinearRgbaView::new(32, 32, &a, 1024, || false).unwrap();
    let m = model(false);
    let pts = points(m);
    let rect = [6, 6, 16, 16];
    let budget = MemoryBudget::new(200000);
    let retained = budget.try_reserve(71).unwrap();
    let mut p = policy(0);
    p.rank.maximum_sites = 800;
    p.rank.maximum_pixel_reads = 4000;
    let calls = Cell::new(0);
    compare_local_rank_region_cached(&av, &av, m, &pts, rect, rect, p, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(budget.used(), 71);
    for stop in 1..=calls.get() {
        let n = Cell::new(0);
        assert!(
            compare_local_rank_region_cached(&av, &av, m, &pts, rect, rect, p, &budget, || {
                n.set(n.get() + 1);
                n.get() == stop
            })
            .is_err()
        );
        assert_eq!(budget.used(), 71);
    }
    for kind in [0, 1] {
        let mut short = p;
        if kind == 0 {
            short.rank.maximum_sites -= 1
        } else {
            short.rank.maximum_pixel_reads -= 1
        };
        assert_eq!(
            compare_local_rank_region_cached(&av, &av, m, &pts, rect, rect, short, &budget, || false),
            Err(LocalRankError::Budget)
        );
        assert_eq!(budget.used(), 71);
    }
    let tiny = MemoryBudget::new(1);
    assert_eq!(
        compare_local_rank_region_cached(&av, &av, m, &pts, rect, rect, p, &tiny, || false),
        Err(LocalRankError::Budget)
    );
    assert_eq!(tiny.used(), 0);
    drop(retained);
    assert_eq!(budget.used(), 0);
}

#[test]
fn all_1016_original_points_require_prepaid_checks_without_truncation() {
    let data: Vec<f32> = (0..128)
        .flat_map(|y| {
            (0..128).flat_map(move |x| {
                let value = 0.2 + ((x * 13 + y * 7) % 29) as f32 / 50.;
                [value, value, value, 1.]
            })
        })
        .collect();
    let view = LinearRgbaView::new(128, 128, &data, 16384, || false).unwrap();
    let pts: Vec<Correspondence> = (0..1016)
        .map(|i| {
            let point = [4. + f64::from(i % 32) * 3., 4. + f64::from(i / 32) * 3.];
            Correspondence {
                source: point,
                target: point,
            }
        })
        .collect();
    let budget = MemoryBudget::new(200000);
    let rect = [4, 4, 16, 16];
    let mut p = policy(0);
    p.maximum_points = 1000;
    p.maximum_point_checks = 1016 * 1017 / 2;
    assert_eq!(
        compare_local_rank_region_cached(&view, &view, model(false), &pts, rect, rect, p, &budget, || false),
        Err(LocalRankError::Budget)
    );
    p.maximum_points = 1016;
    p.maximum_point_checks -= 1;
    assert_eq!(
        compare_local_rank_region_cached(&view, &view, model(false), &pts, rect, rect, p, &budget, || false),
        Err(LocalRankError::Budget)
    );
    p.maximum_point_checks += 1;
    let evidence =
        compare_local_rank_region_cached(&view, &view, model(false), &pts, rect, rect, p, &budget, || false)
            .unwrap();
    assert_eq!(evidence.regional_witnesses, 36);
    assert!(evidence.supported);
    assert_eq!(budget.used(), 0);
    let mut aliased = pts;
    aliased[1015] = aliased[1014];
    assert_eq!(
        compare_local_rank_region_cached(
            &view,
            &view,
            model(false),
            &aliased,
            rect,
            rect,
            p,
            &budget,
            || false
        ),
        Err(LocalRankError::Invalid)
    );
    assert_eq!(budget.used(), 0);
}
