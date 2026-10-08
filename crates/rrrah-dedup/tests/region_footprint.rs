use rrrah_dedup::{
    geometry::ProjectiveTransform,
    region_footprint::{FootprintError, select_filter_footprint},
};
#[test]
fn scale_rotation_and_homography_magnitude_preserve_footprint() {
    for scalar in [1e-100, 1.0, -1e100] {
        let model = ProjectiveTransform {
            matrix: [[0., -0.5, 20.], [0.5, 0., 30.], [0., 0., 1.]].map(|r| r.map(|v| v * scalar)),
        };
        let result = select_filter_footprint(model, [10., 20.], 8, 16, || false).unwrap();
        assert_eq!(result.source_radius, 16);
        assert!((result.area_scale - 0.5).abs() < 1e-14);
        assert_eq!(
            select_filter_footprint(model, [10., 20.], 8, 15, || false),
            Err(FootprintError::Budget)
        );
    }
}
#[test]
fn invalid_horizon_singular_and_cancelled_refuse() {
    let singular = ProjectiveTransform { matrix: [[0.; 3]; 3] };
    assert_eq!(
        select_filter_footprint(singular, [0., 0.], 8, 16, || false),
        Err(FootprintError::Invalid)
    );
    assert_eq!(
        select_filter_footprint(singular, [0., 0.], 8, 16, || true),
        Err(FootprintError::Cancelled)
    );
    let horizon = ProjectiveTransform {
        matrix: [[1., 0., 0.], [0., 1., 0.], [1., 0., -2.]],
    };
    assert_eq!(
        select_filter_footprint(horizon, [2., 0.], 8, 16, || false),
        Err(FootprintError::Invalid)
    );
}

#[test]
fn inlier_domain_rounds_neighbors_and_stays_inside_fitting_region() {
    use rrrah_dedup::{geometry::Correspondence, region_footprint::bound_target_inliers};
    let points = [
        Correspondence {
            source: [1., 2.],
            target: [205.25, 162.5],
        },
        Correspondence {
            source: [3., 4.],
            target: [297.75, 224.9],
        },
    ];
    assert_eq!(
        bound_target_inliers(&points, &[0, 1], [200, 150, 100, 75], 2, || false).unwrap(),
        [205, 162, 94, 63]
    );
}
#[test]
fn inlier_domain_rejects_unbounded_indices_coordinates_and_work() {
    use rrrah_dedup::{
        geometry::Correspondence,
        region_footprint::{FootprintError, bound_target_inliers},
    };
    let points = [
        Correspondence {
            source: [1., 2.],
            target: [205.25, 162.5],
        },
        Correspondence {
            source: [3., 4.],
            target: [297.75, 224.9],
        },
    ];
    for ids in [&[0, 0][..], &[1, 0], &[2], &[]] {
        assert_eq!(
            bound_target_inliers(&points, ids, [200, 150, 100, 75], 2, || false),
            Err(FootprintError::Invalid)
        );
    }
    assert_eq!(
        bound_target_inliers(&points, &[0], [200, 150, 100, 75], 1, || false),
        Err(FootprintError::Budget)
    );
    assert_eq!(
        bound_target_inliers(&points, &[0], [u32::MAX, 150, 100, 75], 2, || false),
        Err(FootprintError::Invalid)
    );
    let mut outside = points;
    outside[0].target[0] = 300.;
    assert_eq!(
        bound_target_inliers(&outside, &[0], [200, 150, 100, 75], 2, || false),
        Err(FootprintError::Invalid)
    );
    outside[0].target[0] = f64::NAN;
    assert_eq!(
        bound_target_inliers(&outside, &[0], [200, 150, 100, 75], 2, || false),
        Err(FootprintError::Invalid)
    );
}
#[test]
fn inlier_domain_observes_mid_and_final_cancel_without_partial_result() {
    use rrrah_dedup::{
        geometry::Correspondence,
        region_footprint::{FootprintError, bound_target_inliers},
    };
    let points = [
        Correspondence {
            source: [1., 2.],
            target: [205.25, 162.5],
        },
        Correspondence {
            source: [3., 4.],
            target: [297.75, 224.9],
        },
    ];
    for stop in 0..4 {
        let calls = std::cell::Cell::new(0);
        assert_eq!(
            bound_target_inliers(&points, &[0, 1], [200, 150, 100, 75], 2, || {
                let n = calls.get();
                calls.set(n + 1);
                n == stop
            }),
            Err(FootprintError::Cancelled)
        );
    }
}
