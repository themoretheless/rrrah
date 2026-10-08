#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::{Correspondence, ProjectiveTransform},
    linear::LinearRgbaView,
    mesh_grid::{MeshDirection, MeshGridPolicy, build_mesh_grid},
    mesh_rank::compare_mesh_rank,
    piecewise_warp::{PiecewisePolicy, PiecewiseWarp},
    rank_region::{RankRegionError, RankRegionPolicy, compare_filtered_rank_region},
};
use std::cell::Cell;
fn grid() -> rrrah_dedup::mesh_grid::MeshGrid {
    let points: Vec<_> = [[0., 0.], [15., 0.], [15., 15.], [0., 15.]]
        .into_iter()
        .map(|p| Correspondence { source: p, target: p })
        .collect();
    let budget = MemoryBudget::new(10000);
    let mesh = PiecewiseWarp::from_triangles(
        &points,
        &[[0, 1, 2], [0, 2, 3]],
        (16, 16),
        (16, 16),
        PiecewisePolicy {
            maximum_points: 4,
            maximum_triangles: 2,
            maximum_pair_tests: 8,
            minimum_twice_area: 1e-6,
        },
        &budget,
        || false,
    )
    .unwrap();
    build_mesh_grid(
        &mesh,
        [0, 0, 16, 16],
        MeshDirection::TargetToSource,
        MeshGridPolicy {
            maximum_triangles: 2,
            maximum_sites: 256,
            maximum_face_sites: 512,
        },
        &budget,
        || false,
    )
    .unwrap()
}
fn pixels(invert: bool) -> Vec<f32> {
    (0..16)
        .flat_map(|y| {
            (0..16).flat_map(move |x| {
                let v = 0.2 + ((x * 13 + y * 7) % 29) as f32 / 50.;
                let v = if invert { 1. - v } else { v };
                [v, v, v, 1.]
            })
        })
        .collect()
}
fn policy() -> RankRegionPolicy {
    RankRegionPolicy {
        radius: 2,
        minimum_contrast: 1e-4,
        minimum_pairs: 100,
        maximum_sites: 256,
        maximum_pixel_reads: 1000000,
    }
}
#[test]
fn summed_mesh_windows_match_direct_projective_windows_and_reject_inversion() {
    let grid = grid();
    let data = pixels(false);
    let inverted = pixels(true);
    let a = LinearRgbaView::new(16, 16, &data, 256, || false).unwrap();
    let budget = MemoryBudget::new(10000);
    for samples in [&data, &inverted] {
        let b = LinearRgbaView::new(16, 16, samples, 256, || false).unwrap();
        for radius in [0, 1, 2] {
            let actual = compare_mesh_rank(&a, &b, &grid, policy(), radius, &budget, || false).unwrap();
            let reference = compare_filtered_rank_region(
                &a,
                &b,
                ProjectiveTransform {
                    matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                },
                [0, 0, 16, 16],
                [0, 0, 16, 16],
                policy(),
                radius,
                || false,
            )
            .unwrap();
            assert_eq!(actual.valid_sites, reference.valid_sites);
            assert_eq!(actual.informative_pairs, reference.informative_pairs);
            assert_eq!(actual.agreeing_pairs, reference.agreeing_pairs);
            assert_eq!(actual.pixel_reads, 1280);
            assert_eq!(budget.used(), 0);
        }
    }
}
#[test]
fn cancellation_memory_reads_and_overflow_refuse_without_retaining_scratch() {
    let grid = grid();
    let data = pixels(false);
    let a = LinearRgbaView::new(16, 16, &data, 256, || false).unwrap();
    let budget = MemoryBudget::new(6936);
    let calls = Cell::new(0);
    compare_mesh_rank(&a, &a, &grid, policy(), 1, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    for checkpoint in 1..=total {
        let c = Cell::new(0);
        assert!(matches!(
            compare_mesh_rank(&a, &a, &grid, policy(), 1, &budget, || {
                c.set(c.get() + 1);
                c.get() == checkpoint
            }),
            Err(RankRegionError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    assert!(matches!(
        compare_mesh_rank(&a, &a, &grid, policy(), 1, &MemoryBudget::new(6935), || false),
        Err(RankRegionError::Budget)
    ));
    let mut short = policy();
    short.maximum_pixel_reads = 1279;
    assert!(matches!(
        compare_mesh_rank(&a, &a, &grid, short, 1, &budget, || false),
        Err(RankRegionError::Budget)
    ));
    assert!(matches!(
        compare_mesh_rank(&a, &a, &grid, policy(), u32::MAX, &budget, || false),
        Err(RankRegionError::Budget)
    ));
}
