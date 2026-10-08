use rrrah_dedup::{
    gradient::{describe_gradient, describe_gradient_interpolated},
    linear::LinearRgbaView,
    local::LocalError,
};
use std::cell::Cell;

#[test]
fn explicit_recipe_pyramid_preserves_positions_limits_cancel_and_managed_ownership() {
    use rrrah_dedup::{
        gradient::{GradientCellRecipe, extract_gradient_pyramid, extract_gradient_pyramid_with_recipe},
        local::LocalPolicy,
        pyramid::PyramidPolicy,
    };
    let mut seed = 7654321_u64;
    let pixels: Vec<f32> = (0..96 * 96)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let v = (seed & 255) as f32 / 256.;
            [v, v, v, 1.]
        })
        .collect();
    let view = LinearRgbaView::new(96, 96, &pixels, 9216, || false).unwrap();
    let policy = PyramidPolicy {
        local: LocalPolicy {
            max_pixels: 9216,
            max_candidates: 9216,
            max_features: 64,
            minimum_corner_score: 0.0001,
        },
        max_levels: 2,
        max_total_pixels: 11520,
        max_total_features: 128,
    };
    let cap = 64 * 2 * 512;
    let fixed = extract_gradient_pyramid(&view, policy, cap, || false).unwrap();
    assert_eq!(
        fixed,
        extract_gradient_pyramid_with_recipe(&view, policy, cap, GradientCellRecipe::Fixed, || false)
            .unwrap()
    );
    let calls = Cell::new(0);
    let smooth =
        extract_gradient_pyramid_with_recipe(&view, policy, cap, GradientCellRecipe::Interpolated, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    assert!(smooth.len() >= 10);
    assert_eq!(
        fixed.iter().map(|f| f.position).collect::<Vec<_>>(),
        smooth.iter().map(|f| f.position).collect::<Vec<_>>()
    );
    assert!(
        fixed
            .iter()
            .zip(&smooth)
            .any(|(a, b)| a.descriptor != b.descriptor)
    );
    assert!(matches!(
        extract_gradient_pyramid_with_recipe(
            &view,
            policy,
            cap - 1,
            GradientCellRecipe::Interpolated,
            || false
        ),
        Err(LocalError::Budget)
    ));
    for stop in [1, calls.get() / 2, calls.get()] {
        calls.set(0);
        assert!(matches!(
            extract_gradient_pyramid_with_recipe(
                &view,
                policy,
                cap,
                GradientCellRecipe::Interpolated,
                || {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }
            ),
            Err(LocalError::Cancelled)
        ));
    }
    assert_eq!(
        smooth,
        extract_gradient_pyramid_with_recipe(&view, policy, cap, GradientCellRecipe::Interpolated, || false)
            .unwrap()
    );
    #[cfg(feature = "raster")]
    {
        use rrrah_core::MemoryBudget;
        use rrrah_dedup::gradient::extract_gradient_pyramid_with_recipe_managed;
        let budget = MemoryBudget::new(64 * 1024 * 1024);
        let features = extract_gradient_pyramid_with_recipe_managed(
            &view,
            policy,
            cap,
            GradientCellRecipe::Interpolated,
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(&*features, smooth.as_slice());
        let peak = budget.peak();
        let retained = budget.used();
        assert!(retained > 0);
        let last = features.clone();
        drop(features);
        assert_eq!(budget.used(), retained);
        drop(last);
        assert_eq!(budget.used(), 0);
        let exact = MemoryBudget::new(peak);
        drop(
            extract_gradient_pyramid_with_recipe_managed(
                &view,
                policy,
                cap,
                GradientCellRecipe::Interpolated,
                &exact,
                || false,
            )
            .unwrap(),
        );
        assert_eq!(exact.used(), 0);
        let short = MemoryBudget::new(peak - 1);
        assert!(matches!(
            extract_gradient_pyramid_with_recipe_managed(
                &view,
                policy,
                cap,
                GradientCellRecipe::Interpolated,
                &short,
                || false
            ),
            Err(LocalError::Budget)
        ));
        assert_eq!(short.used(), 0);
        let fresh = MemoryBudget::new(64 * 1024 * 1024);
        assert!(matches!(
            extract_gradient_pyramid_with_recipe_managed(
                &view,
                policy,
                cap - 1,
                GradientCellRecipe::Interpolated,
                &fresh,
                || false
            ),
            Err(LocalError::Budget)
        ));
        assert_eq!(fresh.peak(), 0);
    }
}

#[test]
fn interpolated_cells_reduce_one_pixel_edge_localization_error() {
    let pixels: Vec<f32> = (0..64)
        .flat_map(|_| {
            (0..64).flat_map(|x| {
                let v = if x < 32 { 0.25 } else { 0.75 };
                [v, v, v, 1.]
            })
        })
        .collect();
    let view = LinearRgbaView::new(64, 64, &pixels, 4096, || false).unwrap();
    let distance =
        |a: &[f64; 128], b: &[f64; 128]| a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum::<f64>();
    let fixed_a = describe_gradient(&view, [31.5, 32.], 1., 0., 256, || false)
        .unwrap()
        .unwrap();
    let fixed_b = describe_gradient(&view, [32.5, 32.], 1., 0., 256, || false)
        .unwrap()
        .unwrap();
    let smooth_a = describe_gradient_interpolated(&view, [31.5, 32.], 1., 0., 256, || false)
        .unwrap()
        .unwrap();
    let smooth_b = describe_gradient_interpolated(&view, [32.5, 32.], 1., 0., 256, || false)
        .unwrap()
        .unwrap();
    let fixed = distance(&fixed_a.0, &fixed_b.0);
    let smooth = distance(&smooth_a.0, &smooth_b.0);
    assert!(smooth < fixed, "interpolated={smooth} fixed={fixed}");
    for d in [smooth_a, smooth_b] {
        assert!(d.0.iter().all(|v| v.is_finite() && *v >= 0.));
        assert!((d.0.iter().map(|v| v * v).sum::<f64>() - 1.).abs() < 1e-12);
    }
}

#[test]
fn interpolated_recipe_preserves_independent_quarter_turn_and_affine_light() {
    let mut seed = 123456789_u64;
    let original: Vec<f32> = (0..4096)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let v = (seed & 255) as f32 / 512.;
            [v, v, v, 1.]
        })
        .collect();
    let mut rotated = vec![0.; original.len()];
    for y in 0..64 {
        for x in 0..64 {
            let a = (y * 64 + x) * 4;
            let b = (x * 64 + 63 - y) * 4;
            rotated[b..b + 4].copy_from_slice(&original[a..a + 4]);
        }
    }
    let illuminated: Vec<f32> = original
        .chunks_exact(4)
        .flat_map(|p| {
            let v = p[0] * 0.5 + 0.125;
            [v, v, v, 1.]
        })
        .collect();
    let view = |p| LinearRgbaView::new(64, 64, p, 4096, || false).unwrap();
    let base = describe_gradient_interpolated(&view(&original), [32., 32.], 1., 0., 256, || false)
        .unwrap()
        .unwrap();
    for d in [
        describe_gradient_interpolated(
            &view(&rotated),
            [31., 32.],
            1.,
            std::f64::consts::FRAC_PI_2,
            256,
            || false,
        )
        .unwrap()
        .unwrap(),
        describe_gradient_interpolated(&view(&illuminated), [32., 32.], 1., 0., 256, || false)
            .unwrap()
            .unwrap(),
    ] {
        assert!(base.0.iter().zip(d.0).all(|(a, b)| (a - b).abs() < 1e-12));
    }
}

#[test]
fn interpolated_recipe_flat_alpha_work_and_cancellation_refusals() {
    let flat = [0.5, 0.5, 0.5, 1.].repeat(4096);
    let view = LinearRgbaView::new(64, 64, &flat, 4096, || false).unwrap();
    for angle in [0., 0.37, 1.1] {
        assert_eq!(
            describe_gradient_interpolated(&view, [32.25, 31.75], 1., angle, 256, || false).unwrap(),
            None
        );
    }
    assert!(matches!(
        describe_gradient_interpolated(&view, [32., 32.], 1., 0., 255, || false),
        Err(LocalError::Budget)
    ));
    let calls = Cell::new(0);
    describe_gradient_interpolated(&view, [32., 32.], 1., 0., 256, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(matches!(
            describe_gradient_interpolated(&view, [32., 32.], 1., 0., 256, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(LocalError::Cancelled)
        ));
    }
    assert_eq!(
        describe_gradient_interpolated(&view, [32., 32.], 1., 0., 256, || false).unwrap(),
        None
    );
    let mut transparent = flat;
    transparent[(32 * 64 + 32) * 4 + 3] = 0.;
    let view = LinearRgbaView::new(64, 64, &transparent, 4096, || false).unwrap();
    assert!(matches!(
        describe_gradient_interpolated(&view, [32., 32.], 1., 0., 256, || false),
        Err(LocalError::Invalid)
    ));
}
