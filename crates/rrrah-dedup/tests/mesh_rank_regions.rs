#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::ProjectiveTransform,
    linear::LinearRgbaView,
    mesh_grid::{ProjectiveGridPolicy, build_projective_grid},
    mesh_rank::compare_mesh_rank_region,
    rank_region::{RankRegionError, RankRegionPolicy, compare_filtered_rank_region},
};
use std::cell::Cell;
fn model() -> ProjectiveTransform {
    ProjectiveTransform {
        matrix: [[1., 0., 2.], [0., 1., 1.], [0.005, 0., 1.]],
    }
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
fn policy() -> RankRegionPolicy {
    RankRegionPolicy {
        radius: 2,
        minimum_contrast: 1e-4,
        minimum_pairs: 10,
        maximum_sites: 1024,
        maximum_pixel_reads: 5000000,
    }
}
#[test]
fn selected_centers_keep_whole_image_window_context_and_source_clip() {
    let data = pixels();
    let a = LinearRgbaView::new(32, 32, &data, 1024, || false).unwrap();
    let budget = MemoryBudget::new(100000);
    let grid = build_projective_grid(
        model(),
        (32, 32),
        (32, 32),
        [0, 0, 32, 32],
        ProjectiveGridPolicy { maximum_sites: 1024 },
        &budget,
        || false,
    )
    .unwrap();
    for src in [[0, 0, 32, 32], [9, 8, 12, 12]] {
        for radius in [0, 1, 3] {
            let direct =
                compare_filtered_rank_region(&a, &a, model(), src, [10, 10, 8, 8], policy(), radius, || {
                    false
                })
                .unwrap();
            let cached = compare_mesh_rank_region(
                &a,
                &a,
                &grid,
                src,
                [10, 10, 8, 8],
                policy(),
                radius,
                &budget,
                || false,
            )
            .unwrap();
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
            assert_eq!(cached.sites, 64);
            if src[0] == 0 {
                assert_eq!(cached.valid_sites, 64);
            } else {
                assert!(cached.valid_sites > 0 && cached.valid_sites < 64);
            }
            assert_eq!(cached.pixel_reads, 5120);
            assert_eq!(budget.used(), 1024 * 16);
        }
    }
    drop(grid);
    assert_eq!(budget.used(), 0);
}
#[test]
fn every_observed_cancel_and_full_atlas_budget_refuse_without_scratch_leaks() {
    let data = pixels();
    let a = LinearRgbaView::new(32, 32, &data, 1024, || false).unwrap();
    let budget = MemoryBudget::new(100000);
    let grid = build_projective_grid(
        model(),
        (32, 32),
        (32, 32),
        [7, 7, 16, 16],
        ProjectiveGridPolicy { maximum_sites: 256 },
        &budget,
        || false,
    )
    .unwrap();
    let retained = budget.used();
    let mut p = policy();
    p.maximum_sites = 256;
    p.maximum_pixel_reads = 1280;
    let calls = Cell::new(0);
    compare_mesh_rank_region(
        &a,
        &a,
        &grid,
        [0, 0, 32, 32],
        [10, 10, 8, 8],
        p,
        1,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    for checkpoint in 1..=calls.get() {
        let c = Cell::new(0);
        assert!(matches!(
            compare_mesh_rank_region(
                &a,
                &a,
                &grid,
                [0, 0, 32, 32],
                [10, 10, 8, 8],
                p,
                1,
                &budget,
                || {
                    c.set(c.get() + 1);
                    c.get() == checkpoint
                }
            ),
            Err(RankRegionError::Cancelled)
        ));
        assert_eq!(budget.used(), retained);
    }
    p.maximum_sites = 255;
    assert!(matches!(
        compare_mesh_rank_region(
            &a,
            &a,
            &grid,
            [0, 0, 32, 32],
            [10, 10, 8, 8],
            p,
            1,
            &budget,
            || false
        ),
        Err(RankRegionError::Budget)
    ));
    p.maximum_sites = 256;
    p.maximum_pixel_reads = 1279;
    assert!(matches!(
        compare_mesh_rank_region(
            &a,
            &a,
            &grid,
            [0, 0, 32, 32],
            [10, 10, 8, 8],
            p,
            1,
            &budget,
            || false
        ),
        Err(RankRegionError::Budget)
    ));
    p.maximum_pixel_reads = 1280;
    assert!(matches!(
        compare_mesh_rank_region(
            &a,
            &a,
            &grid,
            [0, 0, 32, 32],
            [10, 10, 8, 8],
            p,
            1,
            &MemoryBudget::new(17 * 17 * 24 - 1),
            || false
        ),
        Err(RankRegionError::Budget)
    ));
    drop(grid);
    assert_eq!(budget.used(), 0);
}
#[test]
fn centers_outside_atlas_invalid_rectangles_and_unsupported_context_are_explicit() {
    let mut data = pixels();
    let budget = MemoryBudget::new(100000);
    let grid = build_projective_grid(
        model(),
        (32, 32),
        (32, 32),
        [0, 0, 32, 32],
        ProjectiveGridPolicy { maximum_sites: 1024 },
        &budget,
        || false,
    )
    .unwrap();
    let a = LinearRgbaView::new(32, 32, &data, 1024, || false).unwrap();
    for (src, dst) in [
        ([0, 0, 0, 32], [10, 10, 8, 8]),
        ([0, 0, 32, 32], [30, 30, 8, 8]),
        ([u32::MAX, 0, 32, 32], [10, 10, 8, 8]),
    ] {
        assert!(matches!(
            compare_mesh_rank_region(&a, &a, &grid, src, dst, policy(), 1, &budget, || false),
            Err(RankRegionError::Invalid)
        ));
    }
    let small = build_projective_grid(
        model(),
        (32, 32),
        (32, 32),
        [8, 8, 16, 16],
        ProjectiveGridPolicy { maximum_sites: 256 },
        &budget,
        || false,
    )
    .unwrap();
    assert!(matches!(
        compare_mesh_rank_region(
            &a,
            &a,
            &small,
            [0, 0, 32, 32],
            [7, 10, 8, 8],
            policy(),
            1,
            &budget,
            || false
        ),
        Err(RankRegionError::Invalid)
    ));
    drop(small);
    data[(2 * 32 + 4) * 4 + 3] = 0.5;
    let b = LinearRgbaView::new(32, 32, &data, 1024, || false).unwrap();
    assert!(matches!(
        compare_mesh_rank_region(
            &b,
            &b,
            &grid,
            [0, 0, 32, 32],
            [10, 10, 8, 8],
            policy(),
            1,
            &budget,
            || false
        ),
        Err(RankRegionError::Invalid)
    ));
    drop(grid);
    assert_eq!(budget.used(), 0);
}
