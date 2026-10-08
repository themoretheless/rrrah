#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::GeometryPolicy,
    local::{LocalPolicy, MatchPolicy},
    local_scan::LocalFilePolicy,
    warp::WarpPolicy,
};
fn policy() -> LocalFilePolicy {
    LocalFilePolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 200_000,
            max_file_bytes: 1024 * 1024,
        },
        extract: LocalPolicy {
            max_pixels: 200_000,
            max_candidates: 200_000,
            max_features: 32,
            minimum_corner_score: 0.0001,
        },
        matching: MatchPolicy {
            max_comparisons: 4096,
            max_distance: 32,
        },
        geometry: GeometryPolicy {
            tolerance: 1e-5,
            min_inliers: 6,
            max_points: 32,
            max_hypotheses: 40_000,
        },
        pixels: WarpPolicy {
            tolerance: 1e-5,
            max_source_pixels: 400_000,
        },
        minimum_compared_pixels: 1000,
        minimum_coverage_fraction: 0.9,
        minimum_matched_fraction: 1.0,
    }
}

#[test]
fn five_search_collection_matches_direct_and_source_lifecycle() {
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local_scan::{
            LocalFileError, ProjectiveComplementaryFilePolicy, ProjectivePortfolioFilePolicy,
            ProjectivePyramidPhotometricFilePolicy,
        },
        warp::{
            ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy,
            ProjectiveRegistrationPolicy, ProjectiveRegistrationPortfolioPolicy,
            ProjectiveRegistrationTrustPolicy,
        },
    };
    use std::{cell::Cell, io::Write};
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let dir = tempfile::tempdir().unwrap();
    let a_path = dir.path().join("a.png");
    let b_path = dir.path().join("b.png");
    std::fs::write(&a_path, std::fs::read(source.join("2414-base.png")).unwrap()).unwrap();
    std::fs::copy(&a_path, &b_path).unwrap();
    let a = DecodeRequest::new(&a_path);
    let b = DecodeRequest::new(&b_path);
    let mut local = policy();
    local.extract.max_features = 128;
    local.matching.max_comparisons = 128 * 128;
    local.matching.max_distance = 64;
    local.geometry.max_points = 128;
    local.geometry.max_hypotheses = 2048;
    local.geometry.min_inliers = 10;
    local.geometry.tolerance = 2.;
    local.pixels.tolerance = 0.03;
    local.pixels.max_source_pixels = 409_600;
    local.minimum_coverage_fraction = 0.3;
    local.minimum_matched_fraction = 0.9;
    let photometric = PhotometricPolicy {
        residual: local.pixels,
        minimum_samples: 16,
        minimum_variance: 1e-5,
        minimum_gain: 0.2,
        maximum_gain: 5.,
        maximum_offset: 0.1,
    };
    let filter = ColorFilterPolicy {
        filter: FilterPolicy {
            radius: 1,
            max_sample_pairs: 4_000_000,
        },
        color_space: FilterColorSpace::LinearSrgb,
    };
    let registration = ProjectiveRegistrationPolicy {
        radius: 1,
        stride: 8,
        rounds: 128,
        max_sample_pairs: 64_000_000,
    };
    let sampling = ProjectiveSamplingPolicy {
        trials: 2048,
        seed: 0x1234abcd,
    };
    let r = ProjectivePortfolioFilePolicy {
        local,
        filter,
        registration: ProjectiveRegistrationPortfolioPolicy {
            anchored: ProjectiveRegistrationTrustPolicy {
                registration,
                photometric,
                maximum_corner_shift: 1.,
            },
            unanchored: registration,
            max_sample_pairs: 128_000_000,
        },
        spatial: None,
        sampling: Some(sampling),
    };
    let mut multiscale = local;
    multiscale.matching.max_comparisons = 384 * 384;
    multiscale.geometry.max_points = 384;
    let p = ProjectivePyramidPhotometricFilePolicy {
        local: multiscale,
        max_levels: 3,
        max_total_pixels: 400_000,
        max_total_features: 384,
        sampling,
        photometric,
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                max_sample_pairs: 12_000_000,
                ..filter.filter
            },
            ..filter
        },
        fit_mode: PhotometricFitMode::ConstrainedLeastSquares,
    };
    let policy = ProjectiveComplementaryFilePolicy {
        registration: r,
        pyramid: p,
        max_total_comparisons: 128 * 128 + 384 * 384,
        max_total_hypotheses: 4096,
        max_total_sample_pairs: 144_000_000,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);

    use rrrah_dedup::{gradient::GradientMatchPolicy, local_scan::*};
    let q = ProjectiveComplementaryFilterPortfolioPolicy {
        searches: policy,
        secondary: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 3,
                max_sample_pairs: 32_000_000,
            },
            color_space: FilterColorSpace::LinearSrgb,
        },
        max_total_sample_pairs: 176_000_000,
    };
    let gradient = ProjectiveGradientPyramidFilePolicy {
        search: p,
        matching: GradientMatchPolicy {
            max_comparisons: 384 * 384,
            max_squared_distance: 0.5,
            squared_ratio: 0.64,
        },
        max_total_gradient_samples: 128 * 3 * 512,
    };
    let combined = ProjectiveComplementaryGradientPolicy {
        base: q,
        gradient,
        max_total_comparisons: policy.max_total_comparisons + 384 * 384,
        max_total_hypotheses: policy.max_total_hypotheses + p.sampling.trials,
        max_total_sample_pairs: q.max_total_sample_pairs + p.filter.filter.max_sample_pairs,
        max_total_gradient_samples: 2 * gradient.max_total_gradient_samples,
    };

    let five = ProjectiveComplementaryGradientPortfolioPolicy {
        primary: combined,
        interpolated: gradient,
        max_total_comparisons: combined.max_total_comparisons + gradient.matching.max_comparisons,
        max_total_hypotheses: combined.max_total_hypotheses + gradient.search.sampling.trials,
        max_total_sample_pairs: combined.max_total_sample_pairs
            + gradient.search.filter.filter.max_sample_pairs,
        max_total_gradient_samples: combined.max_total_gradient_samples
            + 2 * gradient.max_total_gradient_samples,
    };

    use rrrah_dedup::{
        local_collection::{
            ProjectiveFiveSearchCollectionPolicy, scan_projective_local_collection_five_searches,
        },
        local_index::FileFeatureBudgets,
    };
    let files = vec![
        (1, a.clone()),
        (2, b.clone()),
        (3, DecodeRequest::new(source.join("5495-base.png"))),
    ];
    let config = ProjectiveFiveSearchCollectionPolicy {
        search: five,
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 3840,
            max_hits: 3_000_000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
        max_gradient_retrieval_comparisons: 2_654_208,
    };
    let mut direct = std::collections::BTreeMap::new();
    for i in 0..files.len() {
        for j in i + 1..files.len() {
            let e = compare_local_files_projective_complementary_gradient_portfolio(
                &files[i].1,
                &files[j].1,
                five,
                &budget,
                || false,
            )
            .unwrap();
            direct.insert((files[i].0, files[j].0), (e.candidate, e.accepted_searches));
        }
    }
    assert_eq!(direct[&(1, 2)], (true, [true; 5]));
    let calls = Cell::new(0);
    let report = scan_projective_local_collection_five_searches(files.clone(), config, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let checkpoints = calls.get();
    assert_eq!(report.analysed, vec![1, 2, 3]);
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty()
    );
    assert_eq!(
        report
            .local
            .pairs
            .iter()
            .filter(|p| p.evidence.candidate)
            .map(|p| (p.left, p.right))
            .collect::<Vec<_>>(),
        vec![(1, 2)]
    );
    for pair in report.local.pairs {
        assert_eq!(
            (pair.evidence.candidate, pair.evidence.accepted_searches),
            direct[&(pair.left, pair.right)]
        );
    }
    assert_eq!(budget.used(), 0);
    for stop in [1, checkpoints / 2, checkpoints] {
        calls.set(0);
        assert!(matches!(
            scan_projective_local_collection_five_searches(files.clone(), config, &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(rrrah_dedup::scan::ScanError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let changed = Cell::new(false);
    calls.set(0);
    let mutated = scan_projective_local_collection_five_searches(files.clone(), config, &budget, || {
        calls.set(calls.get() + 1);
        if calls.get() == checkpoints / 2 {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&b_path)
                .unwrap()
                .write_all(&[0])
                .unwrap();
            changed.set(true);
        }
        false
    })
    .unwrap();
    assert!(changed.get());
    assert!(mutated.local.source_issues.iter().any(|(id, _)| *id == 2));
    assert!(mutated.local.pairs.iter().all(|p| p.left != 2 && p.right != 2));
    assert_eq!(budget.used(), 0);
    std::fs::copy(&a_path, &b_path).unwrap();
    let mut reversed = files.clone();
    reversed.reverse();
    let retry = scan_projective_local_collection_five_searches(reversed, config, &budget, || false).unwrap();
    assert_eq!(
        retry
            .local
            .pairs
            .iter()
            .filter(|p| p.evidence.candidate)
            .map(|p| (p.left, p.right))
            .collect::<Vec<_>>(),
        vec![(1, 2)]
    );
    assert_eq!(budget.used(), 0);
    use rrrah_dedup::local_collection::{
        ProjectiveFiveRegionCollectionPolicy, scan_projective_local_collection_five_regions,
    };
    let mut region_search = five.primary.base.searches.pyramid;
    region_search.filter = five.primary.base.secondary;
    let regional_work = 5 * region_search.filter.filter.max_sample_pairs;
    let joined = ProjectiveFiveRegionCollectionPolicy {
        five: config,
        grid: (2, 2),
        max_regions: 4,
        max_total_comparisons: five.max_total_comparisons + region_search.local.matching.max_comparisons,
        max_total_hypotheses: five.max_total_hypotheses + region_search.sampling.trials,
        max_total_sample_pairs: five.max_total_sample_pairs + regional_work,
    };
    let combined =
        scan_projective_local_collection_five_regions(files.clone(), joined, &budget, || false).unwrap();
    assert!(
        combined.file_issues.is_empty()
            && combined.local.issues.is_empty()
            && combined.local.source_issues.is_empty()
    );
    for pair in &combined.local.pairs {
        assert_eq!(
            (
                pair.evidence.whole.candidate,
                pair.evidence.whole.accepted_searches
            ),
            direct[&(pair.left, pair.right)]
        );
        let expected = rrrah_dedup::local_scan::compare_local_files_projective_pyramid_region_grid(
            &files[(pair.left - 1) as usize].1,
            &files[(pair.right - 1) as usize].1,
            rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {
                search: region_search,
                max_regions: 4,
                max_total_sample_pairs: regional_work,
            },
            (2, 2),
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(
            format!("{:?}", pair.evidence.regional.regions),
            format!("{:?}", expected.regions)
        );
        assert_eq!(pair.evidence.regional.whole.candidate, expected.whole.candidate);
    }
    assert!(
        combined
            .local
            .pairs
            .iter()
            .any(|pair| pair.left == 1 && pair.right == 2 && pair.evidence.whole.candidate)
    );
    drop(combined);
    assert_eq!(budget.used(), 0);
    let mut short = joined;
    short.max_total_sample_pairs -= 1;
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    assert!(matches!(
        scan_projective_local_collection_five_regions(
            [(1, DecodeRequest::new("missing-five-region.png"))],
            short,
            &fresh,
            || false
        ),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert_eq!(fresh.peak(), 0);

    use rrrah_dedup::local_collection::scan_projective_local_collection_five_all_regions;
    let mut all = joined;
    all.max_total_sample_pairs += 5 * five.primary.gradient.search.filter.filter.max_sample_pairs
        + 5 * five.interpolated.search.filter.filter.max_sample_pairs;
    let callbacks = Cell::new(0usize);
    let result = scan_projective_local_collection_five_all_regions(files[..2].to_vec(), all, &budget, || {
        callbacks.set(callbacks.get() + 1);
        false
    })
    .unwrap();
    assert!(result.local.issues.is_empty() && result.local.source_issues.is_empty());
    assert_eq!(result.local.pairs.len(), 1);
    let evidence = &result.local.pairs[0].evidence;
    assert_eq!(
        (
            evidence.base.whole.candidate,
            evidence.base.whole.accepted_searches
        ),
        direct[&(1, 2)]
    );
    let lanes = [
        &evidence.base.whole.primary.gradient,
        &evidence.base.whole.interpolated,
    ];
    for (index, lane) in lanes.into_iter().enumerate() {
        let search = if index == 0 {
            five.primary.gradient.search
        } else {
            five.interpolated.search
        };
        let model = lane
            .registered_transform
            .or_else(|| lane.geometry.as_ref().map(|e| e.transform))
            .unwrap();
        let expected = rrrah_dedup::local_scan::compare_local_files_projective_region_grid_transform(
            &files[0].1,
            &files[1].1,
            model,
            rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {
                search,
                max_regions: 4,
                max_total_sample_pairs: 5 * search.filter.filter.max_sample_pairs,
            },
            (2, 2),
            &budget,
            || false,
        )
        .unwrap();
        let actual = evidence.gradient_regions[index].as_ref().unwrap();
        assert_eq!(actual.transform, model);
        assert_eq!(actual.region_support_count(), 4);
        assert_eq!(format!("{:?}", actual.regions), format!("{:?}", expected.regions));
    }
    let final_checkpoint = callbacks.get();
    drop(result);
    assert_eq!(budget.used(), 0);
    for checkpoint in [1, final_checkpoint] {
        let calls = Cell::new(0usize);
        assert!(matches!(
            scan_projective_local_collection_five_all_regions(files[..2].to_vec(), all, &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == checkpoint
            }),
            Err(rrrah_dedup::scan::ScanError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }

    use rrrah_dedup::local_collection::scan_projective_local_collection_five_bidirectional_gradient_regions;
    let mut symmetric = all;
    symmetric.max_regions = 8;
    symmetric.max_total_sample_pairs += 4 * five.primary.gradient.search.filter.filter.max_sample_pairs
        + 4 * five.interpolated.search.filter.filter.max_sample_pairs;
    callbacks.set(0);
    let result = scan_projective_local_collection_five_bidirectional_gradient_regions(
        files[..2].to_vec(),
        symmetric,
        &budget,
        || {
            callbacks.set(callbacks.get() + 1);
            false
        },
    )
    .unwrap();
    assert!(result.local.issues.is_empty() && result.local.source_issues.is_empty());
    assert_eq!(result.local.pairs.len(), 1);
    let evidence = &result.local.pairs[0].evidence;
    assert_eq!(
        (
            evidence.base.whole.candidate,
            evidence.base.whole.accepted_searches
        ),
        direct[&(1, 2)]
    );
    let lanes = [
        &evidence.base.whole.primary.gradient,
        &evidence.base.whole.interpolated,
    ];
    for (index, lane) in lanes.into_iter().enumerate() {
        let search = if index == 0 {
            five.primary.gradient.search
        } else {
            five.interpolated.search
        };
        let model = lane
            .registered_transform
            .or_else(|| lane.geometry.as_ref().map(|e| e.transform))
            .unwrap();
        let expected =
            rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
                &files[0].1,
                &files[1].1,
                model,
                rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {
                    search,
                    max_regions: 8,
                    max_total_sample_pairs: 9 * search.filter.filter.max_sample_pairs,
                },
                (2, 2),
                &budget,
                || false,
            )
            .unwrap();
        let actual = evidence.gradient_regions[index].as_ref().unwrap();
        assert_eq!(actual.region_support_count(), 8);
        assert_eq!(format!("{:?}", actual.regions), format!("{:?}", expected.regions));
    }
    let end = callbacks.get();
    drop(result);
    assert_eq!(budget.used(), 0);
    for checkpoint in [1, end / 2, end] {
        let calls = Cell::new(0usize);
        assert!(matches!(
            scan_projective_local_collection_five_bidirectional_gradient_regions(
                files[..2].to_vec(),
                symmetric,
                &budget,
                || {
                    calls.set(calls.get() + 1);
                    calls.get() == checkpoint
                }
            ),
            Err(rrrah_dedup::scan::ScanError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }

    use rrrah_dedup::local_collection::{
        ProjectiveSixSearchCollectionPolicy, scan_projective_local_collection_six_searches,
    };
    let spatial = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 16,
    };
    let mut six_base = config;
    six_base.budgets.max_features = 10_000;
    six_base.max_gradient_retrieval_comparisons = 10_000_000;
    let six = ProjectiveSixSearchCollectionPolicy {
        five: six_base,
        spatial,
        max_total_comparisons: five.max_total_comparisons + five.interpolated.matching.max_comparisons,
        max_total_hypotheses: five.max_total_hypotheses + five.interpolated.search.sampling.trials,
        max_total_sample_pairs: five.max_total_sample_pairs
            + five.interpolated.search.filter.filter.max_sample_pairs,
    };
    let callbacks = Cell::new(0usize);
    let joined = scan_projective_local_collection_six_searches(files[..2].to_vec(), six, &budget, || {
        callbacks.set(callbacks.get() + 1);
        false
    })
    .unwrap();
    assert!(joined.local.issues.is_empty() && joined.local.source_issues.is_empty());
    assert_eq!(joined.local.pairs.len(), 1);
    let evidence = &joined.local.pairs[0].evidence;
    assert!(evidence.base.candidate && evidence.spatial.candidate);
    let ordinary = compare_local_files_projective_complementary_gradient_portfolio(
        &files[0].1,
        &files[1].1,
        five,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(format!("{:?}", evidence.base), format!("{:?}", ordinary));
    drop(ordinary);
    let expected =
        rrrah_dedup::local_scan::compare_local_files_projective_spatial_gradient_pyramid_with_recipe(
            &files[0].1,
            &files[1].1,
            five.interpolated,
            rrrah_dedup::gradient::GradientCellRecipe::Interpolated,
            spatial,
            &budget,
            || false,
        )
        .unwrap();
    assert_eq!(format!("{:?}", evidence.spatial), format!("{:?}", expected));
    drop(expected);
    let end = callbacks.get();
    drop(joined);
    assert_eq!(budget.used(), 0);
    for stop in [1, end / 2, end] {
        let count = Cell::new(0usize);
        assert!(matches!(
            scan_projective_local_collection_six_searches(files[..2].to_vec(), six, &budget, || {
                count.set(count.get() + 1);
                count.get() == stop
            }),
            Err(rrrah_dedup::scan::ScanError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    callbacks.set(0);
    let changed = Cell::new(false);
    let mutated = scan_projective_local_collection_six_searches(files[..2].to_vec(), six, &budget, || {
        callbacks.set(callbacks.get() + 1);
        if callbacks.get() == end / 2 {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&b_path)
                .unwrap()
                .write_all(&[0])
                .unwrap();
            changed.set(true);
        }
        false
    })
    .unwrap();
    assert!(changed.get());
    assert!(mutated.local.source_issues.iter().any(|(id, _)| *id == 2));
    assert!(mutated.local.pairs.is_empty());
    drop(mutated);
    assert_eq!(budget.used(), 0);
    std::fs::copy(&a_path, &b_path).unwrap();
    let mut reversed = files[..2].to_vec();
    reversed.reverse();
    let retry = scan_projective_local_collection_six_searches(reversed, six, &budget, || false).unwrap();
    assert_eq!(retry.local.pairs.len(), 1);
    assert!(retry.local.pairs[0].evidence.base.candidate && retry.local.pairs[0].evidence.spatial.candidate);
    drop(retry);
    assert_eq!(budget.used(), 0);
    for short in [
        ProjectiveSixSearchCollectionPolicy {
            max_total_comparisons: six.max_total_comparisons - 1,
            ..six
        },
        ProjectiveSixSearchCollectionPolicy {
            max_total_hypotheses: six.max_total_hypotheses - 1,
            ..six
        },
        ProjectiveSixSearchCollectionPolicy {
            max_total_sample_pairs: six.max_total_sample_pairs - 1,
            ..six
        },
    ] {
        let fresh = MemoryBudget::new(64 * 1024 * 1024);
        assert!(matches!(
            scan_projective_local_collection_six_searches(
                [(1, DecodeRequest::new("missing-six.png"))],
                short,
                &fresh,
                || false
            ),
            Err(rrrah_dedup::scan::ScanError::Budget)
        ));
        assert_eq!(fresh.peak(), 0);
    }

    use rrrah_dedup::local_collection::{
        ProjectiveSixRegionCollectionPolicy, scan_projective_local_collection_six_regions,
    };
    let mut binary_search = five.primary.base.searches.pyramid;
    binary_search.filter = five.primary.base.secondary;
    let regional_six = ProjectiveSixRegionCollectionPolicy {
        six,
        grid: (2, 2),
        max_regions: 8,
        max_total_comparisons: six.max_total_comparisons + binary_search.local.matching.max_comparisons,
        max_total_hypotheses: six.max_total_hypotheses + binary_search.sampling.trials,
        max_total_sample_pairs: six.max_total_sample_pairs
            + 5 * binary_search.filter.filter.max_sample_pairs
            + 9 * five.primary.gradient.search.filter.filter.max_sample_pairs
            + 18 * five.interpolated.search.filter.filter.max_sample_pairs,
    };
    let calls = Cell::new(0usize);
    let result =
        scan_projective_local_collection_six_regions(files[..2].to_vec(), regional_six, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    assert!(result.local.issues.is_empty() && result.local.source_issues.is_empty());
    assert_eq!(result.local.pairs.len(), 1);
    let evidence = &result.local.pairs[0].evidence;
    assert!(evidence.whole.base.candidate && evidence.whole.spatial.candidate);
    assert_eq!(evidence.binary_regions.region_support_count(), 4);
    for (index, lane) in [
        &evidence.whole.base.primary.gradient,
        &evidence.whole.base.interpolated,
        &evidence.whole.spatial,
    ]
    .into_iter()
    .enumerate()
    {
        let search = if index == 0 {
            five.primary.gradient.search
        } else {
            five.interpolated.search
        };
        let model = lane
            .registered_transform
            .or_else(|| lane.geometry.as_ref().map(|e| e.transform))
            .unwrap();
        let expected =
            rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
                &files[0].1,
                &files[1].1,
                model,
                rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {
                    search,
                    max_regions: 8,
                    max_total_sample_pairs: 9 * search.filter.filter.max_sample_pairs,
                },
                (2, 2),
                &budget,
                || false,
            )
            .unwrap();
        let actual = evidence.gradient_regions[index].as_ref().unwrap();
        assert_eq!(actual.region_support_count(), 8);
        assert_eq!(format!("{:?}", actual.regions), format!("{:?}", expected.regions));
    }
    let total = calls.get();
    drop(result);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        let n = Cell::new(0usize);
        assert!(matches!(
            scan_projective_local_collection_six_regions(files[..2].to_vec(), regional_six, &budget, || {
                n.set(n.get() + 1);
                n.get() == stop
            }),
            Err(rrrah_dedup::scan::ScanError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    // The regional run has the same whole-search prefix, followed by substantial
    // regional work. Mutate after that prefix and before its final batch guard.
    assert!(total > end + 100);
    let regional_checkpoint = end + (total - end) / 2;
    let regional_calls = Cell::new(0usize);
    let regional_changed = Cell::new(false);
    let mutated =
        scan_projective_local_collection_six_regions(files[..2].to_vec(), regional_six, &budget, || {
            regional_calls.set(regional_calls.get() + 1);
            if regional_calls.get() == regional_checkpoint {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&b_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                regional_changed.set(true);
            }
            false
        })
        .unwrap();
    assert!(regional_changed.get());
    assert!(mutated.local.source_issues.iter().any(|(id, _)| *id == 2));
    assert!(mutated.local.pairs.is_empty());
    drop(mutated);
    assert_eq!(budget.used(), 0);
    std::fs::copy(&a_path, &b_path).unwrap();
    let mut reversed = files[..2].to_vec();
    reversed.reverse();
    let retry =
        scan_projective_local_collection_six_regions(reversed, regional_six, &budget, || false).unwrap();
    assert!(retry.local.issues.is_empty() && retry.local.source_issues.is_empty());
    assert_eq!(retry.local.pairs.len(), 1);
    let evidence = &retry.local.pairs[0].evidence;
    assert!(evidence.whole.base.candidate && evidence.whole.spatial.candidate);
    assert_eq!(evidence.binary_regions.region_support_count(), 4);
    for lane in &evidence.gradient_regions {
        assert_eq!(lane.as_ref().unwrap().region_support_count(), 8);
    }
    drop(retry);
    assert_eq!(budget.used(), 0);
    for short in [
        ProjectiveSixRegionCollectionPolicy {
            max_regions: 7,
            ..regional_six
        },
        ProjectiveSixRegionCollectionPolicy {
            max_total_comparisons: regional_six.max_total_comparisons - 1,
            ..regional_six
        },
        ProjectiveSixRegionCollectionPolicy {
            max_total_hypotheses: regional_six.max_total_hypotheses - 1,
            ..regional_six
        },
        ProjectiveSixRegionCollectionPolicy {
            max_total_sample_pairs: regional_six.max_total_sample_pairs - 1,
            ..regional_six
        },
    ] {
        let fresh = MemoryBudget::new(64 * 1024 * 1024);
        assert!(matches!(
            scan_projective_local_collection_six_regions(
                [(1, DecodeRequest::new("missing-six-regional.png"))],
                short,
                &fresh,
                || false
            ),
            Err(rrrah_dedup::scan::ScanError::Budget)
        ));
        assert_eq!(fresh.peak(), 0);
    }
}
