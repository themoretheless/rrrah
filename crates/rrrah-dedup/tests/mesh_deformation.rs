#![cfg(feature = "raster")]
use rrrah_core::MemoryBudget;
use rrrah_dedup::{
    geometry::{Correspondence, ProjectiveTransform},
    mesh_deformation::{MeshDeformationPolicy, validate_mesh_deformation},
    piecewise_warp::{PiecewiseError, PiecewisePolicy, PiecewiseWarp},
};
use std::cell::Cell;
fn identity() -> ProjectiveTransform {
    ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    }
}
fn policy() -> MeshDeformationPolicy {
    MeshDeformationPolicy {
        maximum_displacement: 1.,
        maximum_edge_slope: 1.,
        maximum_checks: 6,
    }
}
fn mesh(points: [[f64; 2]; 3], target: [[f64; 2]; 3], dimensions: ((u32, u32), (u32, u32))) -> PiecewiseWarp {
    let ps: Vec<_> = points
        .into_iter()
        .zip(target)
        .map(|(source, target)| Correspondence { source, target })
        .collect();
    PiecewiseWarp::from_triangles(
        &ps,
        &[[0, 1, 2]],
        dimensions.0,
        dimensions.1,
        PiecewisePolicy {
            maximum_points: 3,
            maximum_triangles: 1,
            maximum_pair_tests: 3,
            minimum_twice_area: 0.01,
        },
        &MemoryBudget::new(10000),
        || false,
    )
    .unwrap()
}
#[test]
fn analytic_translation_and_shear_have_distinct_displacement_and_slope_bounds() {
    let t = [[10., 10.], [30., 10.], [10., 30.]];
    let m = mesh([[15., 10.], [35., 10.], [15., 30.]], t, ((100, 100), (100, 100)));
    let e = validate_mesh_deformation(&m, identity(), policy(), || false).unwrap();
    assert!((e.maximum_displacement - 0.05).abs() < 1e-14);
    assert!(e.maximum_edge_slope < 1e-14);
    assert!(e.conforms);
    let e = validate_mesh_deformation(
        &m,
        identity(),
        MeshDeformationPolicy {
            maximum_displacement: 0.049,
            ..policy()
        },
        || false,
    )
    .unwrap();
    assert!(!e.conforms);
    assert_eq!(e.checks, 6);
    let m = mesh([[10., 10.], [30., 10.], [20., 30.]], t, ((100, 100), (100, 100)));
    let e = validate_mesh_deformation(&m, identity(), policy(), || false).unwrap();
    assert!((e.maximum_displacement - 0.1).abs() < 1e-14);
    assert!((e.maximum_edge_slope - 0.5).abs() < 1e-14);
    assert!(
        !validate_mesh_deformation(
            &m,
            identity(),
            MeshDeformationPolicy {
                maximum_edge_slope: 0.49,
                ..policy()
            },
            || false
        )
        .unwrap()
        .conforms
    );
}
#[test]
fn exact_perspective_and_asymmetric_axis_normalization() {
    let baseline = ProjectiveTransform {
        matrix: [[1.1, 0.1, 4.], [0.03, 0.9, 2.], [0.001, 0.002, 1.]],
    };
    let source = [[10., 10.], [40., 10.], [10., 40.]];
    let target = source.map(|p| baseline.apply(p).unwrap());
    let m = mesh(source, target, ((100, 80), (140, 90)));
    let e = validate_mesh_deformation(&m, baseline, policy(), || false).unwrap();
    assert!(e.maximum_displacement < 1e-14 && e.maximum_edge_slope < 1e-14);
    let m = mesh(
        [[15., 12.], [35., 12.], [15., 32.]],
        [[10., 10.], [30., 10.], [10., 30.]],
        ((100, 200), (100, 200)),
    );
    let e = validate_mesh_deformation(&m, identity(), policy(), || false).unwrap();
    assert!((e.maximum_displacement - 0.05_f64.hypot(0.01)).abs() < 1e-14);
}
#[test]
fn preflight_refusal_and_every_cancellation_checkpoint_are_atomic() {
    let m = mesh(
        [[10., 10.], [30., 10.], [10., 30.]],
        [[10., 10.], [30., 10.], [10., 30.]],
        ((100, 100), (100, 100)),
    );
    assert_eq!(
        validate_mesh_deformation(
            &m,
            identity(),
            MeshDeformationPolicy {
                maximum_checks: 5,
                ..policy()
            },
            || false
        ),
        Err(PiecewiseError::Budget)
    );
    for x in [f64::NAN, f64::INFINITY, -1.] {
        assert_eq!(
            validate_mesh_deformation(
                &m,
                identity(),
                MeshDeformationPolicy {
                    maximum_displacement: x,
                    ..policy()
                },
                || false
            ),
            Err(PiecewiseError::Invalid)
        );
        assert_eq!(
            validate_mesh_deformation(
                &m,
                identity(),
                MeshDeformationPolicy {
                    maximum_edge_slope: x,
                    ..policy()
                },
                || false
            ),
            Err(PiecewiseError::Invalid)
        );
    }
    let calls = Cell::new(0);
    let expected = validate_mesh_deformation(&m, identity(), policy(), || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    for at in 1..=calls.get() {
        let n = Cell::new(0);
        assert_eq!(
            validate_mesh_deformation(&m, identity(), policy(), || {
                n.set(n.get() + 1);
                n.get() == at
            }),
            Err(PiecewiseError::Cancelled)
        );
        assert_eq!(
            validate_mesh_deformation(&m, identity(), policy(), || false).unwrap(),
            expected
        );
    }
}
