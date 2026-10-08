use rrrah_dedup::{
    gradient::{GradientCellRecipe, GradientMatchPolicy, match_gradients},
    gradient_scale::extract_gradient_scales,
    linear::LinearRgbaView,
    local::{LocalError, LocalPolicy},
    pyramid::PyramidPolicy,
};
fn fixture(name: &str) -> Vec<f32> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/rotation/{name}.rgba")),
    )
    .unwrap()
    .into_iter()
    .map(|v| f32::from(v) / 255.)
    .collect()
}
fn policy() -> PyramidPolicy {
    PyramidPolicy {
        local: LocalPolicy {
            max_pixels: 100000,
            max_candidates: 100000,
            max_features: 500,
            minimum_corner_score: 0.0001,
        },
        max_levels: 4,
        max_total_pixels: 100000,
        max_total_features: 2000,
    }
}
#[test]
fn intermediate_scale_recovers_authored_reduction_without_acceptance_relaxation() {
    let a = fixture("base");
    let b = fixture("scale-117-angle-0");
    let n = fixture("unrelated");
    let av = LinearRgbaView::new(160, 160, &a, 25600, || false).unwrap();
    let bv = LinearRgbaView::new(117, 117, &b, 13689, || false).unwrap();
    let nv = LinearRgbaView::new(160, 160, &n, 25600, || false).unwrap();
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let extract = |v: &LinearRgbaView<'_>| {
        extract_gradient_scales(
            v,
            policy(),
            &factors,
            1024000,
            GradientCellRecipe::Interpolated,
            || false,
        )
        .unwrap()
    };
    let dyadic = |v: &LinearRgbaView<'_>| {
        extract_gradient_scales(
            v,
            policy(),
            &[1., 2., 4.],
            1024000,
            GradientCellRecipe::Interpolated,
            || false,
        )
        .unwrap()
    };
    let baseline = match_gradients(
        &dyadic(&av),
        &dyadic(&bv),
        GradientMatchPolicy {
            max_comparisons: 4000000,
            max_squared_distance: 0.5,
            squared_ratio: 0.64,
        },
        || false,
    )
    .unwrap();
    let baseline_correct = baseline
        .iter()
        .filter(|m| {
            m.source
                .iter()
                .zip(m.target)
                .map(|(s, t)| ((s + 0.5) * 0.73125 - 0.5 - t).powi(2))
                .sum::<f64>()
                <= 4.
        })
        .count();
    println!(
        "same_recipe_dyadic_matches={} correct={}",
        baseline.len(),
        baseline_correct
    );
    let af = extract(&av);
    let bf = extract(&bv);
    let nf = extract(&nv);
    let matching = GradientMatchPolicy {
        max_comparisons: 4000000,
        max_squared_distance: 0.5,
        squared_ratio: 0.64,
    };
    let hits = match_gradients(&af, &bf, matching, || false).unwrap();
    let correct = hits
        .iter()
        .filter(|m| {
            m.source
                .iter()
                .zip(m.target)
                .map(|(s, t)| ((s + 0.5) * 0.73125 - 0.5 - t).powi(2))
                .sum::<f64>()
                <= 4.
        })
        .count();
    let negative = match_gradients(&af, &nf, matching, || false).unwrap();
    println!(
        "positive_matches={} correct={} negative_matches={}",
        hits.len(),
        correct,
        negative.len()
    );
    assert!(correct > baseline_correct);
    assert!(
        correct >= 10,
        "insufficient independent known-transform correspondences: {correct}"
    );
    let geometry = rrrah_dedup::geometry::GeometryPolicy {
        tolerance: 2.,
        min_inliers: 10,
        max_points: 2000,
        max_hypotheses: 4096,
    };
    let sampling = rrrah_dedup::geometry::ProjectiveSamplingPolicy {
        trials: 4096,
        seed: 0x1234abcd,
    };
    let model = rrrah_dedup::geometry::verify_projective_sampled(&hits, geometry, sampling, || false)
        .unwrap()
        .unwrap();
    for x in [32., 64., 96., 128.] {
        for y in [32., 64., 96., 128.] {
            let target = model.transform.apply([x, y]).unwrap();
            let expected = [(x + 0.5) * 0.73125 - 0.5, (y + 0.5) * 0.73125 - 0.5];
            assert!(
                target
                    .iter()
                    .zip(expected)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    < 4.
            );
        }
    }
    assert!(
        rrrah_dedup::geometry::verify_projective_sampled(&negative, geometry, sampling, || false)
            .unwrap()
            .is_none()
    );
}
#[test]
fn admission_and_cancellation_return_no_partial_features() {
    let a = fixture("base");
    let v = LinearRgbaView::new(160, 160, &a, 25600, || false).unwrap();
    let factors = [1., std::f64::consts::SQRT_2];
    assert!(matches!(
        extract_gradient_scales(&v, policy(), &factors, 511999, GradientCellRecipe::Fixed, || {
            false
        }),
        Err(LocalError::Budget)
    ));
    for bad in [&[1., 1.][..], &[0.5][..], &[f64::NAN][..]] {
        assert!(matches!(
            extract_gradient_scales(&v, policy(), bad, 1024000, GradientCellRecipe::Fixed, || false),
            Err(LocalError::Invalid)
        ));
    }
    let mut exact = policy();
    exact.max_total_pixels = 38369;
    exact.max_total_features = 1000;
    extract_gradient_scales(&v, exact, &factors, 512000, GradientCellRecipe::Fixed, || false).unwrap();
    for limited in [
        PyramidPolicy {
            max_total_pixels: 38368,
            ..exact
        },
        PyramidPolicy {
            max_total_features: 999,
            ..exact
        },
    ] {
        assert!(matches!(
            extract_gradient_scales(&v, limited, &factors, 512000, GradientCellRecipe::Fixed, || false),
            Err(LocalError::Budget)
        ));
    }
    let calls = std::cell::Cell::new(0);
    extract_gradient_scales(&v, policy(), &factors, 512000, GradientCellRecipe::Fixed, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    for stop in [0, total / 2, total - 1] {
        calls.set(0);
        assert!(matches!(
            extract_gradient_scales(&v, policy(), &factors, 512000, GradientCellRecipe::Fixed, || {
                let n = calls.get();
                calls.set(n + 1);
                n == stop
            }),
            Err(LocalError::Cancelled)
        ));
    }
}

#[cfg(feature = "raster")]
#[test]
fn managed_scales_exact_peak_last_owner_and_cancellation() {
    use rrrah_core::MemoryBudget;
    use rrrah_dedup::gradient_scale::extract_gradient_scales_managed;
    let pixels = fixture("base");
    let image = LinearRgbaView::new(160, 160, &pixels, 25600, || false).unwrap();
    let factors = [1., std::f64::consts::SQRT_2];
    let budget = MemoryBudget::new(16 * 1024 * 1024);
    let calls = std::cell::Cell::new(0usize);
    let result = extract_gradient_scales_managed(
        &image,
        policy(),
        &factors,
        512000,
        GradientCellRecipe::Interpolated,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let total = calls.get();
    let peak = budget.peak();
    let used = budget.used();
    assert!(used > 0 && peak > used);
    let expected = extract_gradient_scales(
        &image,
        policy(),
        &factors,
        512000,
        GradientCellRecipe::Interpolated,
        || false,
    )
    .unwrap();
    assert_eq!(format!("{:?}", &result[..]), format!("{expected:?}"));
    let clone = result.clone();
    drop(result);
    assert_eq!(budget.used(), used);
    drop(clone);
    assert_eq!(budget.used(), 0);
    for limit in [peak, peak - 1] {
        let b = MemoryBudget::new(limit);
        let r = extract_gradient_scales_managed(
            &image,
            policy(),
            &factors,
            512000,
            GradientCellRecipe::Interpolated,
            &b,
            || false,
        );
        if limit == peak {
            assert!(r.is_ok());
        } else {
            assert!(matches!(r, Err(LocalError::Budget)));
        }
        drop(r);
        assert_eq!(b.used(), 0);
    }
    for stop in [0, total / 2, total - 1] {
        let b = MemoryBudget::new(peak);
        calls.set(0);
        assert!(matches!(
            extract_gradient_scales_managed(
                &image,
                policy(),
                &factors,
                512000,
                GradientCellRecipe::Interpolated,
                &b,
                || {
                    let n = calls.get();
                    calls.set(n + 1);
                    n == stop
                }
            ),
            Err(LocalError::Cancelled)
        ));
        assert_eq!(b.used(), 0);
    }
}

#[cfg(feature = "raster")]
#[test]
fn scale_admission_can_cancel_before_allocating() {
    let pixels = fixture("base");
    let view = LinearRgbaView::new(160, 160, &pixels, 25600, || false).unwrap();
    let calls = std::cell::Cell::new(0);
    let budget = rrrah_core::MemoryBudget::new(16 * 1024 * 1024);
    let result = rrrah_dedup::gradient_scale::extract_gradient_scales_managed(
        &view,
        policy(),
        &[1., std::f64::consts::SQRT_2, 2.],
        1024000,
        GradientCellRecipe::Interpolated,
        &budget,
        || {
            let n = calls.get();
            calls.set(n + 1);
            n == 1
        },
    );
    assert!(matches!(result, Err(LocalError::Cancelled)));
    assert_eq!(calls.get(), 2);
    assert_eq!(budget.peak(), 0);
}

#[cfg(feature = "raster")]
#[test]
fn spatial_scale_selection_recipe_limits_and_owned_memory() {
    use rrrah_dedup::gradient_scale::{
        extract_spatial_gradient_scales, extract_spatial_gradient_scales_managed,
    };
    let pixels = fixture("base");
    let view = LinearRgbaView::new(160, 160, &pixels, 25600, || false).unwrap();
    let factors = [1., std::f64::consts::SQRT_2];
    for recipe in [GradientCellRecipe::Fixed, GradientCellRecipe::Interpolated] {
        let global = extract_gradient_scales(&view, policy(), &factors, 512000, recipe, || false).unwrap();
        let single =
            extract_spatial_gradient_scales(&view, policy(), &factors, 512000, recipe, (1, 1, 500), || false)
                .unwrap();
        assert_eq!(format!("{global:?}"), format!("{single:?}"));
        let budget = rrrah_core::MemoryBudget::new(16 * 1024 * 1024);
        let selected = extract_spatial_gradient_scales_managed(
            &view,
            policy(),
            &factors,
            512000,
            recipe,
            (2, 2, 8),
            &budget,
            || false,
        )
        .unwrap();
        let reference =
            extract_spatial_gradient_scales(&view, policy(), &factors, 512000, recipe, (2, 2, 8), || false)
                .unwrap();
        assert!(!selected.is_empty() && selected.len() <= 64);
        assert_eq!(format!("{:?}", &selected[..]), format!("{reference:?}"));
        let peak = budget.peak();
        let clone = selected.clone();
        drop(selected);
        assert!(budget.used() > 0);
        drop(clone);
        assert_eq!(budget.used(), 0);
        let short = rrrah_core::MemoryBudget::new(peak - 1);
        assert!(matches!(
            extract_spatial_gradient_scales_managed(
                &view,
                policy(),
                &factors,
                512000,
                recipe,
                (2, 2, 8),
                &short,
                || false
            ),
            Err(LocalError::Budget)
        ));
        assert_eq!(short.used(), 0);
    }
    assert!(matches!(
        extract_spatial_gradient_scales(
            &view,
            policy(),
            &factors,
            512000,
            GradientCellRecipe::Fixed,
            (0, 2, 8),
            || false
        ),
        Err(LocalError::Invalid)
    ));
}

#[cfg(feature = "raster")]
#[test]
fn area_scale_matches_independent_box_reference_and_managed_lifecycle() {
    use rrrah_dedup::gradient_scale::extract_spatial_gradient_scales_area_managed as extract;
    let pixels = fixture("base");
    let view = LinearRgbaView::new(160, 160, &pixels, 25600, || false).unwrap();
    let mut reduced = Vec::new();
    for y in 0..80usize {
        for x in 0..80usize {
            for c in 0..4 {
                let sum = [
                    (2 * x, 2 * y),
                    (2 * x + 1, 2 * y),
                    (2 * x, 2 * y + 1),
                    (2 * x + 1, 2 * y + 1),
                ]
                .into_iter()
                .map(|(sx, sy)| f64::from(pixels[(sy * 160 + sx) * 4 + c]))
                .sum::<f64>();
                reduced.push((sum / 4.) as f32);
            }
        }
    }
    let reference_view = LinearRgbaView::new(80, 80, &reduced, 6400, || false).unwrap();
    let mut reference = rrrah_dedup::gradient::extract_spatial_gradients_with_recipe(
        &reference_view,
        policy().local,
        256000,
        GradientCellRecipe::Interpolated,
        (4, 4, 32),
        || false,
    )
    .unwrap();
    for feature in &mut reference {
        feature.position = feature.position.map(|p| (p + 0.5) * 2. - 0.5);
    }
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0usize);
    let actual = extract(
        &view,
        policy(),
        &[2.],
        256000,
        GradientCellRecipe::Interpolated,
        (4, 4, 32),
        25600,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert!(!actual.is_empty());
    assert_eq!(format!("{:?}", &*actual), format!("{reference:?}"));
    let total = calls.get();
    let used = budget.used();
    assert!(used > 0);
    let owner = actual.clone();
    drop(actual);
    assert_eq!(budget.used(), used);
    drop(owner);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        let result = extract(
            &view,
            policy(),
            &[2.],
            256000,
            GradientCellRecipe::Interpolated,
            (4, 4, 32),
            25600,
            &budget,
            || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        );
        assert!(matches!(result, Err(LocalError::Cancelled)), "stop={stop}");
        drop(result);
        assert_eq!(budget.used(), 0);
    }
    let fresh = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    assert!(matches!(
        extract(
            &view,
            policy(),
            &[2.],
            256000,
            GradientCellRecipe::Interpolated,
            (4, 4, 32),
            25599,
            &fresh,
            || false
        ),
        Err(LocalError::Budget)
    ));
    assert_eq!(fresh.peak(), 0);
}
