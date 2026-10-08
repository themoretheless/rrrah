#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::Correspondence,
    mesh_grid::{MeshDirection, MeshGridPolicy, build_mesh_grid},
    piecewise_warp::{PiecewiseError, PiecewisePolicy, PiecewiseWarp},
};
use std::cell::Cell;
fn mesh() -> PiecewiseWarp {
    let p: Vec<_> = [[0., 0.], [10., 0.], [10., 10.], [0., 10.]]
        .into_iter()
        .map(|target| Correspondence {
            source: [target[0] + 0.5, target[1] + 0.25],
            target,
        })
        .collect();
    PiecewiseWarp::from_triangles(
        &p,
        &[[0, 1, 2], [0, 2, 3]],
        (32, 32),
        (32, 32),
        PiecewisePolicy {
            maximum_points: 4,
            maximum_triangles: 2,
            maximum_pair_tests: 8,
            minimum_twice_area: 1e-6,
        },
        &MemoryBudget::new(10000),
        || false,
    )
    .unwrap()
}
fn policy() -> MeshGridPolicy {
    MeshGridPolicy {
        maximum_triangles: 2,
        maximum_sites: 256,
        maximum_face_sites: 242,
    }
}
#[test]
fn both_directions_match_point_queries_and_preserve_holes_and_credit() {
    let mesh = mesh();
    let budget = MemoryBudget::new(4096);
    for direction in [MeshDirection::TargetToSource, MeshDirection::SourceToTarget] {
        let grid = build_mesh_grid(&mesh, [0, 0, 16, 16], direction, policy(), &budget, || false).unwrap();
        assert_eq!(grid.region(), [0, 0, 16, 16]);
        let mut covered = 0;
        for y in 0..16 {
            for x in 0..16 {
                let p = [x as f64, y as f64];
                let expected = match direction {
                    MeshDirection::TargetToSource => mesh.map_target(p, 2, || false),
                    MeshDirection::SourceToTarget => mesh.map_source(p, 2, || false),
                }
                .unwrap();
                let actual = grid.coordinate(x, y);
                assert_eq!(actual.is_some(), expected.is_some());
                if let (Some(a), Some(b)) = (actual, expected) {
                    covered += 1;
                    assert!(a.iter().zip(b).all(|(a, b)| (*a - b).abs() < 1e-12));
                }
            }
        }
        assert_eq!(grid.covered_sites(), covered);
        assert_eq!(grid.coordinate(16, 0), None);
        assert_eq!(budget.used(), 4096);
        let clone = grid.clone();
        drop(grid);
        assert_eq!(budget.used(), 4096);
        drop(clone);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn every_cancel_and_one_short_memory_work_refuse_without_partial_grid() {
    let mesh = mesh();
    let budget = MemoryBudget::new(4096);
    let calls = Cell::new(0);
    let grid = build_mesh_grid(
        &mesh,
        [0, 0, 16, 16],
        MeshDirection::TargetToSource,
        policy(),
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    drop(grid);
    let total = calls.get();
    for checkpoint in 1..=total {
        let c = Cell::new(0);
        assert!(matches!(
            build_mesh_grid(
                &mesh,
                [0, 0, 16, 16],
                MeshDirection::TargetToSource,
                policy(),
                &budget,
                || {
                    c.set(c.get() + 1);
                    c.get() == checkpoint
                }
            ),
            Err(PiecewiseError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let mut short = policy();
    short.maximum_face_sites = 241;
    assert!(matches!(
        build_mesh_grid(
            &mesh,
            [0, 0, 16, 16],
            MeshDirection::TargetToSource,
            short,
            &budget,
            || false
        ),
        Err(PiecewiseError::Budget)
    ));
    assert!(matches!(
        build_mesh_grid(
            &mesh,
            [0, 0, 16, 16],
            MeshDirection::TargetToSource,
            policy(),
            &MemoryBudget::new(4095),
            || false
        ),
        Err(PiecewiseError::Budget)
    ));
    assert!(matches!(
        build_mesh_grid(
            &mesh,
            [u32::MAX, 0, 2, 1],
            MeshDirection::TargetToSource,
            policy(),
            &budget,
            || false
        ),
        Err(PiecewiseError::Invalid)
    ));
    assert_eq!(budget.used(), 0);
}
