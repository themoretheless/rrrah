use rrrah_dedup::{gradient::describe_gradient, linear::LinearRgbaView, local::LocalError};
#[test]
fn gradient_affine_light_flat_refusals_and_cancel() {
    let make = |gain: f32, offset: f32| -> Vec<f32> {
        (0..48)
            .flat_map(|y| {
                (0..48).flat_map(move |x| {
                    let value = ((x * x + 3 * y * y + 7 * x * y) % 137) as f32 / 137.;
                    let v = value * gain + offset;
                    [v, v, v, 1.]
                })
            })
            .collect()
    };
    let a = make(1., 0.);
    let b = make(0.6, 0.1);
    let av = LinearRgbaView::new(48, 48, &a, 2304, || false).unwrap();
    let bv = LinearRgbaView::new(48, 48, &b, 2304, || false).unwrap();
    let first = describe_gradient(&av, [24., 24.], 1., 0., 256, || false)
        .unwrap()
        .unwrap();
    let second = describe_gradient(&bv, [24., 24.], 1., 0., 256, || false)
        .unwrap()
        .unwrap();
    assert!(first.0.iter().zip(second.0).all(|(a, b)| (a - b).abs() < 1e-6));
    assert!((first.0.iter().map(|v| v * v).sum::<f64>() - 1.).abs() < 1e-12);
    let flat = vec![0.5, 0.5, 0.5, 1.].repeat(2304);
    let fv = LinearRgbaView::new(48, 48, &flat, 2304, || false).unwrap();
    assert!(
        describe_gradient(&fv, [24., 24.], 1., 0., 256, || false)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        describe_gradient(&av, [24., 24.], 1., 0., 255, || false),
        Err(LocalError::Budget)
    ));
    assert!(matches!(
        describe_gradient(&av, [0., 0.], 1., 0., 256, || false),
        Err(LocalError::Invalid)
    ));
    assert!(matches!(
        describe_gradient(&av, [24., 24.], 0., 0., 256, || false),
        Err(LocalError::Invalid)
    ));
    for stop in [1, 128, 257] {
        let calls = std::cell::Cell::new(0);
        assert!(matches!(
            describe_gradient(&av, [24., 24.], 1., 0., 256, || {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            }),
            Err(LocalError::Cancelled)
        ));
    }
    assert_eq!(
        describe_gradient(&av, [24., 24.], 1., 0., 256, || false).unwrap(),
        Some(first)
    );
}

#[test]
fn rotated_uniform_patches_never_create_descriptors() {
    for value in [0.5_f32, 1., 10000.] {
        let flat = [value, value, value, 1.].repeat(2304);
        let view = LinearRgbaView::new(48, 48, &flat, 2304, || false).unwrap();
        for angle in [0.3, 1., 2.5] {
            assert!(
                describe_gradient(&view, [24.25, 24.5], 1., angle, 256, || false)
                    .unwrap()
                    .is_none(),
                "value={value} angle={angle}"
            );
        }
    }
}

#[test]
fn independently_rotated_texture_and_alpha_refusal() {
    let mut a = Vec::new();
    for y in 0..48 {
        for x in 0..48 {
            let v = ((x * x + 5 * y * y + 13 * x * y) % 127) as f32 / 127.;
            a.extend([v, v, v, 1.]);
        }
    }
    let mut b = vec![0.; a.len()];
    for y in 0..48 {
        for x in 0..48 {
            let source = (y * 48 + x) * 4;
            let target = (x * 48 + (47 - y)) * 4;
            b[target..target + 4].copy_from_slice(&a[source..source + 4]);
        }
    }
    let av = LinearRgbaView::new(48, 48, &a, 2304, || false).unwrap();
    let bv = LinearRgbaView::new(48, 48, &b, 2304, || false).unwrap();
    let first = describe_gradient(&av, [24., 24.], 1., 0., 256, || false)
        .unwrap()
        .unwrap();
    let rotated = describe_gradient(&bv, [23., 24.], 1., std::f64::consts::FRAC_PI_2, 256, || false)
        .unwrap()
        .unwrap();
    assert!(first.0.iter().zip(rotated.0).all(|(a, b)| (a - b).abs() < 1e-12));
    a[(24 * 48 + 24) * 4 + 3] = 0.;
    let alpha = LinearRgbaView::new(48, 48, &a, 2304, || false).unwrap();
    assert!(matches!(
        describe_gradient(&alpha, [24., 24.], 1., 0., 256, || false),
        Err(LocalError::Invalid)
    ));
}

#[test]
fn gradient_matching_mutual_ties_admission_and_cancel() {
    use rrrah_dedup::gradient::{GradientDescriptor, GradientFeature, GradientMatchPolicy, match_gradients};
    let feature = |index: usize, position: [f64; 2]| {
        let mut d = [0.; 128];
        d[index] = 1.;
        GradientFeature {
            position,
            descriptor: GradientDescriptor(d),
        }
    };
    let left = [feature(0, [1., 2.]), feature(1, [3., 4.]), feature(2, [5., 6.])];
    let right = [
        feature(2, [15., 16.]),
        feature(0, [11., 12.]),
        feature(1, [13., 14.]),
    ];
    let p = GradientMatchPolicy {
        max_comparisons: 9,
        max_squared_distance: 0.1,
        squared_ratio: 0.64,
    };
    let calls = std::cell::Cell::new(0);
    let result = match_gradients(&left, &right, p, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let checkpoints = calls.get();
    assert_eq!(result.len(), 3);
    for (i, c) in result.iter().enumerate() {
        assert_eq!(c.source, left[i].position);
        assert_eq!(c.target, [left[i].position[0] + 10., left[i].position[1] + 10.]);
    }
    assert!(
        match_gradients(&[left[0]], &[right[1], right[1]], p, || false)
            .unwrap()
            .is_empty()
    );
    assert!(
        match_gradients(&[left[0], left[0]], &[right[1]], p, || false)
            .unwrap()
            .is_empty()
    );
    assert!(
        match_gradients(&left, &[feature(5, [0., 0.])], p, || false)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        match_gradients(
            &left,
            &right,
            GradientMatchPolicy {
                max_comparisons: 8,
                ..p
            },
            || false
        ),
        Err(LocalError::Budget)
    ));
    let mut invalid = left;
    invalid[0].descriptor.0[0] = f64::NAN;
    assert!(matches!(
        match_gradients(&invalid, &right, p, || false),
        Err(LocalError::Invalid)
    ));
    invalid = left;
    invalid[0].descriptor.0 = [0.; 128];
    assert!(matches!(
        match_gradients(&invalid, &right, p, || false),
        Err(LocalError::Invalid)
    ));
    for stop in [1, checkpoints / 2, checkpoints] {
        calls.set(0);
        assert!(matches!(
            match_gradients(&left, &right, p, || {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            }),
            Err(LocalError::Cancelled)
        ));
    }
    assert_eq!(match_gradients(&left, &right, p, || false).unwrap().len(), 3);
}

#[test]
fn dominant_gradient_orientation_analytic_angles_flat_and_limits() {
    use rrrah_dedup::gradient::gradient_orientation;
    for (dx, dy, expected) in [
        (1., 0., 0.),
        (0., 1., std::f64::consts::FRAC_PI_2),
        (1., 1., std::f64::consts::FRAC_PI_4),
    ] {
        let pixels: Vec<f32> = (0..48)
            .flat_map(|y| {
                (0..48).flat_map(move |x| {
                    let v = ((x as f64 * dx + y as f64 * dy) / 96.) as f32;
                    [v, v, v, 1.]
                })
            })
            .collect();
        let view = LinearRgbaView::new(48, 48, &pixels, 2304, || false).unwrap();
        let angle = gradient_orientation(&view, [24, 24], 256, || false)
            .unwrap()
            .unwrap();
        let error = (angle - expected + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        assert!(error.abs() < 1e-5, "angle={angle} expected={expected}");
        assert!(matches!(
            gradient_orientation(&view, [24, 24], 255, || false),
            Err(LocalError::Budget)
        ));
        assert!(matches!(
            gradient_orientation(&view, [u32::MAX, 24], 256, || false),
            Err(LocalError::Invalid)
        ));
        assert!(matches!(
            gradient_orientation(&view, [24, 24], 256, || true),
            Err(LocalError::Cancelled)
        ));
    }
    let flat = [0.5, 0.5, 0.5, 1.].repeat(2304);
    let view = LinearRgbaView::new(48, 48, &flat, 2304, || false).unwrap();
    assert_eq!(
        gradient_orientation(&view, [24, 24], 256, || false).unwrap(),
        None
    );
}

#[test]
fn extracted_gradients_identity_unrelated_work_and_cancel() {
    use rrrah_dedup::{
        gradient::{GradientMatchPolicy, extract_gradients, match_gradients},
        local::LocalPolicy,
    };
    let pixels = |mut seed: u64| -> Vec<f32> {
        (0..96 * 96)
            .flat_map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let v = (seed & 255) as f32 / 255.;
                [v, v, v, 1.]
            })
            .collect()
    };
    let a = pixels(1234567);
    let b = pixels(9876543);
    let av = LinearRgbaView::new(96, 96, &a, 9216, || false).unwrap();
    let bv = LinearRgbaView::new(96, 96, &b, 9216, || false).unwrap();
    let p = LocalPolicy {
        max_pixels: 9216,
        max_candidates: 9216,
        max_features: 100,
        minimum_corner_score: 0.0001,
    };
    let calls = std::cell::Cell::new(0);
    let first = extract_gradients(&av, p, 51200, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let checkpoints = calls.get();
    assert!(first.len() > 10);
    assert!(
        first
            .iter()
            .all(|f| f.position.iter().all(|v| *v >= 13. && *v < 83.))
    );
    let repeated = extract_gradients(&av, p, 51200, || false).unwrap();
    assert_eq!(first, repeated);
    let policy = GradientMatchPolicy {
        max_comparisons: 10000,
        max_squared_distance: 0.3,
        squared_ratio: 0.64,
    };
    let matched = match_gradients(&first, &repeated, policy, || false).unwrap();
    assert_eq!(matched.len(), first.len());
    assert!(matched.iter().all(|m| m.source == m.target));
    let other = extract_gradients(&bv, p, 51200, || false).unwrap();
    assert!(match_gradients(&first, &other, policy, || false).unwrap().len() < 10);
    assert!(matches!(
        extract_gradients(&av, p, 51199, || false),
        Err(LocalError::Budget)
    ));
    for stop in [1, checkpoints / 2, checkpoints] {
        calls.set(0);
        assert!(matches!(
            extract_gradients(&av, p, 51200, || {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            }),
            Err(LocalError::Cancelled)
        ));
    }
    assert_eq!(extract_gradients(&av, p, 51200, || false).unwrap(), first);
}

#[test]
fn gradient_pyramid_independent_double_size_coordinates_and_work() {
    use rrrah_dedup::{
        gradient::{GradientMatchPolicy, extract_gradient_pyramid, extract_gradients, match_gradients},
        local::LocalPolicy,
        pyramid::PyramidPolicy,
    };
    let mut seed = 471823_u64;
    let a: Vec<f32> = (0..96 * 96)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let v = (seed & 255) as f32 / 255.;
            [v, v, v, 1.]
        })
        .collect();
    let mut b = Vec::new();
    for y in 0..192 {
        for x in 0..192 {
            let index = ((y / 2) * 96 + x / 2) * 4;
            b.extend_from_slice(&a[index..index + 4]);
        }
    }
    let av = LinearRgbaView::new(96, 96, &a, 36864, || false).unwrap();
    let bv = LinearRgbaView::new(192, 192, &b, 36864, || false).unwrap();
    let p = PyramidPolicy {
        local: LocalPolicy {
            max_pixels: 36864,
            max_candidates: 36864,
            max_features: 100,
            minimum_corner_score: 0.0001,
        },
        max_levels: 1,
        max_total_pixels: 50000,
        max_total_features: 200,
    };
    let first = extract_gradient_pyramid(&av, p, 51200, || false).unwrap();
    assert_eq!(first, extract_gradients(&av, p.local, 51200, || false).unwrap());
    let p = PyramidPolicy { max_levels: 2, ..p };
    let doubled = extract_gradient_pyramid(&bv, p, 102400, || false).unwrap();
    #[cfg(feature = "raster")]
    {
        use rrrah_core::MemoryBudget;
        use rrrah_dedup::gradient::extract_gradient_pyramid_managed;
        let budget = MemoryBudget::new(4 * 1024 * 1024);
        let calls = std::cell::Cell::new(0);
        let features = extract_gradient_pyramid_managed(&bv, p, 102400, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        let checkpoints = calls.get();
        assert_eq!(&features[..], &doubled[..]);
        let used = budget.used();
        assert!(used > 0);
        let clone = features.clone();
        drop(features);
        assert_eq!(budget.used(), used);
        drop(clone);
        assert_eq!(budget.used(), 0);
        let peak = budget.peak();
        for limit in [0, peak - 1] {
            let small = MemoryBudget::new(limit);
            assert!(matches!(
                extract_gradient_pyramid_managed(&bv, p, 102400, &small, || false),
                Err(LocalError::Budget)
            ));
            assert_eq!(small.used(), 0);
        }
        for stop in [1, checkpoints / 2, checkpoints] {
            calls.set(0);
            assert!(matches!(
                extract_gradient_pyramid_managed(&bv, p, 102400, &budget, || {
                    calls.set(calls.get() + 1);
                    calls.get() >= stop
                }),
                Err(LocalError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
        let exact = MemoryBudget::new(peak);
        let retry = extract_gradient_pyramid_managed(&bv, p, 102400, &exact, || false).unwrap();
        assert_eq!(&retry[..], &doubled[..]);
        drop(retry);
        assert_eq!(exact.used(), 0);
        let before = MemoryBudget::new(4 * 1024 * 1024);
        assert!(matches!(
            extract_gradient_pyramid_managed(&bv, p, 102399, &before, || false),
            Err(LocalError::Budget)
        ));
        assert_eq!(before.peak(), 0);
    }

    let matches = match_gradients(
        &first,
        &doubled,
        GradientMatchPolicy {
            max_comparisons: 20000,
            max_squared_distance: 0.3,
            squared_ratio: 0.64,
        },
        || false,
    )
    .unwrap();
    let correct = matches
        .iter()
        .filter(|m| {
            m.source
                .iter()
                .zip(m.target)
                .all(|(a, b)| ((*a + 0.5) * 2. - 0.5 - b).abs() < 1e-9)
        })
        .count();
    assert!(correct >= 10, "matches={} correct={correct}", matches.len());
    assert!(matches!(
        extract_gradient_pyramid(&bv, p, 102399, || false),
        Err(LocalError::Budget)
    ));
    assert!(matches!(
        extract_gradient_pyramid(&bv, p, 102400, || true),
        Err(LocalError::Cancelled)
    ));
}

#[test]
fn spatial_gradient_selection_preserves_recipe_and_cell_limits() {
    use rrrah_dedup::{
        gradient::{
            GradientCellRecipe, extract_gradients_with_recipe, extract_spatial_gradients_with_recipe,
        },
        local::LocalPolicy,
    };
    let pixels: Vec<f32> = (0..96)
        .flat_map(|y| {
            (0..96).flat_map(move |x| {
                let v = ((x * x + 3 * y * y + 7 * x * y + 11 * x + 23 * y) % 137) as f32 / 137.;
                [v, v, v, 1.]
            })
        })
        .collect();
    let view = LinearRgbaView::new(96, 96, &pixels, 9216, || false).unwrap();
    let policy = LocalPolicy {
        max_pixels: 9216,
        max_candidates: 9216,
        max_features: 16,
        minimum_corner_score: 0.0001,
    };
    for recipe in [GradientCellRecipe::Fixed, GradientCellRecipe::Interpolated] {
        let legacy = extract_gradients_with_recipe(&view, policy, 8192, recipe, || false).unwrap();
        let single =
            extract_spatial_gradients_with_recipe(&view, policy, 8192, recipe, (1, 1, 16), || false).unwrap();
        assert_eq!(format!("{legacy:?}"), format!("{single:?}"));
        let calls = std::cell::Cell::new(0usize);
        let distributed =
            extract_spatial_gradients_with_recipe(&view, policy, 8192, recipe, (2, 2, 3), || {
                calls.set(calls.get() + 1);
                false
            })
            .unwrap();
        let total = calls.get();
        let mut occupancy = [0usize; 4];
        assert!(!distributed.is_empty());
        for feature in &distributed {
            let x = feature.position[0] as usize;
            let y = feature.position[1] as usize;
            occupancy[(y * 2 / 96) * 2 + x * 2 / 96] += 1;
            let angle =
                rrrah_dedup::gradient::gradient_orientation(&view, [x as u32, y as u32], 256, || false)
                    .unwrap()
                    .unwrap();
            let independent = if recipe == GradientCellRecipe::Fixed {
                rrrah_dedup::gradient::describe_gradient(&view, feature.position, 1., angle, 256, || false)
            } else {
                rrrah_dedup::gradient::describe_gradient_interpolated(
                    &view,
                    feature.position,
                    1.,
                    angle,
                    256,
                    || false,
                )
            }
            .unwrap()
            .unwrap();
            assert_eq!(feature.descriptor.0, independent.0);
        }
        assert!(occupancy.iter().all(|n| *n <= 3));
        assert!(occupancy.iter().filter(|n| **n > 0).count() >= 2);
        for stop in [0, total / 2, total - 1] {
            calls.set(0);
            assert!(matches!(
                extract_spatial_gradients_with_recipe(&view, policy, 8192, recipe, (2, 2, 3), || {
                    let n = calls.get();
                    calls.set(n + 1);
                    n == stop
                }),
                Err(LocalError::Cancelled)
            ));
        }
        assert!(matches!(
            extract_spatial_gradients_with_recipe(&view, policy, 8191, recipe, (2, 2, 3), || false),
            Err(LocalError::Budget)
        ));
        assert!(matches!(
            extract_spatial_gradients_with_recipe(&view, policy, 8192, recipe, (0, 2, 3), || false),
            Err(LocalError::Invalid)
        ));
        assert!(matches!(
            extract_spatial_gradients_with_recipe(&view, policy, 8192, recipe, (17, 1, 3), || false),
            Err(LocalError::Budget)
        ));
    }
}

#[cfg(feature = "raster")]
#[test]
fn managed_spatial_pyramid_matches_unmanaged_and_releases_last_owner() {
    use rrrah_core::MemoryBudget;
    use rrrah_dedup::{
        gradient::{
            GradientCellRecipe, extract_spatial_gradient_pyramid_with_recipe,
            extract_spatial_gradient_pyramid_with_recipe_managed,
        },
        local::LocalPolicy,
        pyramid::PyramidPolicy,
    };
    let pixels: Vec<f32> = (0..64)
        .flat_map(|y| {
            (0..64).flat_map(move |x| {
                let v = ((x * x + 3 * y * y + 7 * x * y + 11 * x + 23 * y) % 137) as f32 / 137.;
                [v, v, v, 1.]
            })
        })
        .collect();
    let view = LinearRgbaView::new(64, 64, &pixels, 4096, || false).unwrap();
    let policy = PyramidPolicy {
        local: LocalPolicy {
            max_pixels: 4096,
            max_candidates: 4096,
            max_features: 16,
            minimum_corner_score: 0.0001,
        },
        max_levels: 2,
        max_total_pixels: 8192,
        max_total_features: 32,
    };
    for recipe in [GradientCellRecipe::Fixed, GradientCellRecipe::Interpolated] {
        let expected =
            extract_spatial_gradient_pyramid_with_recipe(&view, policy, 16384, recipe, (2, 2, 3), || false)
                .unwrap();
        assert!(!expected.is_empty());
        let budget = MemoryBudget::new(16 * 1024 * 1024);
        let calls = std::cell::Cell::new(0usize);
        let result = extract_spatial_gradient_pyramid_with_recipe_managed(
            &view,
            policy,
            16384,
            recipe,
            (2, 2, 3),
            &budget,
            || {
                calls.set(calls.get() + 1);
                false
            },
        )
        .unwrap();
        assert_eq!(format!("{expected:?}"), format!("{:?}", &result[..]));
        let total = calls.get();
        let peak = budget.peak();
        let retained = budget.used();
        assert!(retained > 0);
        let last = result.clone();
        drop(result);
        assert_eq!(budget.used(), retained);
        drop(last);
        assert_eq!(budget.used(), 0);
        let exact = MemoryBudget::new(peak);
        let result = extract_spatial_gradient_pyramid_with_recipe_managed(
            &view,
            policy,
            16384,
            recipe,
            (2, 2, 3),
            &exact,
            || false,
        )
        .unwrap();
        drop(result);
        assert_eq!(exact.used(), 0);
        let short = MemoryBudget::new(peak - 1);
        assert!(matches!(
            extract_spatial_gradient_pyramid_with_recipe_managed(
                &view,
                policy,
                16384,
                recipe,
                (2, 2, 3),
                &short,
                || false
            ),
            Err(LocalError::Budget)
        ));
        assert_eq!(short.used(), 0);
        for stop in [0, total / 2, total - 1] {
            calls.set(0);
            assert!(matches!(
                extract_spatial_gradient_pyramid_with_recipe_managed(
                    &view,
                    policy,
                    16384,
                    recipe,
                    (2, 2, 3),
                    &budget,
                    || {
                        let n = calls.get();
                        calls.set(n + 1);
                        n == stop
                    }
                ),
                Err(LocalError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
        let zero = MemoryBudget::new(0);
        assert!(matches!(
            extract_spatial_gradient_pyramid_with_recipe_managed(
                &view,
                policy,
                16384,
                recipe,
                (0, 2, 3),
                &zero,
                || false
            ),
            Err(LocalError::Invalid)
        ));
        assert_eq!(zero.peak(), 0);
    }
}
