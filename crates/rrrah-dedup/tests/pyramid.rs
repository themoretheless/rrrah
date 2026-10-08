use rrrah_dedup::{
    geometry::{GeometryPolicy, verify_similarity},
    linear::LinearRgbaView,
    local::{LocalPolicy, MatchPolicy, match_features},
    pyramid::{PyramidPolicy, extract_pyramid},
};

#[cfg(feature = "raster")]
#[test]
fn managed_oriented_pyramid_admission_cancel_retry_and_last_owner_release() {
    use rrrah_core::MemoryBudget;
    use rrrah_dedup::{
        local::LocalError,
        pyramid::{extract_oriented_pyramid, extract_oriented_pyramid_managed},
    };
    use std::cell::Cell;
    let mut seed = 1234567_u64;
    let pixels: Vec<f32> = (0..64 * 64)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let v = (seed & 255) as f32 / 255.;
            [v, v, v, 1.]
        })
        .collect();
    let view = LinearRgbaView::new(64, 64, &pixels, 4096, || false).unwrap();
    let policy = PyramidPolicy {
        local: LocalPolicy {
            max_pixels: 4096,
            max_candidates: 4096,
            max_features: 100,
            minimum_corner_score: 0.0001,
        },
        max_levels: 3,
        max_total_pixels: 6000,
        max_total_features: 300,
    };
    for spatial in [false, true] {
        let expected = if spatial {
            rrrah_dedup::pyramid::extract_spatial_oriented_pyramid(&view, policy, 4, 4, 6, || false).unwrap()
        } else {
            extract_oriented_pyramid(&view, policy, || false).unwrap()
        };
        let run = |budget: &MemoryBudget, cancel: &dyn Fn() -> bool| {
            if spatial {
                rrrah_dedup::pyramid::extract_spatial_oriented_pyramid_managed(
                    &view, policy, 4, 4, 6, budget, cancel,
                )
            } else {
                extract_oriented_pyramid_managed(&view, policy, budget, cancel)
            }
        };
        assert!(!expected.is_empty());
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let calls = Cell::new(0);
        let features = run(&budget, &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        let checkpoints = calls.get();
        assert_eq!(features.len(), expected.len());
        for (a, b) in features.iter().zip(&expected) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.recipe, b.recipe);
            assert_eq!(a.descriptor, b.descriptor);
            assert_eq!(a.quarter_turns, b.quarter_turns);
        }
        let retained = budget.used();
        assert!(retained > 0);
        let owner = features.clone();
        drop(features);
        assert_eq!(budget.used(), retained);
        drop(owner);
        assert_eq!(budget.used(), 0);
        let peak = budget.peak();
        for limit in [0, peak - 1] {
            let small = MemoryBudget::new(limit);
            assert!(matches!(run(&small, &|| false), Err(LocalError::Budget)));
            assert_eq!(small.used(), 0);
        }
        for stop in [1, checkpoints / 2, checkpoints] {
            calls.set(0);
            assert!(
                matches!(
                    run(&budget, &|| {
                        calls.set(calls.get() + 1);
                        calls.get() >= stop
                    }),
                    Err(LocalError::Cancelled)
                ),
                "stop={stop}"
            );
            assert_eq!(budget.used(), 0);
        }
        let exact = MemoryBudget::new(peak);
        let retry = run(&exact, &|| false).unwrap();
        assert_eq!(retry.len(), expected.len());
        drop(retry);
        assert_eq!(exact.used(), 0);
    }
}
#[test]
fn scale_is_estimated_from_extracted_multiresolution_features() {
    let mut seed = 1_234_567_u64;
    let source: Vec<f32> = (0..64 * 64)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let v = f32::from(u8::try_from(seed & 255).unwrap()) / 255.0;
            [v, v, v, 1.0]
        })
        .collect();
    let mut enlarged = Vec::new();
    for y in 0..128 {
        for x in 0..128 {
            let offset = ((y / 2) * 64 + x / 2) * 4;
            enlarged.extend_from_slice(&source[offset..offset + 4]);
        }
    }
    let a = LinearRgbaView::new(64, 64, &source, 4096, || false).unwrap();
    let b = LinearRgbaView::new(128, 128, &enlarged, 16384, || false).unwrap();
    let policy = PyramidPolicy {
        local: LocalPolicy {
            max_pixels: 20000,
            max_candidates: 20000,
            max_features: 200,
            minimum_corner_score: 0.01,
        },
        max_levels: 2,
        max_total_pixels: 25000,
        max_total_features: 400,
    };
    let left = extract_pyramid(&a, policy, || false).unwrap();
    let right = extract_pyramid(&b, policy, || false).unwrap();
    let matches = match_features(
        &left,
        &right,
        MatchPolicy {
            max_comparisons: 200_000,
            max_distance: 20,
        },
        || false,
    )
    .unwrap();
    assert!(matches.len() >= 4);
    let geometry = verify_similarity(
        &matches,
        GeometryPolicy {
            tolerance: 0.01,
            min_inliers: 4,
            max_points: 400,
            max_hypotheses: 80000,
        },
        || false,
    )
    .unwrap()
    .unwrap();
    assert!((geometry.transform.a - 2.0).abs() < 1e-6);
    assert!(geometry.transform.b.abs() < 1e-6);
    assert!((geometry.transform.translation[0] - 0.5).abs() < 1e-6);
    assert!((geometry.transform.translation[1] - 0.5).abs() < 1e-6);
    assert!(
        extract_pyramid(
            &a,
            PyramidPolicy {
                max_total_pixels: 1,
                ..policy
            },
            || false
        )
        .is_err()
    );
    assert!(extract_pyramid(&a, policy, || true).is_err());
}

#[test]
fn enlarged_quarter_turn_recovers_scale_and_rotation_together() {
    let mut seed = 1_234_567_u64;
    let pixels: Vec<f32> = (0..64 * 64)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let v = f32::from(u8::try_from(seed & 255).unwrap()) / 255.0;
            [v, v, v, 1.0]
        })
        .collect();
    let mut rotated = vec![0.0; 128 * 128 * 4];
    for y in 0..128 {
        for x in 0..128 {
            let source = ((y / 2) * 64 + x / 2) * 4;
            let target = (x * 128 + 127 - y) * 4;
            rotated[target..target + 4].copy_from_slice(&pixels[source..source + 4]);
        }
    }
    let source = LinearRgbaView::new(64, 64, &pixels, 4096, || false).unwrap();
    let target = LinearRgbaView::new(128, 128, &rotated, 16384, || false).unwrap();
    let policy = PyramidPolicy {
        local: LocalPolicy {
            max_pixels: 20000,
            max_candidates: 20000,
            max_features: 200,
            minimum_corner_score: 0.01,
        },
        max_levels: 2,
        max_total_pixels: 25000,
        max_total_features: 400,
    };
    let left = extract_pyramid(&source, policy, || false).unwrap();
    let right = extract_pyramid(&target, policy, || false).unwrap();
    let matches = match_features(
        &left,
        &right,
        MatchPolicy {
            max_comparisons: 200_000,
            max_distance: 20,
        },
        || false,
    )
    .unwrap();
    let evidence = verify_similarity(
        &matches,
        GeometryPolicy {
            tolerance: 0.01,
            min_inliers: 4,
            max_points: 400,
            max_hypotheses: 80000,
        },
        || false,
    )
    .unwrap()
    .unwrap();
    assert!(evidence.transform.a.abs() < 1e-6);
    assert!((evidence.transform.b - 2.0).abs() < 1e-6);
    assert!((evidence.transform.translation[0] - 126.5).abs() < 1e-6);
    assert!((evidence.transform.translation[1] - 0.5).abs() < 1e-6);
}

#[test]
fn oriented_pyramid_keeps_recipe_source_coordinates_and_admission() {
    use rrrah_dedup::{
        local::{FeatureRecipe, LocalError},
        pyramid::extract_oriented_pyramid,
    };
    let mut seed = 1_234_567_u64;
    let pixels = (0..96 * 96)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let value = f32::from(u8::try_from(seed & 255).unwrap()) / 255.0;
            [value, value, value, 1.0]
        })
        .collect::<Vec<_>>();
    let image = LinearRgbaView::new(96, 96, &pixels, 9216, || false).unwrap();
    let policy = PyramidPolicy {
        local: LocalPolicy {
            max_pixels: 10000,
            max_candidates: 10000,
            max_features: 100,
            minimum_corner_score: 0.0001,
        },
        max_levels: 2,
        max_total_pixels: 12000,
        max_total_features: 200,
    };
    let features = extract_oriented_pyramid(&image, policy, || false).unwrap();
    assert!(features.len() > 10);
    let first_level = extract_oriented_pyramid(
        &image,
        PyramidPolicy {
            max_levels: 1,
            ..policy
        },
        || false,
    )
    .unwrap();
    assert!(features.len() > first_level.len());
    assert_eq!(
        extract_oriented_pyramid(
            &image,
            PyramidPolicy {
                max_total_features: first_level.len(),
                ..policy
            },
            || false,
        )
        .unwrap_err(),
        LocalError::Budget
    );
    let checks = std::cell::Cell::new(0);
    extract_oriented_pyramid(&image, policy, || {
        checks.set(checks.get() + 1);
        false
    })
    .unwrap();
    let final_check = checks.get();
    checks.set(0);
    assert_eq!(
        extract_oriented_pyramid(&image, policy, || {
            checks.set(checks.get() + 1);
            checks.get() == final_check
        })
        .unwrap_err(),
        LocalError::Cancelled
    );
    let retried = extract_oriented_pyramid(&image, policy, || false).unwrap();
    assert_eq!(retried.len(), features.len());
    for (actual, expected) in retried.iter().zip(&features) {
        assert_eq!(actual.position, expected.position);
        assert_eq!(actual.descriptor, expected.descriptor);
    }
    assert!(
        features
            .iter()
            .all(|f| f.recipe == FeatureRecipe::OrientedScaleBriefV1
                && f.position.iter().all(|v| (0.0..96.0).contains(v)))
    );
    let matched = match_features(
        &features,
        &features,
        MatchPolicy {
            max_comparisons: 40000,
            max_distance: 64,
        },
        || false,
    )
    .unwrap();
    let geometry = verify_similarity(
        &matched,
        GeometryPolicy {
            tolerance: 0.01,
            min_inliers: 10,
            max_points: 200,
            max_hypotheses: 20000,
        },
        || false,
    )
    .unwrap()
    .unwrap();
    assert!((geometry.transform.a - 1.0).abs() < 1e-9);
    assert!(geometry.transform.b.abs() < 1e-9);
    assert!(matches!(
        extract_oriented_pyramid(&image, policy, || true),
        Err(LocalError::Cancelled)
    ));
    assert!(matches!(
        extract_oriented_pyramid(
            &image,
            PyramidPolicy {
                max_total_pixels: 1,
                ..policy
            },
            || false
        ),
        Err(LocalError::Budget)
    ));
}

#[test]
fn spatial_pyramid_preserves_level_coordinates_admission_and_cancellation() {
    use rrrah_dedup::{
        local::{LocalError, extract_spatial_oriented},
        pyramid::extract_spatial_oriented_pyramid,
    };
    use std::cell::Cell;
    let mut seed = 918273_u64;
    let samples: Vec<f32> = (0..128 * 128)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let v = (seed & 255) as f32 / 255.;
            [v, v, v, 1.]
        })
        .collect();
    let view = LinearRgbaView::new(128, 128, &samples, 16384, || false).unwrap();
    let p = PyramidPolicy {
        local: LocalPolicy {
            max_pixels: 16384,
            max_candidates: 16384,
            max_features: 100,
            minimum_corner_score: 0.0001,
        },
        max_levels: 1,
        max_total_pixels: 22000,
        max_total_features: 300,
    };
    let expected = extract_spatial_oriented(&view, p.local, 4, 4, 6, || false).unwrap();
    let single = extract_spatial_oriented_pyramid(&view, p, 4, 4, 6, || false).unwrap();
    assert!(!single.is_empty());
    assert_eq!(single.len(), expected.len());
    for (a, b) in single.iter().zip(&expected) {
        assert_eq!(a.position, b.position);
        assert_eq!(a.recipe, b.recipe);
        assert_eq!(a.descriptor, b.descriptor);
        assert_eq!(a.quarter_turns, b.quarter_turns);
    }
    let p = PyramidPolicy { max_levels: 3, ..p };
    let calls = Cell::new(0);
    let full = extract_spatial_oriented_pyramid(&view, p, 4, 4, 6, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let checkpoints = calls.get();
    assert!(full.len() > single.len());
    assert!(
        full.iter()
            .all(|f| f.position.iter().all(|v| *v >= 0. && *v < 128.))
    );
    for stop in [1, checkpoints / 2, checkpoints] {
        calls.set(0);
        assert!(matches!(
            extract_spatial_oriented_pyramid(&view, p, 4, 4, 6, || {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            }),
            Err(LocalError::Cancelled)
        ));
    }
    assert_eq!(
        extract_spatial_oriented_pyramid(&view, p, 4, 4, 6, || false)
            .unwrap()
            .len(),
        full.len()
    );
    assert!(matches!(
        extract_spatial_oriented_pyramid(&view, p, 0, 4, 6, || false),
        Err(LocalError::Invalid)
    ));
    assert!(matches!(
        extract_spatial_oriented_pyramid(&view, p, usize::MAX, 2, 6, || false),
        Err(LocalError::Budget)
    ));
    assert!(matches!(
        extract_spatial_oriented_pyramid(
            &view,
            PyramidPolicy {
                max_total_features: single.len() - 1,
                ..p
            },
            4,
            4,
            6,
            || false
        ),
        Err(LocalError::Budget)
    ));
    assert!(matches!(
        extract_spatial_oriented_pyramid(
            &view,
            PyramidPolicy {
                max_total_pixels: 16384,
                ..p
            },
            4,
            4,
            6,
            || false
        ),
        Err(LocalError::Budget)
    ));
}
