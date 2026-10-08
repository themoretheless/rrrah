#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::Correspondence,
    linear::LinearRgbaView,
    local_rank::{LocalRankError, LocalRankPolicy},
    piecewise_local_rank::{PiecewiseLocalRankPolicy, compare_piecewise_local_rank_region},
    piecewise_warp::{PiecewisePolicy, PiecewiseWarp},
    rank_region::RankRegionPolicy,
};
use std::cell::Cell;
fn mesh(budget: &MemoryBudget) -> PiecewiseWarp {
    let points =
        [[2., 2.], [26., 2.], [26., 26.], [2., 26.]].map(|p| Correspondence { source: p, target: p });
    PiecewiseWarp::from_triangles(
        &points,
        &[[0, 1, 2], [0, 2, 3]],
        (32, 32),
        (32, 32),
        PiecewisePolicy {
            maximum_points: 4,
            maximum_triangles: 2,
            maximum_pair_tests: 10,
            minimum_twice_area: 1e-6,
        },
        budget,
        || false,
    )
    .unwrap()
}
fn points() -> Vec<Correspondence> {
    (0..10)
        .map(|i| {
            let p = [8. + f64::from(i % 5) * 3., 8. + f64::from(i / 5) * 5.];
            Correspondence { source: p, target: p }
        })
        .collect()
}
fn pixels() -> Vec<f32> {
    (0..32)
        .flat_map(|y| {
            (0..32).flat_map(move |x| {
                let v = 0.2 + ((x * 13 + y * 7) % 29) as f32 / 50.;
                [v, v, v, 1.]
            })
        })
        .collect()
}
fn policy() -> PiecewiseLocalRankPolicy {
    PiecewiseLocalRankPolicy {
        local: LocalRankPolicy {
            rank: RankRegionPolicy {
                radius: 2,
                minimum_contrast: 0.001,
                minimum_pairs: 10,
                maximum_sites: 800,
                maximum_pixel_reads: 4000,
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
        maximum_triangles: 2,
        maximum_triangle_tests: 40,
        maximum_face_sites: 1600,
    }
}
#[test]
fn combined_piecewise_requires_mesh_consistent_regional_witnesses() {
    let budget = MemoryBudget::new(100000);
    let mesh = mesh(&budget);
    let held = budget.used();
    let pixels = pixels();
    let view = LinearRgbaView::new(32, 32, &pixels, 1024, || false).unwrap();
    let wrong_frame = LinearRgbaView::new(16, 64, &pixels, 1024, || false).unwrap();
    assert_eq!(
        compare_piecewise_local_rank_region(
            &view,
            &wrong_frame,
            &mesh,
            &points(),
            [4, 4, 8, 8],
            [4, 4, 8, 8],
            policy(),
            &budget,
            || false
        ),
        Err(LocalRankError::Invalid)
    );
    assert_eq!(budget.used(), held);
    let points = points();
    let rect = [6, 6, 16, 16];
    let e = compare_piecewise_local_rank_region(
        &view,
        &view,
        &mesh,
        &points,
        rect,
        rect,
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    assert!(e.supported);
    assert_eq!(e.regional_witnesses, 10);
    assert_eq!(budget.used(), held);
    let mut inconsistent = points.clone();
    inconsistent[0].target[1] += 1.;
    assert_eq!(
        compare_piecewise_local_rank_region(
            &view,
            &view,
            &mesh,
            &inconsistent,
            rect,
            rect,
            policy(),
            &budget,
            || false
        ),
        Err(LocalRankError::InsufficientGeometry)
    );
    let mut aliases = points;
    aliases[9] = aliases[0];
    assert_eq!(
        compare_piecewise_local_rank_region(
            &view,
            &view,
            &mesh,
            &aliases,
            rect,
            rect,
            policy(),
            &budget,
            || false
        ),
        Err(LocalRankError::Invalid)
    );
    assert_eq!(budget.used(), held);
    drop(mesh);
    assert_eq!(budget.used(), 0);
}
#[test]
fn piecewise_aggregate_work_and_every_cancel_release_scratch() {
    let budget = MemoryBudget::new(100000);
    let mesh = mesh(&budget);
    let held = budget.used();
    let pixels = pixels();
    let view = LinearRgbaView::new(32, 32, &pixels, 1024, || false).unwrap();
    let points = points();
    let rect = [6, 6, 16, 16];
    let calls = Cell::new(0);
    compare_piecewise_local_rank_region(
        &view,
        &view,
        &mesh,
        &points,
        rect,
        rect,
        policy(),
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in 1..=calls.get() {
        let n = Cell::new(0);
        assert!(
            compare_piecewise_local_rank_region(
                &view,
                &view,
                &mesh,
                &points,
                rect,
                rect,
                policy(),
                &budget,
                || {
                    n.set(n.get() + 1);
                    n.get() == stop
                }
            )
            .is_err()
        );
        assert_eq!(budget.used(), held);
    }
    for kind in 0..5 {
        let mut p = policy();
        match kind {
            0 => p.local.maximum_point_checks -= 1,
            1 => p.maximum_triangle_tests -= 1,
            2 => p.maximum_face_sites -= 1,
            3 => p.local.rank.maximum_sites -= 1,
            _ => p.local.rank.maximum_pixel_reads -= 1,
        };
        assert_eq!(
            compare_piecewise_local_rank_region(&view, &view, &mesh, &points, rect, rect, p, &budget, || {
                false
            }),
            Err(LocalRankError::Budget)
        );
        assert_eq!(budget.used(), held);
    }
    let tiny = MemoryBudget::new(1);
    assert_eq!(
        compare_piecewise_local_rank_region(
            &view,
            &view,
            &mesh,
            &points,
            rect,
            rect,
            policy(),
            &tiny,
            || false
        ),
        Err(LocalRankError::Budget)
    );
    assert_eq!(tiny.used(), 0);
}
