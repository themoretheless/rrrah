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
fn gradient_collection_matches_direct_pairs_and_atomic_admission() {
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
    use rrrah_dedup::{
        gradient::{GradientCellRecipe, GradientMatchPolicy},
        local_collection::{ProjectiveGradientCollectionPolicy, scan_projective_local_collection_gradient},
        local_index::FileFeatureBudgets,
        local_scan::ProjectiveGradientPyramidFilePolicy,
    };
    use std::cell::Cell;
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
    let sampling = ProjectiveSamplingPolicy {
        trials: 2048,
        seed: 0x1234abcd,
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
    let gradient = ProjectiveGradientPyramidFilePolicy {
        search: p,
        matching: GradientMatchPolicy {
            max_comparisons: 384 * 384,
            max_squared_distance: 0.5,
            squared_ratio: 0.64,
        },
        max_total_gradient_samples: 128 * 3 * 512,
    };

    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let unrelated = DecodeRequest::new(source.join("5495-base.png"));
    let files = vec![(1, a.clone()), (2, b.clone()), (3, unrelated)];
    for recipe in [GradientCellRecipe::Fixed, GradientCellRecipe::Interpolated] {
        let config = ProjectiveGradientCollectionPolicy {
            search: gradient,
            recipe,
            budgets: FileFeatureBudgets {
                max_files: 3,
                max_features: 1152,
                max_hits: 1_327_104,
                max_pair_counts: 3,
                max_pairs: 3,
            },
            max_retrieval_comparisons: 1_327_104,
        };
        use rrrah_dedup::{
            local_collection::scan_projective_local_collection_spatial_gradient,
            local_scan::{
                SpatialFeaturePolicy, compare_local_files_projective_spatial_gradient_pyramid_with_recipe,
            },
        };
        let spatial = SpatialFeaturePolicy {
            columns: 2,
            rows: 2,
            max_per_cell: 32,
        };
        let mut spatial_direct = std::collections::BTreeMap::new();
        for i in 0..files.len() {
            for j in i + 1..files.len() {
                let e = compare_local_files_projective_spatial_gradient_pyramid_with_recipe(
                    &files[i].1,
                    &files[j].1,
                    gradient,
                    recipe,
                    spatial,
                    &budget,
                    || false,
                )
                .unwrap();
                spatial_direct.insert((files[i].0, files[j].0), format!("{e:?}"));
            }
        }
        let spatial_calls = Cell::new(0usize);
        let spatial_report = scan_projective_local_collection_spatial_gradient(
            files.clone(),
            config,
            spatial,
            &budget,
            || {
                spatial_calls.set(spatial_calls.get() + 1);
                false
            },
        )
        .unwrap();
        assert!(
            spatial_report.file_issues.is_empty()
                && spatial_report.local.issues.is_empty()
                && spatial_report.local.source_issues.is_empty()
        );
        assert!(
            spatial_report
                .local
                .pairs
                .iter()
                .any(|pair| pair.left == 1 && pair.right == 2 && pair.evidence.candidate)
        );
        for pair in &spatial_report.local.pairs {
            assert_eq!(
                format!("{:?}", pair.evidence),
                spatial_direct[&(pair.left, pair.right)]
            );
        }
        let spatial_total = spatial_calls.get();
        drop(spatial_report);
        assert_eq!(budget.used(), 0);
        for stop in [1, spatial_total / 2, spatial_total] {
            spatial_calls.set(0);
            assert!(matches!(
                scan_projective_local_collection_spatial_gradient(
                    files.clone(),
                    config,
                    spatial,
                    &budget,
                    || {
                        spatial_calls.set(spatial_calls.get() + 1);
                        spatial_calls.get() == stop
                    }
                ),
                Err(rrrah_dedup::scan::ScanError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
        let empty_budget = MemoryBudget::new(64 * 1024 * 1024);
        assert!(matches!(
            scan_projective_local_collection_spatial_gradient(
                [(1, DecodeRequest::new("missing-spatial-gradient-collection.png"))],
                config,
                SpatialFeaturePolicy {
                    columns: 0,
                    ..spatial
                },
                &empty_budget,
                || false
            ),
            Err(rrrah_dedup::scan::ScanError::InvalidPolicy)
        ));
        assert_eq!(empty_budget.peak(), 0);
        use rrrah_dedup::local_collection::{
            ProjectiveSpatialGradientRegionCollectionPolicy,
            scan_projective_local_collection_spatial_gradient_regions,
        };
        let regions_config = ProjectiveSpatialGradientRegionCollectionPolicy {
            gradient: config,
            spatial,
            grid: (2, 2),
            max_regions: 8,
            max_total_sample_pairs: 10 * gradient.search.filter.filter.max_sample_pairs,
        };
        spatial_calls.set(0);
        let regional_report = scan_projective_local_collection_spatial_gradient_regions(
            files[..2].to_vec(),
            regions_config,
            &budget,
            || {
                spatial_calls.set(spatial_calls.get() + 1);
                false
            },
        )
        .unwrap();
        assert!(regional_report.local.issues.is_empty() && regional_report.local.source_issues.is_empty());
        assert_eq!(regional_report.local.pairs.len(), 1);
        let confirmed = &regional_report.local.pairs[0].evidence;
        assert!(confirmed.whole.candidate);
        assert_eq!(confirmed.regions.as_ref().unwrap().region_support_count(), 8);
        let total = spatial_calls.get();
        drop(regional_report);
        assert_eq!(budget.used(), 0);
        for stop in [1, total / 2, total] {
            spatial_calls.set(0);
            assert!(matches!(
                scan_projective_local_collection_spatial_gradient_regions(
                    files[..2].to_vec(),
                    regions_config,
                    &budget,
                    || {
                        spatial_calls.set(spatial_calls.get() + 1);
                        spatial_calls.get() == stop
                    }
                ),
                Err(rrrah_dedup::scan::ScanError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
        for short in [
            ProjectiveSpatialGradientRegionCollectionPolicy {
                max_regions: 7,
                ..regions_config
            },
            ProjectiveSpatialGradientRegionCollectionPolicy {
                max_total_sample_pairs: regions_config.max_total_sample_pairs - 1,
                ..regions_config
            },
        ] {
            let fresh = MemoryBudget::new(64 * 1024 * 1024);
            assert!(matches!(
                scan_projective_local_collection_spatial_gradient_regions(
                    [(1, DecodeRequest::new("missing-spatial-regional.png"))],
                    short,
                    &fresh,
                    || false
                ),
                Err(rrrah_dedup::scan::ScanError::Budget)
            ));
            assert_eq!(fresh.peak(), 0);
        }
        spatial_calls.set(0);
        let changed = Cell::new(false);
        let mutated = scan_projective_local_collection_spatial_gradient_regions(
            files[..2].to_vec(),
            regions_config,
            &budget,
            || {
                spatial_calls.set(spatial_calls.get() + 1);
                if spatial_calls.get() == total / 2 {
                    use std::io::Write;
                    std::fs::OpenOptions::new()
                        .append(true)
                        .open(&b_path)
                        .unwrap()
                        .write_all(&[0])
                        .unwrap();
                    changed.set(true);
                }
                false
            },
        )
        .unwrap();
        assert!(changed.get());
        assert!(mutated.local.source_issues.iter().any(|(id, _)| *id == 2));
        assert!(mutated.local.pairs.is_empty());
        drop(mutated);
        assert_eq!(budget.used(), 0);
        std::fs::copy(&a_path, &b_path).unwrap();
        let mut reversed = files[..2].to_vec();
        reversed.reverse();
        let retry = scan_projective_local_collection_spatial_gradient_regions(
            reversed,
            regions_config,
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(
            retry.local.pairs[0]
                .evidence
                .regions
                .as_ref()
                .unwrap()
                .region_support_count(),
            8
        );
        drop(retry);
        assert_eq!(budget.used(), 0);
        let mut invalid = config;
        invalid.search.matching.squared_ratio = 1.;
        let fresh = MemoryBudget::new(64 * 1024 * 1024);
        let missing = vec![(99, DecodeRequest::new(dir.path().join("missing.png")))];
        assert!(matches!(
            scan_projective_local_collection_gradient(missing, invalid, &fresh, || false),
            Err(rrrah_dedup::scan::ScanError::InvalidPolicy)
        ));
        assert_eq!(fresh.peak(), 0);
        let mut direct = std::collections::BTreeSet::new();
        for i in 0..files.len() {
            for j in i + 1..files.len() {
                let e = rrrah_dedup::local_scan::compare_local_files_projective_gradient_pyramid_with_recipe(
                    &files[i].1,
                    &files[j].1,
                    gradient,
                    recipe,
                    &budget,
                    || false,
                )
                .unwrap();
                if e.candidate {
                    direct.insert((files[i].0, files[j].0));
                }
            }
        }
        assert_eq!(direct, std::collections::BTreeSet::from([(1, 2)]));
        let calls = Cell::new(0);
        let report = scan_projective_local_collection_gradient(files.clone(), config, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        let checkpoints = calls.get();
        assert!(
            report.file_issues.is_empty()
                && report.local.issues.is_empty()
                && report.local.source_issues.is_empty()
        );
        assert_eq!(report.analysed, vec![1, 2, 3]);
        let found = report
            .local
            .pairs
            .iter()
            .filter(|p| p.evidence.candidate)
            .map(|p| (p.left, p.right))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(found, direct);
        assert!(report.descriptor_hits > 0);
        assert_eq!(budget.used(), 0);
        for stop in [1, checkpoints / 2, checkpoints] {
            calls.set(0);
            assert!(matches!(
                scan_projective_local_collection_gradient(files.clone(), config, &budget, || {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }),
                Err(rrrah_dedup::scan::ScanError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
        let mut short = config;
        short.max_retrieval_comparisons = 0;
        assert!(matches!(
            scan_projective_local_collection_gradient(files.clone(), short, &budget, || false),
            Err(rrrah_dedup::scan::ScanError::Budget)
        ));
        assert_eq!(budget.used(), 0);
        let changed = Cell::new(false);
        calls.set(0);
        let mutated = scan_projective_local_collection_gradient(files.clone(), config, &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == checkpoints / 2 {
                use std::io::Write;
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
        assert!(!mutated.analysed.contains(&2));
        assert!(
            mutated
                .local
                .pairs
                .iter()
                .all(|pair| pair.left != 2 && pair.right != 2)
        );
        assert_eq!(budget.used(), 0);
        std::fs::copy(&a_path, &b_path).unwrap();
        let mut reversed = files.clone();
        reversed.reverse();
        let retry = scan_projective_local_collection_gradient(reversed, config, &budget, || false).unwrap();
        assert_eq!(
            retry
                .local
                .pairs
                .iter()
                .filter(|p| p.evidence.candidate)
                .map(|p| (p.left, p.right))
                .collect::<std::collections::BTreeSet<_>>(),
            direct
        );
        assert_eq!(budget.used(), 0);
    }
}
