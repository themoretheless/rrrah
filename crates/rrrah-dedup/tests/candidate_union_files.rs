#![cfg(feature = "decode")]
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    gradient::{GradientCellRecipe, GradientMatchPolicy},
    local::{LocalPolicy, MatchPolicy},
    local_scan::{
        LocalFilePolicy, ProjectiveGradientPyramidFilePolicy, ProjectivePyramidPhotometricFilePolicy,
    },
    warp::{
        ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy, WarpPolicy,
    },
};
fn policy() -> ProjectiveGradientPyramidFilePolicy {
    let warp = WarpPolicy {
        tolerance: 0.03,
        max_source_pixels: 409600,
    };
    ProjectiveGradientPyramidFilePolicy {
        search: ProjectivePyramidPhotometricFilePolicy {
            local: LocalFilePolicy {
                decode: AnimationBudget {
                    max_frames: 1,
                    max_pixels: 200000,
                    max_file_bytes: 1048576,
                },
                extract: LocalPolicy {
                    max_pixels: 200000,
                    max_candidates: 200000,
                    max_features: 500,
                    minimum_corner_score: 0.0001,
                },
                matching: MatchPolicy {
                    max_comparisons: 4000000,
                    max_distance: 64,
                },
                geometry: GeometryPolicy {
                    tolerance: 2.,
                    min_inliers: 10,
                    max_points: 2000,
                    max_hypotheses: 4096,
                },
                pixels: warp,
                minimum_compared_pixels: 1000,
                minimum_coverage_fraction: 0.3,
                minimum_matched_fraction: 0.9,
            },
            max_levels: 4,
            max_total_pixels: 400000,
            max_total_features: 2000,
            sampling: ProjectiveSamplingPolicy {
                trials: 4096,
                seed: 0x1234abcd,
            },
            photometric: PhotometricPolicy {
                residual: warp,
                minimum_samples: 16,
                minimum_variance: 1e-5,
                minimum_gain: 0.2,
                maximum_gain: 5.,
                maximum_offset: 0.1,
            },
            filter: ColorFilterPolicy {
                filter: FilterPolicy {
                    radius: 3,
                    max_sample_pairs: 32000000,
                },
                color_space: FilterColorSpace::LinearSrgb,
            },
            fit_mode: PhotometricFitMode::ConstrainedLeastSquares,
        },
        matching: GradientMatchPolicy {
            max_comparisons: 4000000,
            max_squared_distance: 0.5,
            squared_ratio: 0.64,
        },
        max_total_gradient_samples: 1024000,
    }
}
use rrrah_dedup::local_scan::{
    LocalFileError, ProjectiveCandidateUnionRegionsFilePolicy, ProjectiveDistinctScaleRegionsFilePolicy,
    SpatialFeaturePolicy, compare_local_files_projective_candidate_union_regions_managed as compare,
};
fn config() -> ProjectiveCandidateUnionRegionsFilePolicy {
    let mut gradient = policy();
    gradient.matching.max_comparisons = 8000000;
    ProjectiveCandidateUnionRegionsFilePolicy {
        search: ProjectiveDistinctScaleRegionsFilePolicy {
            gradient,
            recipe: GradientCellRecipe::Interpolated,
            spatial: SpatialFeaturePolicy {
                columns: 4,
                rows: 4,
                max_per_cell: 32,
            },
            competitor_radius: 2.,
            grid: (2, 2),
            max_regions: 8,
            max_total_sample_pairs: 320000000,
        },
        low_contrast_corner_score: 0.000001,
        smoothing_radius: 2,
        max_smoothing_taps_per_image: 2000000,
        max_area_taps_per_extraction: 4000000,
        max_total_gradient_samples: 4096000,
        max_total_match_comparisons: 16000000,
        max_union_points: 4000,
        max_union_comparisons: 8000000,
    }
}
fn factors() -> [f64; 4] {
    [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2]
}
#[test]
fn asymmetric_smoothing_late_cancellation_and_source_replacement_are_atomic() {
    use rrrah_dedup::local_scan::compare_local_files_projective_candidate_union_regions_with_smoothing_managed as asymmetric;
    use std::io::Write;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.png");
    let right = dir.path().join("right.png");
    std::fs::copy(root.join("base.png"), &left).unwrap();
    let bytes = std::fs::read(root.join("scale-117-angle-0.png")).unwrap();
    std::fs::write(&right, &bytes).unwrap();
    let a = rrrah_decode::DecodeRequest::new(&left);
    let b = rrrah_decode::DecodeRequest::new(&right);
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    drop(
        asymmetric(&a, &b, config(), &factors(), [4, 0], &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    let total = calls.get();
    assert!(total > 20);
    assert_eq!(budget.used(), 0);
    for stop in [total / 2, total] {
        calls.set(0);
        assert!(matches!(
            asymmetric(&a, &b, config(), &factors(), [4, 0], &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(LocalFileError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    calls.set(0);
    let changed = std::cell::Cell::new(false);
    let result = asymmetric(&a, &b, config(), &factors(), [4, 0], &budget, || {
        calls.set(calls.get() + 1);
        if calls.get() == total - 10 {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&right)
                .unwrap()
                .write_all(&[0])
                .unwrap();
            changed.set(true);
        }
        false
    });
    assert!(changed.get());
    assert!(matches!(result, Err(LocalFileError::Source(_))), "{result:?}");
    assert_eq!(budget.used(), 0);
    std::fs::write(&right, &bytes).unwrap();
    let restored = asymmetric(&b, &a, config(), &factors(), [0, 4], &budget, || false).unwrap();
    assert!(restored.regions.as_ref().unwrap().region_support_count() > 0);
    drop(restored);
    assert_eq!(budget.used(), 0);
}
#[test]
fn asymmetric_smoothing_keeps_original_pixel_confirmation_and_terminal_release() {
    use rrrah_dedup::local_scan::compare_local_files_projective_candidate_union_regions_with_smoothing_managed as asymmetric;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let a = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let b = rrrah_decode::DecodeRequest::new(root.join("scale-117-angle-0.png"));
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let e = asymmetric(&a, &b, config(), &factors(), [4, 0], &budget, || false).unwrap();
    let g = e
        .geometry
        .as_ref()
        .expect("unmodified low-contrast recipe keeps this positive");
    let direct = rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
        &a,
        &b,
        g.transform,
        rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {
            search: config().search.gradient.search,
            max_regions: 8,
            max_total_sample_pairs: 288000000,
        },
        (2, 2),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(
        format!("{:?}", e.regions.as_ref().unwrap()),
        format!("{direct:?}")
    );
    assert_eq!(direct.region_support_count(), 8);
    drop((direct, e));
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        asymmetric(&a, &b, config(), &factors(), [4, 0], &budget, || true),
        Err(LocalFileError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    let mut denied = config();
    denied.max_smoothing_taps_per_image = 1;
    assert!(asymmetric(&a, &b, denied, &factors(), [4, 0], &budget, || false).is_err());
    assert_eq!(budget.used(), 0);
    let other = rrrah_decode::DecodeRequest::new(root.join("unrelated.png"));
    let negative = asymmetric(&a, &other, config(), &factors(), [4, 0], &budget, || false).unwrap();
    assert_eq!(
        negative.regions.as_ref().map_or(0, |r| r.region_support_count()),
        0
    );
    drop(negative);
    assert_eq!(budget.used(), 0);
}
#[test]
fn fresh_union_regions_match_supplied_model_and_refuse_unrelated() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let a = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    for (name, positive) in [("scale-117-angle-0.png", true), ("unrelated.png", false)] {
        let b = rrrah_decode::DecodeRequest::new(root.join(name));
        let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let e = compare(&a, &b, config(), &factors(), &budget, || false).unwrap();
        let supports = e.regions.as_ref().map_or(0, |r| r.region_support_count());
        assert_eq!(supports > 0, positive);
        if positive {
            assert_eq!(supports, 8);
            let g = e.geometry.as_ref().unwrap();
            let direct =
                rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
                    &a,
                    &b,
                    g.transform,
                    rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {
                        search: config().search.gradient.search,
                        max_regions: 8,
                        max_total_sample_pairs: 288000000,
                    },
                    (2, 2),
                    &budget,
                    || false,
                )
                .unwrap();
            assert_eq!(
                format!("{:?}", e.regions.as_ref().unwrap()),
                format!("{direct:?}")
            );
            drop(direct);
            let points = e.correspondences.clone();
            let inliers = g.inliers.clone();
            let regions = e.regions.as_ref().unwrap().regions.clone();
            drop(e);
            assert!(budget.used() > 0);
            drop(points);
            assert!(budget.used() > 0);
            drop(inliers);
            assert!(budget.used() > 0);
            drop(regions);
        } else {
            drop(e);
        }
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn common_source_guard_cancellation_and_preio_aggregate_admission() {
    use std::io::Write;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.png");
    let right = dir.path().join("right.png");
    let bytes = std::fs::read(root.join("scale-117-angle-0.png")).unwrap();
    std::fs::copy(root.join("base.png"), &left).unwrap();
    std::fs::write(&right, &bytes).unwrap();
    let a = rrrah_decode::DecodeRequest::new(&left);
    let b = rrrah_decode::DecodeRequest::new(&right);
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    drop(
        compare(&a, &b, config(), &factors(), &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    let total = calls.get();
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(matches!(
            compare(&a, &b, config(), &factors(), &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(LocalFileError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    for stop in [total / 2, total - 10] {
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let result = compare(&a, &b, config(), &factors(), &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&right)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                changed.set(true);
            }
            false
        });
        assert!(changed.get());
        assert!(matches!(result, Err(LocalFileError::Source(_))), "{result:?}");
        assert_eq!(budget.used(), 0);
        std::fs::write(&right, &bytes).unwrap();
    }
    let restored = compare(&b, &a, config(), &factors(), &budget, || false).unwrap();
    assert_eq!(restored.regions.as_ref().unwrap().region_support_count(), 8);
    drop(restored);
    assert_eq!(budget.used(), 0);
    let missing = rrrah_decode::DecodeRequest::new(dir.path().join("missing.png"));
    for field in 0..3 {
        let mut p = config();
        match field {
            0 => p.max_total_gradient_samples -= 1,
            1 => p.max_total_match_comparisons -= 1,
            _ => p.max_union_comparisons = 7997999,
        };
        let denied = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        assert!(matches!(
            compare(&missing, &missing, p, &factors(), &denied, || false),
            Err(LocalFileError::Features(rrrah_dedup::local::LocalError::Budget))
        ));
        assert_eq!(denied.peak(), 0);
    }
}
fn collection_config() -> rrrah_dedup::local_collection::ProjectiveCandidateUnionRegionsCollectionPolicy {
    rrrah_dedup::local_collection::ProjectiveCandidateUnionRegionsCollectionPolicy {
        search: config(),
        budgets: rrrah_dedup::local_index::FileFeatureBudgets {
            max_files: 3,
            max_features: 12000,
            max_hits: 144000000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
        max_retrieval_comparisons: 144000000,
    }
}
#[test]
fn union_collection_retains_every_fresh_positive_and_exact_pair_evidence() {
    use rrrah_dedup::local_collection::scan_projective_local_collection_candidate_union_regions_managed as scan;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let files = [
        (1, rrrah_decode::DecodeRequest::new(root.join("base.png"))),
        (
            2,
            rrrah_decode::DecodeRequest::new(root.join("scale-117-angle-0.png")),
        ),
        (3, rrrah_decode::DecodeRequest::new(root.join("unrelated.png"))),
    ];
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let report = scan(files.clone(), collection_config(), &factors(), &budget, || false).unwrap();
    assert_eq!(report.analysed, [1, 2, 3]);
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty()
    );
    for i in 0..3 {
        for j in i + 1..3 {
            let fresh = compare(&files[i].1, &files[j].1, config(), &factors(), &budget, || false).unwrap();
            let retrieved = report
                .local
                .pairs
                .iter()
                .find(|p| (p.left, p.right) == (files[i].0, files[j].0));
            if fresh
                .regions
                .as_ref()
                .is_some_and(|r| r.region_support_count() > 0)
            {
                assert!(retrieved.is_some());
                assert_eq!((i, j), (0, 1));
            }
            if let Some(pair) = retrieved {
                assert_eq!(format!("{:?}", pair.evidence), format!("{fresh:?}"));
            }
        }
    }
    drop(report);
    assert_eq!(budget.used(), 0);
    let mut denied = collection_config();
    denied.search.max_total_match_comparisons -= 1;
    let touched = std::cell::Cell::new(false);
    let iterator = std::iter::from_fn(|| {
        touched.set(true);
        Some(files[0].clone())
    });
    let b = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    assert!(matches!(
        scan(iterator, denied, &factors(), &b, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert!(!touched.get());
    assert_eq!(b.peak(), 0);
}
#[test]
fn union_collection_removes_all_changed_source_edges_and_restores_reversed_retry() {
    use rrrah_dedup::local_collection::scan_projective_local_collection_candidate_union_regions_managed as scan;
    use std::io::Write;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let bytes = std::fs::read(root.join("base.png")).unwrap();
    let files: Vec<_> = (1..=3)
        .map(|id| {
            let path = dir.path().join(format!("{id}.png"));
            std::fs::write(&path, &bytes).unwrap();
            (id, rrrah_decode::DecodeRequest::new(path))
        })
        .collect();
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let baseline = scan(files.clone(), collection_config(), &factors(), &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(baseline.local.pairs.len(), 3);
    assert!(
        baseline
            .local
            .pairs
            .iter()
            .all(|p| p.evidence.regions.as_ref().unwrap().region_support_count() == 8)
    );
    let total = calls.get();
    drop(baseline);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(matches!(
            scan(files.clone(), collection_config(), &factors(), &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(rrrah_dedup::scan::ScanError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    for stop in [total / 2, total - 10] {
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let r = scan(files.clone(), collection_config(), &factors(), &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&files[1].1.path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                changed.set(true);
            }
            false
        })
        .unwrap();
        assert!(changed.get());
        assert_eq!(
            r.local
                .pairs
                .iter()
                .map(|p| (p.left, p.right))
                .collect::<Vec<_>>(),
            [(1, 3)]
        );
        assert!(r.local.source_issues.iter().any(|p| p.0 == 2));
        drop(r);
        assert_eq!(budget.used(), 0);
        std::fs::write(&files[1].1.path, &bytes).unwrap();
    }
    let r = scan(
        files.into_iter().rev(),
        collection_config(),
        &factors(),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(r.local.pairs.len(), 3);
    assert!(
        r.local
            .pairs
            .iter()
            .all(|p| p.evidence.regions.as_ref().unwrap().region_support_count() == 8)
    );
    drop(r);
    assert_eq!(budget.used(), 0);
}

#[test]
fn fallback_preserves_symmetric_positive_and_runs_all_misses_with_atomic_refusal() {
    use rrrah_dedup::local_scan::{
        ProjectiveCandidateUnionFallbackFilePolicy,
        compare_local_files_projective_candidate_union_fallback_regions_managed as fallback,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let a = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let b = rrrah_decode::DecodeRequest::new(root.join("scale-117-angle-0.png"));
    let other = rrrah_decode::DecodeRequest::new(root.join("unrelated.png"));
    let search = config();
    let p = ProjectiveCandidateUnionFallbackFilePolicy {
        search,
        asymmetric_radius: 4,
        max_total_gradient_samples: search.max_total_gradient_samples * 3,
        max_total_match_comparisons: search.max_total_match_comparisons * 3,
        max_total_union_comparisons: search.max_union_comparisons * 3,
    };
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let baseline = compare(&a, &b, search, &factors(), &budget, || false).unwrap();
    let result = fallback(&a, &b, p, &factors(), &budget, || false).unwrap();
    assert_eq!(result.attempted_recipes, 1);
    assert_eq!(result.smoothing_radii, [search.smoothing_radius; 2]);
    assert_eq!(format!("{:?}", result.evidence), format!("{baseline:?}"));
    drop((result, baseline));
    assert_eq!(budget.used(), 0);
    let calls = std::cell::Cell::new(0);
    let result = fallback(&a, &other, p, &factors(), &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    assert_eq!(result.attempted_recipes, 3);
    assert_eq!(result.smoothing_radii, [0, 4]);
    assert_eq!(
        result
            .evidence
            .regions
            .as_ref()
            .map_or(0, |r| r.region_support_count()),
        0
    );
    drop(result);
    assert_eq!(budget.used(), 0);
    for stop in [total / 2, total] {
        calls.set(0);
        assert!(matches!(
            fallback(&a, &other, p, &factors(), &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(LocalFileError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let temporary = tempfile::tempdir().unwrap();
    let changed_path = temporary.path().join("other.png");
    let bytes = std::fs::read(&other.path).unwrap();
    std::fs::write(&changed_path, &bytes).unwrap();
    let changed_request = rrrah_decode::DecodeRequest::new(&changed_path);
    let changed = std::cell::Cell::new(false);
    calls.set(0);
    let refused = fallback(&a, &changed_request, p, &factors(), &budget, || {
        calls.set(calls.get() + 1);
        if calls.get() == total - 10 {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .append(true)
                .open(&changed_path)
                .unwrap()
                .write_all(&[0])
                .unwrap();
            changed.set(true);
        }
        false
    });
    assert!(changed.get());
    assert!(matches!(refused, Err(LocalFileError::Source(_))));
    assert_eq!(budget.used(), 0);
    std::fs::write(&changed_path, &bytes).unwrap();
    let restored = fallback(&a, &changed_request, p, &factors(), &budget, || false).unwrap();
    assert_eq!(restored.attempted_recipes, 3);
    drop(restored);
    assert_eq!(budget.used(), 0);
    let missing = rrrah_decode::DecodeRequest::new(root.join("missing.png"));
    for field in 0..3 {
        let mut denied = p;
        match field {
            0 => denied.max_total_gradient_samples -= 1,
            1 => denied.max_total_match_comparisons -= 1,
            _ => denied.max_total_union_comparisons -= 1,
        };
        let memory = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        assert!(matches!(
            fallback(&missing, &missing, denied, &factors(), &memory, || false),
            Err(LocalFileError::Features(rrrah_dedup::local::LocalError::Budget))
        ));
        assert_eq!(memory.peak(), 0);
    }
}
#[test]
fn fallback_collection_preserves_fresh_pair_evidence_and_preiteration_admission() {
    use rrrah_dedup::local_collection::{
        ProjectiveCandidateUnionFallbackCollectionPolicy,
        scan_projective_local_collection_candidate_union_fallback_regions_managed as scan,
    };
    use rrrah_dedup::local_scan::{
        ProjectiveCandidateUnionFallbackFilePolicy,
        compare_local_files_projective_candidate_union_fallback_regions_managed as fallback,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let files = [
        (1, rrrah_decode::DecodeRequest::new(root.join("base.png"))),
        (
            2,
            rrrah_decode::DecodeRequest::new(root.join("scale-117-angle-0.png")),
        ),
        (3, rrrah_decode::DecodeRequest::new(root.join("unrelated.png"))),
    ];
    let search = config();
    let p = ProjectiveCandidateUnionFallbackFilePolicy {
        search,
        asymmetric_radius: 4,
        max_total_gradient_samples: search.max_total_gradient_samples * 3,
        max_total_match_comparisons: search.max_total_match_comparisons * 3,
        max_total_union_comparisons: search.max_union_comparisons * 3,
    };
    let policy = ProjectiveCandidateUnionFallbackCollectionPolicy {
        search: p,
        budgets: rrrah_dedup::local_index::FileFeatureBudgets {
            max_files: 3,
            max_features: 24000,
            max_hits: 576000000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
        max_retrieval_comparisons: 576000000,
    };
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let result = scan(files.clone(), policy, &factors(), &budget, || false).unwrap();
    assert!(
        result.file_issues.is_empty()
            && result.local.issues.is_empty()
            && result.local.source_issues.is_empty()
    );
    for i in 0..3 {
        for j in i + 1..3 {
            let fresh = fallback(&files[i].1, &files[j].1, p, &factors(), &budget, || false).unwrap();
            let retrieved = result
                .local
                .pairs
                .iter()
                .find(|pair| (pair.left, pair.right) == (files[i].0, files[j].0));
            if fresh
                .evidence
                .regions
                .as_ref()
                .is_some_and(|r| r.region_support_count() > 0)
            {
                assert!(retrieved.is_some());
            }
            if let Some(pair) = retrieved {
                assert_eq!(format!("{:?}", pair.evidence), format!("{fresh:?}"));
            }
        }
    }
    drop(result);
    assert_eq!(budget.used(), 0);
    let mut denied = policy;
    denied.search.max_total_gradient_samples -= 1;
    let touched = std::cell::Cell::new(false);
    let iterator = std::iter::from_fn(|| {
        touched.set(true);
        Some(files[0].clone())
    });
    let memory = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    assert!(matches!(
        scan(iterator, denied, &factors(), &memory, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert!(!touched.get());
    assert_eq!(memory.peak(), 0);
}

#[test]
fn fallback_collection_cancellation_source_change_and_restored_retry() {
    use rrrah_dedup::local_collection::scan_projective_local_collection_candidate_union_fallback_regions_managed as scan;
    use std::io::Write;
    let search = config();
    let fallback = rrrah_dedup::local_scan::ProjectiveCandidateUnionFallbackFilePolicy {
        search,
        asymmetric_radius: 4,
        max_total_gradient_samples: search.max_total_gradient_samples * 3,
        max_total_match_comparisons: search.max_total_match_comparisons * 3,
        max_total_union_comparisons: search.max_union_comparisons * 3,
    };
    let mut limits = collection_config().budgets;
    limits.max_features = 24000;
    limits.max_hits = 576000000;
    let policy = rrrah_dedup::local_collection::ProjectiveCandidateUnionFallbackCollectionPolicy {
        search: fallback,
        budgets: limits,
        max_retrieval_comparisons: 576000000,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let bytes = std::fs::read(root.join("base.png")).unwrap();
    let files: Vec<_> = (1..=3)
        .map(|id| {
            let path = dir.path().join(format!("{id}.png"));
            std::fs::write(&path, &bytes).unwrap();
            (id, rrrah_decode::DecodeRequest::new(path))
        })
        .collect();
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let baseline = scan(files.clone(), policy, &factors(), &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(baseline.local.pairs.len(), 3);
    assert!(baseline.local.pairs.iter().all(|p| {
        p.evidence
            .evidence
            .regions
            .as_ref()
            .unwrap()
            .region_support_count()
            == 8
    }));
    let total = calls.get();
    drop(baseline);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(matches!(
            scan(files.clone(), policy, &factors(), &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(rrrah_dedup::scan::ScanError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    for stop in [total / 2, total - 10] {
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let r = scan(files.clone(), policy, &factors(), &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&files[1].1.path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                changed.set(true);
            }
            false
        })
        .unwrap();
        assert!(changed.get());
        assert_eq!(
            r.local
                .pairs
                .iter()
                .map(|p| (p.left, p.right))
                .collect::<Vec<_>>(),
            [(1, 3)]
        );
        assert!(r.local.source_issues.iter().any(|p| p.0 == 2));
        drop(r);
        assert_eq!(budget.used(), 0);
        std::fs::write(&files[1].1.path, &bytes).unwrap();
    }
    let r = scan(files.into_iter().rev(), policy, &factors(), &budget, || false).unwrap();
    assert_eq!(r.local.pairs.len(), 3);
    assert!(r.local.pairs.iter().all(|p| {
        p.evidence
            .evidence
            .regions
            .as_ref()
            .unwrap()
            .region_support_count()
            == 8
    }));
    drop(r);
    assert_eq!(budget.used(), 0);
}
