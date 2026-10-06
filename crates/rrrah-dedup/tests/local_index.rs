use rrrah_dedup::{
    local::{Feature, FeatureRecipe, LocalError},
    local_index::{DescriptorCandidate, LocalDescriptorIndex},
};
fn feature(recipe: FeatureRecipe, seed: u64) -> Feature {
    let mut state = seed.wrapping_add(7);
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut values = [[0u64; 4]; 4];
    for variant in &mut values {
        for word in variant {
            *word = next();
        }
    }
    Feature {
        recipe,
        position: [0.0, 0.0],
        descriptor: values[0],
        quarter_turns: [values[1], values[2], values[3]],
    }
}
fn distance(left: &Feature, right: &Feature) -> u32 {
    let left_count = if matches!(
        left.recipe,
        FeatureRecipe::OrientedScaleBriefV1 | FeatureRecipe::RankOrientedScaleBriefV1
    ) {
        4
    } else {
        1
    };
    let lv = [
        left.descriptor,
        left.quarter_turns[0],
        left.quarter_turns[1],
        left.quarter_turns[2],
    ];
    let rv = [
        right.descriptor,
        right.quarter_turns[0],
        right.quarter_turns[1],
        right.quarter_turns[2],
    ];
    lv[..left_count]
        .iter()
        .flat_map(|a| {
            rv.iter()
                .map(move |b| a.iter().zip(b).map(|(x, y)| (x ^ y).count_ones()).sum::<u32>())
        })
        .min()
        .unwrap()
}
#[test]
fn descriptor_index_equals_independent_exhaustive_variant_search() {
    for recipe in [
        FeatureRecipe::QuarterTurnBriefV1,
        FeatureRecipe::OrientedBriefV1,
        FeatureRecipe::OrientedScaleBriefV1,
        FeatureRecipe::RankOrientedScaleBriefV1,
    ] {
        let mut entries = (0..64).map(|id| (id, feature(recipe, id))).collect::<Vec<_>>();
        entries.push((u64::MAX, feature(recipe, 3)));
        let index = LocalDescriptorIndex::new(entries.clone(), 65, || false).unwrap();
        for seed in [0, 3, 27, 91] {
            let query = feature(recipe, seed);
            for radius in [0, 1, 16, 64, 96, 128, 256, u32::MAX] {
                let mut expected = entries
                    .iter()
                    .filter_map(|(id, f)| {
                        let d = distance(&query, f);
                        (d <= radius.min(256)).then_some(DescriptorCandidate { id: *id, distance: d })
                    })
                    .collect::<Vec<_>>();
                expected.sort_unstable_by_key(|c| (c.distance, c.id));
                assert_eq!(
                    index.search(&query, radius, 65, || false).unwrap(),
                    expected,
                    "{recipe:?} {seed} {radius}"
                );
            }
        }
    }
}
#[test]
fn collision_buckets_recipes_and_limits_never_return_partial_results() {
    let recipe = FeatureRecipe::OrientedScaleBriefV1;
    let query = feature(recipe, 3);
    let index = LocalDescriptorIndex::new([(1, query), (2, query)], 2, || false).unwrap();
    assert!(matches!(
        index.search(&query, 0, 1, || false),
        Err(LocalError::Budget)
    ));
    assert!(matches!(
        index.search(&query, 0, 2, || true),
        Err(LocalError::Cancelled)
    ));
    let calls = std::cell::Cell::new(0usize);
    assert!(matches!(
        index.search(&query, 256, 2, || {
            calls.set(calls.get() + 1);
            calls.get() == 7
        }),
        Err(LocalError::Cancelled)
    ));
    assert!(matches!(
        index.search(&feature(FeatureRecipe::QuarterTurnBriefV1, 3), 0, 2, || false),
        Err(LocalError::Invalid)
    ));
    assert!(matches!(
        LocalDescriptorIndex::new([(1, query), (1, query)], 2, || false),
        Err(LocalError::Invalid)
    ));
    assert!(matches!(
        LocalDescriptorIndex::new([(1, query)], 0, || false),
        Err(LocalError::Budget)
    ));
    assert!(matches!(
        LocalDescriptorIndex::new(
            [(1, query), (2, feature(FeatureRecipe::QuarterTurnBriefV1, 3))],
            2,
            || false
        ),
        Err(LocalError::Invalid)
    ));
    assert!(matches!(
        LocalDescriptorIndex::new([(1, query)], 2, || true),
        Err(LocalError::Cancelled)
    ));
}

#[test]
fn indexed_mutual_matching_equals_exhaustive_ratios_for_every_recipe() {
    use rrrah_dedup::{
        local::{MatchPolicy, match_features},
        local_index::{IndexedMatchPolicy, match_features_indexed},
    };
    for recipe in [
        FeatureRecipe::QuarterTurnBriefV1,
        FeatureRecipe::OrientedBriefV1,
        FeatureRecipe::OrientedScaleBriefV1,
        FeatureRecipe::RankOrientedScaleBriefV1,
    ] {
        let left = (0..24)
            .map(|id| {
                let mut f = feature(recipe, id);
                f.position = [f64::from(u32::try_from(id).unwrap()), 0.0];
                f
            })
            .collect::<Vec<_>>();
        let mut right = (0..30)
            .map(|id| {
                let mut f = feature(recipe, id);
                f.position = [0.0, f64::from(u32::try_from(id).unwrap())];
                f
            })
            .collect::<Vec<_>>();
        right.push(right[3]);
        for maximum in [0, 1, 32, 64, 96, 128, 256, u32::MAX] {
            let expected = match_features(
                &left,
                &right,
                MatchPolicy {
                    max_comparisons: 1000,
                    max_distance: maximum,
                },
                || false,
            )
            .unwrap();
            let actual = match_features_indexed(
                &left,
                &right,
                IndexedMatchPolicy {
                    max_features_per_side: 31,
                    max_hits: 2000,
                    max_distance: maximum,
                },
                || false,
            )
            .unwrap();
            assert_eq!(positions(actual), positions(expected), "{recipe:?} {maximum}");
        }
    }
}
#[test]
fn reverse_matching_preserves_asymmetric_variant_metric_and_hit_limits() {
    use rrrah_dedup::{
        local::{MatchPolicy, match_features},
        local_index::{IndexedMatchPolicy, match_features_indexed},
    };
    let mut left = feature(FeatureRecipe::QuarterTurnBriefV1, 7);
    left.descriptor = [0; 4];
    left.quarter_turns = [[1; 4]; 3];
    let mut right = left;
    right.descriptor = [u64::MAX; 4];
    right.quarter_turns = [[0; 4]; 3];
    right.position = [1.0, 2.0];
    let policy = IndexedMatchPolicy {
        max_features_per_side: 1,
        max_hits: 2,
        max_distance: 0,
    };
    let expected = match_features(
        &[left],
        &[right],
        MatchPolicy {
            max_comparisons: 1,
            max_distance: 0,
        },
        || false,
    )
    .unwrap();
    assert_eq!(expected.len(), 1);
    assert_eq!(
        positions(match_features_indexed(&[left], &[right], policy, || false).unwrap()),
        positions(expected)
    );
    assert!(matches!(
        match_features_indexed(
            &[left],
            &[right],
            IndexedMatchPolicy {
                max_hits: 1,
                ..policy
            },
            || false
        ),
        Err(LocalError::Budget)
    ));
    assert!(matches!(
        match_features_indexed(&[left], &[right], policy, || true),
        Err(LocalError::Cancelled)
    ));
}

fn positions(matches: Vec<rrrah_dedup::geometry::Correspondence>) -> Vec<[[f64; 2]; 2]> {
    matches.into_iter().map(|c| [c.source, c.target]).collect()
}

#[cfg(feature = "decode")]
#[test]
fn extracted_rotation_and_scale_features_keep_exhaustive_correspondences() {
    use rrrah_dedup::{
        local::{LocalPolicy, MatchPolicy, extract_multiscale_oriented, match_features},
        local_index::{IndexedMatchPolicy, match_features_indexed},
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let source = rrrah_dedup::decode::decode_selected_frame(
        &rrrah_decode::DecodeRequest::new(root.join("base.png")),
        200_000,
        &budget,
        || false,
    )
    .unwrap();
    let target = rrrah_dedup::decode::decode_selected_frame(
        &rrrah_decode::DecodeRequest::new(root.join("scale-216-angle-17.png")),
        200_000,
        &budget,
        || false,
    )
    .unwrap();
    let extraction = LocalPolicy {
        max_pixels: 200_000,
        max_candidates: 200_000,
        max_features: 500,
        minimum_corner_score: 0.0001,
    };
    let left = extract_multiscale_oriented(&source.view(|| false).unwrap(), extraction, || false).unwrap();
    let right = extract_multiscale_oriented(&target.view(|| false).unwrap(), extraction, || false).unwrap();
    let expected = match_features(
        &left,
        &right,
        MatchPolicy {
            max_comparisons: 250_000,
            max_distance: 64,
        },
        || false,
    )
    .unwrap();
    assert!(expected.len() >= 10);
    let actual = match_features_indexed(
        &left,
        &right,
        IndexedMatchPolicy {
            max_features_per_side: 500,
            max_hits: 500_000,
            max_distance: 64,
        },
        || false,
    )
    .unwrap();
    assert_eq!(positions(actual), positions(expected));
    drop(source);
    drop(target);
    assert_eq!(budget.used(), 0);
}

#[test]
fn file_pair_retrieval_equals_exhaustive_distinct_query_feature_counts() {
    use rrrah_dedup::local_index::{FileFeatureBudgets, descriptor_file_pairs};
    for recipe in [
        FeatureRecipe::QuarterTurnBriefV1,
        FeatureRecipe::OrientedBriefV1,
        FeatureRecipe::OrientedScaleBriefV1,
        FeatureRecipe::RankOrientedScaleBriefV1,
    ] {
        let files = [
            (10, (0..6).map(|s| feature(recipe, s)).collect::<Vec<_>>()),
            (20, (3..9).map(|s| feature(recipe, s)).collect()),
            (u64::MAX, (60..66).map(|s| feature(recipe, s)).collect()),
            (0, Vec::new()),
        ];
        for radius in [0, 64, 128, 256] {
            for minimum in [1, 3, 6] {
                let mut expected = Vec::new();
                for (left, a) in &files {
                    for (right, b) in &files {
                        if left < right
                            && a.iter()
                                .filter(|q| b.iter().any(|t| distance(q, t) <= radius))
                                .count()
                                >= minimum
                        {
                            expected.push((*left, *right));
                        }
                    }
                }
                expected.sort_unstable();
                let result = descriptor_file_pairs(
                    files.clone(),
                    radius,
                    minimum,
                    FileFeatureBudgets {
                        max_files: 4,
                        max_features: 18,
                        max_hits: 324,
                        max_pair_counts: 6,
                        max_pairs: 6,
                    },
                    || false,
                )
                .unwrap();
                assert_eq!(result.pairs, expected, "{recipe:?} {radius} {minimum}");
                assert!(result.insufficient_features.contains(&0));
            }
        }
    }
}

#[test]
fn file_pair_storage_limits_preserve_sorted_complete_results() {
    use rrrah_dedup::local_index::{FileFeatureBudgets, descriptor_file_pairs};
    let files = [30, 10, 20].map(|id| (id, vec![feature(FeatureRecipe::OrientedBriefV1, 7)]));
    let limits = FileFeatureBudgets {
        max_files: 3,
        max_features: 3,
        max_hits: 9,
        max_pair_counts: 3,
        max_pairs: 3,
    };
    let run = |budgets| descriptor_file_pairs(files.clone(), 0, 1, budgets, || false);
    assert_eq!(run(limits).unwrap().pairs, [(10, 20), (10, 30), (20, 30)]);
    for reduced in [
        FileFeatureBudgets {
            max_files: 2,
            ..limits
        },
        FileFeatureBudgets {
            max_features: 2,
            ..limits
        },
        FileFeatureBudgets {
            max_hits: 8,
            ..limits
        },
        FileFeatureBudgets {
            max_pair_counts: 2,
            ..limits
        },
        FileFeatureBudgets {
            max_pairs: 2,
            ..limits
        },
    ] {
        assert!(matches!(run(reduced), Err(LocalError::Budget)));
    }
    assert_eq!(run(limits).unwrap().pairs, [(10, 20), (10, 30), (20, 30)]);
    let duplicate = [(10, Vec::new()), (10, Vec::new())];
    assert!(matches!(
        descriptor_file_pairs(duplicate, 0, 1, limits, || false),
        Err(LocalError::Invalid)
    ));
}
