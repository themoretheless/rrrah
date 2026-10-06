use rrrah_dedup::{
    geometry::{GeometryPolicy, verify_similarity},
    linear::LinearRgbaView,
    local::{LocalPolicy, MatchPolicy, extract, match_features},
    warp::{WarpPolicy, verify_bidirectional},
};
const EXTRACT: LocalPolicy = LocalPolicy {
    max_pixels: 10000,
    max_candidates: 10000,
    max_features: 200,
    minimum_corner_score: 0.01,
};
const MATCH: MatchPolicy = MatchPolicy {
    max_comparisons: 100_000,
    max_distance: 40,
};
fn image(seed: u64) -> Vec<f32> {
    let mut seed = seed;
    (0..64 * 64)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let value = f32::from(u8::try_from(seed & 255).unwrap()) / 255.0;
            [value, value, value, 1.0]
        })
        .collect()
}
#[test]
fn extracted_crop_matches_drive_geometry_and_pixel_verification() {
    let pixels = image(1_234_567);
    let mut cropped = Vec::new();
    for y in 10..54 {
        cropped.extend_from_slice(&pixels[(y * 64 + 10) * 4..(y * 64 + 54) * 4]);
    }
    let source = LinearRgbaView::new(64, 64, &pixels, 4096, || false).unwrap();
    let target = LinearRgbaView::new(44, 44, &cropped, 1936, || false).unwrap();
    let left = extract(&source, EXTRACT, || false).unwrap();
    let right = extract(&target, EXTRACT, || false).unwrap();
    let matches = match_features(&left, &right, MATCH, || false).unwrap();
    assert!(matches.len() >= 4);
    let geometry = verify_similarity(
        &matches,
        GeometryPolicy {
            tolerance: 0.01,
            min_inliers: 4,
            max_points: 200,
            max_hypotheses: 20000,
        },
        || false,
    )
    .unwrap()
    .unwrap();
    assert!((geometry.transform.translation[0] + 10.0).abs() < 1e-6);
    assert!((geometry.transform.translation[1] + 10.0).abs() < 1e-6);
    let pixels = verify_bidirectional(
        &source,
        &target,
        geometry.transform,
        WarpPolicy {
            tolerance: 1e-6,
            max_source_pixels: 10000,
        },
        || false,
    )
    .unwrap();
    assert_eq!(pixels.forward.matched_pixels, 1936);
    assert_eq!(pixels.reverse.matched_pixels, 1936);
    let unrelated = image(999);
    let unrelated = LinearRgbaView::new(64, 64, &unrelated, 4096, || false).unwrap();
    let negative = extract(&unrelated, EXTRACT, || false).unwrap();
    assert!(
        match_features(&left, &negative, MATCH, || false)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn flat_images_ambiguous_descriptors_budgets_and_cancel_are_explicit() {
    use rrrah_dedup::local::{Feature, LocalError};
    let pixels = vec![0.0; 32 * 32 * 4];
    let view = LinearRgbaView::new(32, 32, &pixels, 1024, || false).unwrap();
    assert!(extract(&view, EXTRACT, || false).unwrap().is_empty());
    assert_eq!(
        extract(&view, EXTRACT, || true).unwrap_err(),
        LocalError::Cancelled
    );
    assert_eq!(
        extract(
            &view,
            LocalPolicy {
                max_pixels: 1,
                ..EXTRACT
            },
            || false
        )
        .unwrap_err(),
        LocalError::Budget
    );
    let a = Feature {
        recipe: rrrah_dedup::local::FeatureRecipe::QuarterTurnBriefV1,
        position: [0.0; 2],
        descriptor: [0; 4],
        quarter_turns: [[0; 4]; 3],
    };
    let b = Feature {
        recipe: rrrah_dedup::local::FeatureRecipe::QuarterTurnBriefV1,
        position: [1.0; 2],
        descriptor: [0; 4],
        quarter_turns: [[0; 4]; 3],
    };
    assert!(
        match_features(std::slice::from_ref(&a), &[a.clone(), b], MATCH, || false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        match_features(
            std::slice::from_ref(&a),
            std::slice::from_ref(&a),
            MatchPolicy {
                max_comparisons: 0,
                ..MATCH
            },
            || false
        )
        .unwrap_err(),
        LocalError::Budget
    );
}

#[test]
fn quarter_turn_crops_are_found_without_manual_correspondences() {
    let pixels = image(1_234_567);
    let source = LinearRgbaView::new(64, 64, &pixels, 4096, || false).unwrap();
    let left = extract(&source, EXTRACT, || false).unwrap();
    let mut crop = Vec::new();
    for y in 10..54 {
        crop.extend_from_slice(&pixels[(y * 64 + 10) * 4..(y * 64 + 54) * 4]);
    }
    for turn in 1..=3 {
        let mut rotated = vec![0.0; crop.len()];
        for y in 0..44 {
            for x in 0..44 {
                let (tx, ty) = match turn {
                    1 => (43 - y, x),
                    2 => (43 - x, 43 - y),
                    _ => (y, 43 - x),
                };
                rotated[(ty * 44 + tx) * 4..(ty * 44 + tx) * 4 + 4]
                    .copy_from_slice(&crop[(y * 44 + x) * 4..(y * 44 + x) * 4 + 4]);
            }
        }
        let target = LinearRgbaView::new(44, 44, &rotated, 1936, || false).unwrap();
        let right = extract(&target, EXTRACT, || false).unwrap();
        let matches = match_features(&left, &right, MATCH, || false).unwrap();
        assert!(matches.len() >= 4, "turn {turn}");
        let geometry = verify_similarity(
            &matches,
            GeometryPolicy {
                tolerance: 0.01,
                min_inliers: 4,
                max_points: 200,
                max_hypotheses: 20000,
            },
            || false,
        )
        .unwrap()
        .unwrap();
        let evidence = verify_bidirectional(
            &source,
            &target,
            geometry.transform,
            WarpPolicy {
                tolerance: 1e-6,
                max_source_pixels: 10000,
            },
            || false,
        )
        .unwrap();
        assert_eq!(evidence.forward.matched_pixels, 1936, "turn {turn}");
        assert_eq!(evidence.reverse.matched_pixels, 1936, "turn {turn}");
    }
}
#[test]
fn one_shot_matching_cancellation_discards_every_partial_result_and_retries() {
    use rrrah_dedup::local::{Feature, FeatureRecipe, LocalError};
    let features = [0_u64, u64::MAX, 0x5555_5555_5555_5555, 0xaaaa_aaaa_aaaa_aaaa]
        .into_iter()
        .enumerate()
        .map(|(i, word)| Feature {
            recipe: FeatureRecipe::OrientedBriefV1,
            position: [i as f64, 1.0],
            descriptor: [word; 4],
            quarter_turns: [[word; 4]; 3],
        })
        .collect::<Vec<_>>();
    let policy = MatchPolicy {
        max_comparisons: 16,
        max_distance: 0,
    };
    let checks = std::cell::Cell::new(0);
    let expected = match_features(&features, &features, policy, || {
        checks.set(checks.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(expected.len(), 4);
    for checkpoint in 1..=checks.get() {
        let current = std::cell::Cell::new(0);
        assert_eq!(
            match_features(&features, &features, policy, || {
                current.set(current.get() + 1);
                current.get() == checkpoint
            })
            .unwrap_err(),
            LocalError::Cancelled
        );
    }
    let retry = match_features(&features, &features, policy, || false).unwrap();
    assert_eq!(retry.len(), expected.len());
    for (actual, expected) in retry.iter().zip(expected) {
        assert_eq!(actual.source, expected.source);
        assert_eq!(actual.target, expected.target);
    }
}
