#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::Correspondence,
    linear::LinearRgbaView,
    mesh_grid::{MeshDirection, MeshGridPolicy, build_mesh_grid},
    mesh_refine::{MeshRefinePolicy, SeedRefinement, propose_mesh_translations},
    piecewise_warp::{PiecewisePolicy, PiecewiseWarp},
    rank_region::RankRegionError,
};
use std::cell::Cell;
fn grid() -> rrrah_dedup::mesh_grid::MeshGrid {
    let points: Vec<_> = [[0., 0.], [63., 0.], [63., 63.], [0., 63.]]
        .into_iter()
        .map(|p| Correspondence { source: p, target: p })
        .collect();
    let budget = MemoryBudget::new(100000);
    let mesh = PiecewiseWarp::from_triangles(
        &points,
        &[[0, 1, 2], [0, 2, 3]],
        (64, 64),
        (64, 64),
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
        [0, 0, 64, 64],
        MeshDirection::TargetToSource,
        MeshGridPolicy {
            maximum_triangles: 2,
            maximum_sites: 4096,
            maximum_face_sites: 8192,
        },
        &budget,
        || false,
    )
    .unwrap()
}
fn pixels(shift: u32) -> Vec<f32> {
    (0..64)
        .flat_map(|y| {
            (0..64).flat_map(move |x| {
                let x = (x + 64 - shift) % 64;
                let v = 0.2 + ((x * 13 + y * 7) % 29) as f32 / 50.;
                [v, v, v, 1.]
            })
        })
        .collect()
}
fn policy() -> MeshRefinePolicy {
    MeshRefinePolicy {
        offset_radius: 2,
        offset_step: 2,
        minimum_contrast: 0.005,
        minimum_pairs: 64,
        minimum_training_fraction: 0.9,
        minimum_validation_fraction: 0.9,
        maximum_seeds: 2,
        maximum_offsets: 9,
        maximum_pixel_reads: 71114,
    }
}
#[test]
fn known_translation_is_recovered_without_using_validation_for_selection() {
    let grid = grid();
    let original = pixels(0);
    let shifted = pixels(2);
    let source = LinearRgbaView::new(64, 64, &shifted, 4096, || false).unwrap();
    let target = LinearRgbaView::new(64, 64, &original, 4096, || false).unwrap();
    let budget = MemoryBudget::new(100000);
    let result =
        propose_mesh_translations(&source, &target, &grid, &[[32, 32]], policy(), &budget, || false).unwrap();
    let SeedRefinement::Scored(e) = result[0] else {
        panic!("missing evidence")
    };
    assert_eq!(e.offset, [2, 0]);
    assert!(e.eligible_proposal);
    assert_eq!(e.training_agreeing, e.training_pairs);
    assert_eq!(e.validation_agreeing, e.validation_pairs);
    let bytes = std::mem::size_of::<SeedRefinement>() as u64;
    assert_eq!(budget.used(), bytes);
    let clone = result.clone();
    drop(result);
    assert_eq!(budget.used(), bytes);
    drop(clone);
    assert_eq!(budget.used(), 0);
}
#[test]
fn cancellation_and_prepaid_read_limits_leave_no_partial_proposals() {
    let grid = grid();
    let original = pixels(0);
    let view = LinearRgbaView::new(64, 64, &original, 4096, || false).unwrap();
    let budget = MemoryBudget::new(100000);
    let mut p = policy();
    p.offset_radius = 0;
    p.maximum_offsets = 1;
    let calls = Cell::new(0);
    let result = propose_mesh_translations(&view, &view, &grid, &[[32, 32]], p, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    drop(result);
    let total = calls.get();
    for checkpoint in 1..=total {
        let c = Cell::new(0);
        assert!(matches!(
            propose_mesh_translations(&view, &view, &grid, &[[32, 32]], p, &budget, || {
                c.set(c.get() + 1);
                c.get() == checkpoint
            }),
            Err(RankRegionError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let mut short = policy();
    short.maximum_pixel_reads = 35556;
    assert!(matches!(
        propose_mesh_translations(&view, &view, &grid, &[[32, 32]], short, &budget, || false),
        Err(RankRegionError::Budget)
    ));
    assert!(matches!(
        propose_mesh_translations(
            &view,
            &view,
            &grid,
            &[[32, 32]],
            policy(),
            &MemoryBudget::new(1),
            || false
        ),
        Err(RankRegionError::Budget)
    ));
    let result = propose_mesh_translations(
        &view,
        &view,
        &grid,
        &[[0, 0], [32, 32]],
        policy(),
        &budget,
        || false,
    )
    .unwrap();
    assert!(matches!(result[0], SeedRefinement::Unavailable));
    drop(result);
    assert_eq!(budget.used(), 0);
}
#[test]
fn flat_support_and_invalid_visible_samples_are_explicit() {
    let grid = grid();
    let flat = vec![0.25f32; 64 * 64 * 4];
    let mut opaque = flat.clone();
    for p in opaque.chunks_exact_mut(4) {
        p[3] = 1.;
    }
    let view = LinearRgbaView::new(64, 64, &opaque, 4096, || false).unwrap();
    let budget = MemoryBudget::new(100000);
    let result =
        propose_mesh_translations(&view, &view, &grid, &[[32, 32]], policy(), &budget, || false).unwrap();
    assert!(matches!(result[0], SeedRefinement::Uninformative));
    drop(result);
    assert_eq!(budget.used(), 0);
    let transparent = LinearRgbaView::new(64, 64, &flat, 4096, || false).unwrap();
    assert!(matches!(
        propose_mesh_translations(&transparent, &view, &grid, &[[32, 32]], policy(), &budget, || {
            false
        }),
        Err(RankRegionError::Invalid)
    ));
    assert_eq!(budget.used(), 0);
    let mut invalid = policy();
    invalid.offset_radius = 1;
    assert!(matches!(
        propose_mesh_translations(&view, &view, &grid, &[[32, 32]], invalid, &budget, || false),
        Err(RankRegionError::Invalid)
    ));
    let mut weak = policy();
    weak.maximum_offsets = 8;
    assert!(matches!(
        propose_mesh_translations(&view, &view, &grid, &[[32, 32]], weak, &budget, || false),
        Err(RankRegionError::Budget)
    ));
}
