use rrrah_dedup::{
    geometry::{GeometryPolicy, verify_similarity},
    linear::LinearRgbaView,
    local::{LocalError, LocalPolicy, MatchPolicy, extract, extract_oriented, match_features},
    warp::{WarpPolicy, verify_bidirectional},
};
fn image(name: &str) -> Vec<f32> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    std::fs::read(root.join(format!("{name}.rgba")))
        .unwrap()
        .into_iter()
        .map(|v| f32::from(v) / 255.)
        .collect()
}
const POLICY: LocalPolicy = LocalPolicy {
    max_pixels: 25600,
    max_candidates: 25600,
    max_features: 500,
    minimum_corner_score: 0.0001,
};
const MATCH: MatchPolicy = MatchPolicy {
    max_comparisons: 250_000,
    max_distance: 64,
};
const GEOMETRY: GeometryPolicy = GeometryPolicy {
    tolerance: 2.,
    min_inliers: 10,
    max_points: 500,
    max_hypotheses: 125_000,
};
#[test]
fn independent_arbitrary_rotations_recover_geometry_and_reject_unrelated_texture() {
    let pixels = image("base");
    let base = LinearRgbaView::new(160, 160, &pixels, 25600, || false).unwrap();
    let left = extract_oriented(&base, POLICY, || false).unwrap();
    for angle in [17_i32, -37, 63] {
        let pixels = image(&format!("angle-{angle}"));
        let target = LinearRgbaView::new(160, 160, &pixels, 25600, || false).unwrap();
        let right = extract_oriented(&target, POLICY, || false).unwrap();
        let matches = match_features(&left, &right, MATCH, || false).unwrap();
        println!("{angle}: {} features, {} matches", right.len(), matches.len());
        let geometry = verify_similarity(&matches, GEOMETRY, || false).unwrap().unwrap();
        println!(
            "{angle}: inliers={}, transform={:?}",
            geometry.inliers.len(),
            geometry.transform
        );
        let radians = f64::from(angle).to_radians();
        assert!((geometry.transform.a - radians.cos()).abs() < 0.03);
        assert!((geometry.transform.b + radians.sin()).abs() < 0.03);
        let expected = [
            79.5 * (1. - radians.cos() - radians.sin()),
            79.5 * (1. - radians.cos() + radians.sin()),
        ];
        for (a, b) in geometry.transform.translation.into_iter().zip(expected) {
            assert!((a - b).abs() < 3.);
        }
        let residual = verify_bidirectional(
            &base,
            &target,
            geometry.transform,
            WarpPolicy {
                tolerance: 0.03,
                max_source_pixels: 25600,
            },
            || false,
        )
        .unwrap();
        println!("{angle}: residual {residual:?}");
        assert!(residual.forward.compared_pixels > 18000);
        assert!(residual.forward.matched_pixels * 100 > residual.forward.compared_pixels * 90);
        assert!(residual.reverse.compared_pixels > 18000);
        assert!(residual.reverse.matched_pixels * 100 > residual.reverse.compared_pixels * 90);
    }
    let pixels = image("unrelated");
    let negative = LinearRgbaView::new(160, 160, &pixels, 25600, || false).unwrap();
    let right = extract_oriented(&negative, POLICY, || false).unwrap();
    let matches = match_features(&left, &right, MATCH, || false).unwrap();
    assert!(verify_similarity(&matches, GEOMETRY, || false).unwrap().is_none());
    let pixels = verify_bidirectional(
        &base,
        &negative,
        rrrah_dedup::geometry::Transform {
            a: 1.,
            b: 0.,
            translation: [0.; 2],
        },
        WarpPolicy {
            tolerance: 0.03,
            max_source_pixels: 25600,
        },
        || false,
    )
    .unwrap();
    assert!(pixels.forward.matched_pixels * 100 < pixels.forward.compared_pixels * 90);
    let legacy = extract(&base, POLICY, || false).unwrap();
    assert_eq!(
        match_features(&left, &legacy, MATCH, || false).unwrap_err(),
        LocalError::Invalid
    );
    assert_eq!(
        extract_oriented(&base, POLICY, || true).unwrap_err(),
        LocalError::Cancelled
    );
}

#[test]
fn oriented_extraction_cancels_inside_descriptor_work_and_enforces_budgets() {
    let pixels = image("base");
    let view = LinearRgbaView::new(160, 160, &pixels, 25600, || false).unwrap();
    let calls = std::cell::Cell::new(0_usize);
    let features = extract_oriented(&view, POLICY, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert!(features.len() > 100);
    assert!(
        features
            .iter()
            .all(|v| v.recipe == rrrah_dedup::local::FeatureRecipe::OrientedBriefV1)
    );
    let stop = calls.get() - 128;
    calls.set(0);
    assert_eq!(
        extract_oriented(&view, POLICY, || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        })
        .unwrap_err(),
        LocalError::Cancelled
    );
    for policy in [
        LocalPolicy {
            max_pixels: 1,
            ..POLICY
        },
        LocalPolicy {
            max_candidates: 1,
            ..POLICY
        },
    ] {
        assert_eq!(
            extract_oriented(&view, policy, || false).unwrap_err(),
            LocalError::Budget
        );
    }
    assert!(
        extract_oriented(
            &view,
            LocalPolicy {
                max_features: 0,
                ..POLICY
            },
            || false
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn independently_resized_images_estimate_non_dyadic_scale_and_rotation() {
    let pixels = image("base");
    let source = LinearRgbaView::new(160, 160, &pixels, 25600, || false).unwrap();
    let policy = LocalPolicy {
        max_pixels: 50000,
        max_candidates: 50000,
        ..POLICY
    };
    let left = rrrah_dedup::local::extract_multiscale_oriented(&source, policy, || false).unwrap();
    for (size, angle) in [(216_u32, 0_i32), (117, 0), (216, 17)] {
        let pixels = image(&format!("scale-{size}-angle-{angle}"));
        let target = LinearRgbaView::new(size, size, &pixels, 50000, || false).unwrap();
        let right = rrrah_dedup::local::extract_multiscale_oriented(&target, policy, || false).unwrap();
        let matches = match_features(&left, &right, MATCH, || false).unwrap();
        println!("size={size}, angle={angle}, matches={}", matches.len());
        let evidence = verify_similarity(&matches, GEOMETRY, || false).unwrap().unwrap();
        println!(
            "inliers={}, transform={:?}",
            evidence.inliers.len(),
            evidence.transform
        );
        let scale = f64::from(size) / 160.;
        let radians = f64::from(angle).to_radians();
        assert!((evidence.transform.a - scale * radians.cos()).abs() < 0.03);
        assert!((evidence.transform.b + scale * radians.sin()).abs() < 0.03);
        let center = (f64::from(size) - 1.) / 2.;
        let expected = [
            center - scale * radians.cos() * 79.5 - scale * radians.sin() * 79.5,
            center + scale * radians.sin() * 79.5 - scale * radians.cos() * 79.5,
        ];
        for (actual, expected) in evidence.transform.translation.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 3.);
        }
        let residual = verify_bidirectional(
            &source,
            &target,
            evidence.transform,
            WarpPolicy {
                tolerance: 0.03,
                max_source_pixels: 50000,
            },
            || false,
        )
        .unwrap();
        println!("residual={residual:?}");
        for direction in [residual.forward, residual.reverse] {
            assert!(direction.compared_pixels > 10000);
            assert!(direction.matched_pixels * 100 > direction.compared_pixels * 90);
        }
    }
    let pixels = image("unrelated");
    let unrelated = LinearRgbaView::new(160, 160, &pixels, 25600, || false).unwrap();
    let right = rrrah_dedup::local::extract_multiscale_oriented(&unrelated, policy, || false).unwrap();
    let matches = match_features(&left, &right, MATCH, || false).unwrap();
    assert!(verify_similarity(&matches, GEOMETRY, || false).unwrap().is_none());
    let oriented_only = extract_oriented(&source, policy, || false).unwrap();
    assert_eq!(
        match_features(&left, &oriented_only, MATCH, || false).unwrap_err(),
        LocalError::Invalid
    );
    let count = std::cell::Cell::new(0_usize);
    rrrah_dedup::local::extract_multiscale_oriented(&source, policy, || {
        count.set(count.get() + 1);
        false
    })
    .unwrap();
    let stop = count.get() - 128;
    count.set(0);
    assert_eq!(
        rrrah_dedup::local::extract_multiscale_oriented(&source, policy, || {
            count.set(count.get() + 1);
            count.get() == stop
        })
        .unwrap_err(),
        LocalError::Cancelled
    );
    assert_eq!(
        rrrah_dedup::local::extract_multiscale_oriented(
            &source,
            LocalPolicy {
                max_pixels: 1,
                ..policy
            },
            || false
        )
        .unwrap_err(),
        LocalError::Budget
    );
}

#[test]
fn reflected_descriptors_reach_original_geometry_and_pixel_proof() {
    use rrrah_dedup::{
        geometry::verify_reflected_similarity, local::extract_reflected_oriented,
        warp::verify_reflected_bidirectional,
    };
    let pixels = image("base");
    let mirrored = (0..160)
        .flat_map(|y| {
            (0..160)
                .rev()
                .flat_map(move |x| (y * 160 + x) * 4..(y * 160 + x) * 4 + 4)
        })
        .map(|i| pixels[i])
        .collect::<Vec<_>>();
    let source = LinearRgbaView::new(160, 160, &pixels, 25600, || false).unwrap();
    let target = LinearRgbaView::new(160, 160, &mirrored, 25600, || false).unwrap();
    let left = extract_oriented(&source, POLICY, || false).unwrap();
    let calls = std::cell::Cell::new(0);
    let right = extract_reflected_oriented(&target, POLICY, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(left.len(), right.len());
    for (a, b) in left.iter().zip(&right) {
        assert_eq!(a.descriptor, b.descriptor);
        assert_eq!(a.position[0], 159. - b.position[0]);
        assert_eq!(a.position[1], b.position[1]);
    }
    let matches = match_features(&left, &right, MATCH, || false).unwrap();
    let geometry = verify_reflected_similarity(&matches, GEOMETRY, || false)
        .unwrap()
        .unwrap();
    let proof = verify_reflected_bidirectional(
        &source,
        &target,
        geometry.similarity.transform,
        WarpPolicy {
            tolerance: 1e-6,
            max_source_pixels: 25600,
        },
        || false,
    )
    .unwrap();
    assert_eq!(proof.forward.matched_pixels, 25600);
    assert_eq!(proof.reverse.matched_pixels, 25600);
    for checkpoint in [1, 100, 1000, calls.get()] {
        let current = std::cell::Cell::new(0);
        assert_eq!(
            extract_reflected_oriented(&target, POLICY, || {
                current.set(current.get() + 1);
                current.get() == checkpoint
            })
            .unwrap_err(),
            LocalError::Cancelled
        );
    }
    assert_eq!(
        extract_reflected_oriented(&target, POLICY, || false)
            .unwrap()
            .len(),
        right.len()
    );
}
