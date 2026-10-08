#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::ProjectiveTransform,
    linear::LinearRgbaView,
    mesh_grid::{ProjectiveGridPolicy, build_projective_grid},
    mesh_rank::compare_mesh_rank,
    piecewise_warp::PiecewiseError,
    rank_region::{RankRegionPolicy, compare_filtered_rank_region},
};
use std::cell::Cell;
fn identity() -> ProjectiveTransform {
    ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    }
}
#[test]
fn coordinates_match_independent_inverse_and_holes_retain_shared_credit() {
    let m = ProjectiveTransform {
        matrix: [[1., 0., 3.], [0., 1., 2.], [0.01, 0., 1.]],
    };
    let budget = MemoryBudget::new(20000);
    let grid = build_projective_grid(
        m,
        (24, 24),
        (24, 24),
        [2, 1, 20, 20],
        ProjectiveGridPolicy { maximum_sites: 400 },
        &budget,
        || false,
    )
    .unwrap();
    let mut covered = 0;
    for y in 1..21 {
        for x in 2..22 {
            let sx = (f64::from(x) - 3.) / (1. - 0.01 * f64::from(x));
            let sy = f64::from(y) * (1. + 0.01 * sx) - 2.;
            if sx >= 0. && sy >= 0. && sx < 24. && sy < 24. {
                let p = grid.coordinate(x, y).unwrap();
                assert!((p[0] - sx).abs() < 1e-11 && (p[1] - sy).abs() < 1e-11);
                covered += 1;
            } else {
                assert!(grid.coordinate(x, y).is_none());
            }
        }
    }
    assert_eq!(grid.covered_sites(), covered);
    assert!(grid.coordinate(0, 0).is_none());
    assert_eq!(budget.used(), 400 * 16);
    let clone = grid.clone();
    drop(grid);
    assert_eq!(budget.used(), 400 * 16);
    drop(clone);
    assert_eq!(budget.used(), 0);
}
#[test]
fn summed_rank_counts_match_direct_projective_windows() {
    let data: Vec<f32> = (0..24)
        .flat_map(|y| {
            (0..24).flat_map(move |x| {
                let v = 0.2 + ((x * 13 + y * 7) % 29) as f32 / 50.;
                [v, v, v, 1.]
            })
        })
        .collect();
    let inverted: Vec<f32> = data
        .chunks_exact(4)
        .flat_map(|p| [1. - p[0], 1. - p[1], 1. - p[2], 1.])
        .collect();
    let a = LinearRgbaView::new(24, 24, &data, 576, || false).unwrap();
    let budget = MemoryBudget::new(64000);
    for model in [
        identity(),
        ProjectiveTransform {
            matrix: [[1., 0., 2.], [0., 1., 1.], [0.005, 0., 1.]],
        },
    ] {
        let grid = build_projective_grid(
            model,
            (24, 24),
            (24, 24),
            [0, 0, 24, 24],
            ProjectiveGridPolicy { maximum_sites: 576 },
            &budget,
            || false,
        )
        .unwrap();
        let policy = RankRegionPolicy {
            radius: 2,
            minimum_contrast: 1e-4,
            minimum_pairs: 100,
            maximum_sites: 576,
            maximum_pixel_reads: 2000000,
        };
        for pixels in [&data, &inverted] {
            let b = LinearRgbaView::new(24, 24, pixels, 576, || false).unwrap();
            for radius in [0, 1, 3] {
                let direct = compare_filtered_rank_region(
                    &a,
                    &b,
                    model,
                    [0, 0, 24, 24],
                    [0, 0, 24, 24],
                    policy,
                    radius,
                    || false,
                )
                .unwrap();
                let cached = compare_mesh_rank(&a, &b, &grid, policy, radius, &budget, || false).unwrap();
                assert_eq!(
                    (
                        cached.sites,
                        cached.valid_sites,
                        cached.informative_pairs,
                        cached.agreeing_pairs
                    ),
                    (
                        direct.sites,
                        direct.valid_sites,
                        direct.informative_pairs,
                        direct.agreeing_pairs
                    )
                );
                assert_eq!(budget.used(), 576 * 16);
            }
        }
        drop(grid);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn cancellation_and_prepaid_limits_leave_no_atlas() {
    let budget = MemoryBudget::new(1024);
    let calls = Cell::new(0);
    let run = |cancel: &dyn Fn() -> bool| {
        build_projective_grid(
            identity(),
            (16, 16),
            (16, 16),
            [0, 0, 8, 8],
            ProjectiveGridPolicy { maximum_sites: 64 },
            &budget,
            cancel,
        )
    };
    drop(
        run(&|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    assert_eq!(budget.used(), 0);
    for checkpoint in 1..=calls.get() {
        let c = Cell::new(0);
        assert!(matches!(
            run(&|| {
                c.set(c.get() + 1);
                c.get() == checkpoint
            }),
            Err(PiecewiseError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    assert!(matches!(
        build_projective_grid(
            identity(),
            (16, 16),
            (16, 16),
            [0, 0, 8, 8],
            ProjectiveGridPolicy { maximum_sites: 63 },
            &budget,
            || false
        ),
        Err(PiecewiseError::Budget)
    ));
    assert!(matches!(
        build_projective_grid(
            identity(),
            (16, 16),
            (16, 16),
            [0, 0, 8, 8],
            ProjectiveGridPolicy { maximum_sites: 64 },
            &MemoryBudget::new(1023),
            || false
        ),
        Err(PiecewiseError::Budget)
    ));
    let horizon = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [1., 0., -5.]],
    };
    assert!(matches!(
        build_projective_grid(
            horizon,
            (16, 16),
            (16, 16),
            [0, 0, 8, 8],
            ProjectiveGridPolicy { maximum_sites: 64 },
            &budget,
            || false
        ),
        Err(PiecewiseError::Invalid)
    ));
    assert!(matches!(
        build_projective_grid(
            identity(),
            (16, 16),
            (16, 16),
            [u32::MAX, 0, 8, 8],
            ProjectiveGridPolicy { maximum_sites: 64 },
            &budget,
            || false
        ),
        Err(PiecewiseError::Invalid)
    ));
}
