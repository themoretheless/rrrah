use rrrah_dedup::{
    linear::LinearRgbaView,
    local::{FeatureRecipe, LocalError, LocalPolicy, MatchPolicy, match_features},
    ordinal::extract_rank_oriented,
};
fn pixels(mut seed: u64) -> Vec<f32> {
    (0..96 * 96)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let value = f32::from(u8::try_from(seed & 255).unwrap()) / 255.0;
            [value, value, value, 1.0]
        })
        .collect()
}
fn policy() -> LocalPolicy {
    LocalPolicy {
        max_pixels: 10000,
        max_candidates: 10000,
        max_features: 150,
        minimum_corner_score: 0.0001,
    }
}
#[test]
fn monotone_neutral_transfers_preserve_features_and_indexed_correspondences() {
    let source = pixels(1_234_567);
    let image = LinearRgbaView::new(96, 96, &source, 10000, || false).unwrap();
    let left = extract_rank_oriented(&image, policy(), || false).unwrap();
    assert!(left.len() > 10);
    assert!(
        left.iter()
            .all(|f| f.recipe == FeatureRecipe::RankOrientedScaleBriefV1)
    );
    for power in [0.5, 2.0, 3.0] {
        let transformed = source
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0].powf(power), p[1].powf(power), p[2].powf(power), 1.0])
            .collect::<Vec<_>>();
        let view = LinearRgbaView::new(96, 96, &transformed, 10000, || false).unwrap();
        let right = extract_rank_oriented(&view, policy(), || false).unwrap();
        assert_eq!(left.len(), right.len());
        for (a, b) in left.iter().zip(&right) {
            assert_eq!(a.position.map(f64::to_bits), b.position.map(f64::to_bits));
            assert_eq!(a.descriptor, b.descriptor);
            assert_eq!(a.quarter_turns, b.quarter_turns);
        }
        let exhaustive = match_features(
            &left,
            &right,
            MatchPolicy {
                max_comparisons: 22500,
                max_distance: 64,
            },
            || false,
        )
        .unwrap();
        let indexed = rrrah_dedup::local_index::match_features_indexed(
            &left,
            &right,
            rrrah_dedup::local_index::IndexedMatchPolicy {
                max_features_per_side: 150,
                max_hits: 45000,
                max_distance: 64,
            },
            || false,
        )
        .unwrap();
        assert!(exhaustive.len() > 10);
        assert_eq!(exhaustive.len(), indexed.len());
        for (a, b) in exhaustive.iter().zip(&indexed) {
            assert_eq!(a.source.map(f64::to_bits), b.source.map(f64::to_bits));
            assert_eq!(a.target.map(f64::to_bits), b.target.map(f64::to_bits));
        }
    }
    assert!(matches!(
        extract_rank_oriented(&image, policy(), || true),
        Err(LocalError::Cancelled)
    ));
    assert!(matches!(
        extract_rank_oriented(
            &image,
            LocalPolicy {
                max_pixels: 1,
                ..policy()
            },
            || false
        ),
        Err(LocalError::Budget)
    ));
    let unrelated = pixels(9_876_543);
    let view = LinearRgbaView::new(96, 96, &unrelated, 10000, || false).unwrap();
    let other = extract_rank_oriented(&view, policy(), || false).unwrap();
    let matches = match_features(
        &left,
        &other,
        MatchPolicy {
            max_comparisons: 22500,
            max_distance: 64,
        },
        || false,
    )
    .unwrap();
    assert!(matches.len() < 10);
}
