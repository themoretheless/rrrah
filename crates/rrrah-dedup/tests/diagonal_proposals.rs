#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::Correspondence,
    piecewise_warp::{PiecewiseError, PiecewisePolicy, PiecewiseWarp},
    triangulation::{DiagonalPolicy, propose_source_diagonals},
};
use std::cell::Cell;
fn points() -> Vec<Correspondence> {
    vec![
        Correspondence {
            source: [1389.5, 1305.5],
            target: [504.5, 546.5],
        },
        Correspondence {
            source: [1365.5, 1301.5],
            target: [492.5, 546.5],
        },
        Correspondence {
            source: [1392.5003589374987, 1307.647545195113],
            target: [506.49556211075463, 546.0935418572013],
        },
        Correspondence {
            source: [1322.5, 1120.5],
            target: [476.7970773009196, 475.3828637385465],
        },
    ]
}
fn policy() -> DiagonalPolicy {
    DiagonalPolicy {
        maximum_points: 4,
        maximum_triangles: 2,
        maximum_flips: 1,
        maximum_tests: 1000,
        minimum_twice_area: 1e-6,
    }
}
#[test]
fn diagonal_proposal_preserves_landmarks_and_passes_complete_mesh_validation() {
    let p = points();
    let faces = [[0, 1, 2], [1, 3, 2]];
    let budget = MemoryBudget::new(10000);
    let changed = propose_source_diagonals(&p, &faces, policy(), &budget, || false).unwrap();
    assert_eq!(changed.len(), 2);
    assert_ne!(&*changed, &faces);
    let mesh = PiecewiseWarp::from_triangles(
        &p,
        &changed,
        (2048, 1536),
        (800, 600),
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
    for point in &p {
        let value = mesh.map_target(point.target, 2, || false).unwrap().unwrap();
        assert!(value.iter().zip(point.source).all(|(a, b)| (*a - b).abs() < 1e-7));
    }
    drop(mesh);
    drop(changed);
    assert_eq!(budget.used(), 0);
}
#[test]
fn cancellation_budgets_and_invalid_indices_have_no_partial_output() {
    let p = points();
    let faces = [[0, 1, 2], [1, 3, 2]];
    let budget = MemoryBudget::new(10000);
    let calls = Cell::new(0);
    let r = propose_source_diagonals(&p, &faces, policy(), &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    drop(r);
    let total = calls.get();
    for checkpoint in 1..=total {
        let count = Cell::new(0);
        assert!(matches!(
            propose_source_diagonals(&p, &faces, policy(), &budget, || {
                count.set(count.get() + 1);
                count.get() == checkpoint
            }),
            Err(PiecewiseError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let mut zero = policy();
    zero.maximum_flips = 0;
    assert!(matches!(
        propose_source_diagonals(&p, &faces, zero, &budget, || false),
        Err(PiecewiseError::Budget)
    ));
    zero = policy();
    zero.maximum_tests = 0;
    assert!(matches!(
        propose_source_diagonals(&p, &faces, zero, &budget, || false),
        Err(PiecewiseError::Budget)
    ));
    assert!(matches!(
        propose_source_diagonals(&p, &[[0, 1, 4]], policy(), &budget, || false),
        Err(PiecewiseError::Invalid)
    ));
    assert!(matches!(
        propose_source_diagonals(&p, &faces, policy(), &MemoryBudget::new(1), || false),
        Err(PiecewiseError::Budget)
    ));
    assert_eq!(budget.used(), 0);
}
