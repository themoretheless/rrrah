#![cfg(feature = "decode")]
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    gradient::{GradientCellRecipe, GradientMatchPolicy},
    local::{LocalPolicy, MatchPolicy},
    local_scan::{
        LocalFilePolicy, ProjectiveGradientPyramidFilePolicy, ProjectivePyramidPhotometricFilePolicy,
        compare_local_files_projective_gradient_scales_with_recipe,
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
#[test]
fn scaled_files_confirm_pixels_and_refuse_unrelated() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let a = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    for (name, positive) in [("scale-117-angle-0.png", true), ("unrelated.png", false)] {
        let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let b = rrrah_decode::DecodeRequest::new(root.join(name));
        let e = compare_local_files_projective_gradient_scales_with_recipe(
            &a,
            &b,
            policy(),
            GradientCellRecipe::Interpolated,
            &factors,
            &budget,
            || false,
        )
        .unwrap();
        println!("{name}: {e:?}");
        assert_eq!(e.candidate, positive);
        drop(e);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn scale_file_admission_and_cancellation_are_atomic() {
    use rrrah_dedup::local_scan::LocalFileError;
    let missing = rrrah_decode::DecodeRequest::new("/rrrah-missing-scale-fixture.png");
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    for factors in [&[1., 1.][..], &[f64::NAN][..], &[0.5][..]] {
        assert!(matches!(
            compare_local_files_projective_gradient_scales_with_recipe(
                &missing,
                &missing,
                policy(),
                GradientCellRecipe::Interpolated,
                factors,
                &budget,
                || false
            ),
            Err(LocalFileError::Features(rrrah_dedup::local::LocalError::Invalid))
        ));
        assert_eq!(budget.peak(), 0);
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let a = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let b = rrrah_decode::DecodeRequest::new(root.join("scale-117-angle-0.png"));
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let calls = std::cell::Cell::new(0usize);
    let baseline = compare_local_files_projective_gradient_scales_with_recipe(
        &a,
        &b,
        policy(),
        GradientCellRecipe::Interpolated,
        &factors,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert!(baseline.candidate);
    drop(baseline);
    assert_eq!(budget.used(), 0);
    let total = calls.get();
    for stop in [0, total / 2, total - 1] {
        calls.set(0);
        let result = compare_local_files_projective_gradient_scales_with_recipe(
            &a,
            &b,
            policy(),
            GradientCellRecipe::Interpolated,
            &factors,
            &budget,
            || {
                let n = calls.get();
                calls.set(n + 1);
                n == stop
            },
        );
        assert!(result.is_err());
        drop(result);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn changed_source_refuses_scale_evidence_and_restored_retry_succeeds() {
    use std::io::Write;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.png");
    let right = dir.path().join("right.png");
    let original = std::fs::read(root.join("scale-117-angle-0.png")).unwrap();
    std::fs::write(&left, std::fs::read(root.join("base.png")).unwrap()).unwrap();
    std::fs::write(&right, &original).unwrap();
    let a = rrrah_decode::DecodeRequest::new(&left);
    let b = rrrah_decode::DecodeRequest::new(&right);
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0usize);
    let baseline = compare_local_files_projective_gradient_scales_with_recipe(
        &a,
        &b,
        policy(),
        GradientCellRecipe::Interpolated,
        &factors,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert!(baseline.candidate);
    drop(baseline);
    assert_eq!(budget.used(), 0);
    let total = calls.get();
    for stop in [total / 2, total - 2] {
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let result = compare_local_files_projective_gradient_scales_with_recipe(
            &a,
            &b,
            policy(),
            GradientCellRecipe::Interpolated,
            &factors,
            &budget,
            || {
                let n = calls.get();
                calls.set(n + 1);
                if n == stop {
                    std::fs::OpenOptions::new()
                        .append(true)
                        .open(&right)
                        .unwrap()
                        .write_all(&[0])
                        .unwrap();
                    changed.set(true);
                }
                false
            },
        );
        assert!(changed.get());
        assert!(
            matches!(result, Err(rrrah_dedup::local_scan::LocalFileError::Source(_))),
            "{result:?}"
        );
        drop(result);
        assert_eq!(budget.used(), 0);
        std::fs::write(&right, &original).unwrap();
    }
    let retry = compare_local_files_projective_gradient_scales_with_recipe(
        &b,
        &a,
        policy(),
        GradientCellRecipe::Interpolated,
        &factors,
        &budget,
        || false,
    )
    .unwrap();
    assert!(retry.candidate);
    drop(retry);
    assert_eq!(budget.used(), 0);
}

#[test]
fn collection_scale_edges_equal_fresh_pair_confirmation() {
    use rrrah_dedup::{
        local_collection::{
            ProjectiveGradientCollectionPolicy, scan_projective_local_collection_gradient_scales,
        },
        local_index::FileFeatureBudgets,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let files: Vec<_> = ["base.png", "scale-117-angle-0.png", "unrelated.png"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| (i as u64 + 1, rrrah_decode::DecodeRequest::new(root.join(name))))
        .collect();
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let config = ProjectiveGradientCollectionPolicy {
        search: policy(),
        recipe: GradientCellRecipe::Interpolated,
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 6000,
            max_hits: 36000000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
        max_retrieval_comparisons: 36000000,
    };
    let report =
        scan_projective_local_collection_gradient_scales(files.clone(), config, &factors, &budget, || false)
            .unwrap();
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty()
    );
    assert!(
        report
            .local
            .pairs
            .iter()
            .any(|p| p.left == 1 && p.right == 2 && p.evidence.candidate)
    );
    assert!(
        report
            .local
            .pairs
            .iter()
            .filter(|p| p.right == 3)
            .all(|p| !p.evidence.candidate)
    );
    for pair in &report.local.pairs {
        let direct = compare_local_files_projective_gradient_scales_with_recipe(
            &files[(pair.left - 1) as usize].1,
            &files[(pair.right - 1) as usize].1,
            policy(),
            config.recipe,
            &factors,
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(format!("{:?}", pair.evidence), format!("{direct:?}"));
        drop(direct);
    }
    drop(report);
    assert_eq!(budget.used(), 0);
}

#[test]
fn collection_scale_mutation_and_cancellation_remove_partial_results() {
    use rrrah_dedup::{
        local_collection::{
            ProjectiveGradientCollectionPolicy, scan_projective_local_collection_gradient_scales,
        },
        local_index::FileFeatureBudgets,
    };
    use std::io::Write;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    let saved = std::fs::read(root.join("scale-117-angle-0.png")).unwrap();
    std::fs::write(&a, std::fs::read(root.join("base.png")).unwrap()).unwrap();
    std::fs::write(&b, &saved).unwrap();
    let files = vec![
        (1, rrrah_decode::DecodeRequest::new(&a)),
        (2, rrrah_decode::DecodeRequest::new(&b)),
    ];
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let config = ProjectiveGradientCollectionPolicy {
        search: policy(),
        recipe: GradientCellRecipe::Interpolated,
        budgets: FileFeatureBudgets {
            max_files: 2,
            max_features: 4000,
            max_hits: 16000000,
            max_pair_counts: 1,
            max_pairs: 1,
        },
        max_retrieval_comparisons: 16000000,
    };
    let calls = std::cell::Cell::new(0usize);
    let baseline =
        scan_projective_local_collection_gradient_scales(files.clone(), config, &factors, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    assert!(baseline.local.pairs[0].evidence.candidate);
    drop(baseline);
    assert_eq!(budget.used(), 0);
    let total = calls.get();
    for stop in [0, total / 2, total - 1] {
        calls.set(0);
        let result = scan_projective_local_collection_gradient_scales(
            files.clone(),
            config,
            &factors,
            &budget,
            || {
                let n = calls.get();
                calls.set(n + 1);
                n == stop
            },
        );
        assert!(result.is_err());
        drop(result);
        assert_eq!(budget.used(), 0);
    }
    for stop in [total / 2, total - 10] {
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let result = scan_projective_local_collection_gradient_scales(
            files.clone(),
            config,
            &factors,
            &budget,
            || {
                let n = calls.get();
                calls.set(n + 1);
                if n == stop {
                    std::fs::OpenOptions::new()
                        .append(true)
                        .open(&b)
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
        assert!(result.local.source_issues.iter().any(|(id, _)| *id == 2));
        assert!(result.local.pairs.is_empty());
        drop(result);
        assert_eq!(budget.used(), 0);
        std::fs::write(&b, &saved).unwrap();
    }
    let retry = scan_projective_local_collection_gradient_scales(
        files.into_iter().rev(),
        config,
        &factors,
        &budget,
        || false,
    )
    .unwrap();
    assert!(retry.local.pairs[0].evidence.candidate);
    drop(retry);
    assert_eq!(budget.used(), 0);
}

#[test]
fn intermediate_scale_files_retain_authored_rotations_and_enlargements() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let a = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    for name in [
        "base.png",
        "angle-17.png",
        "angle--37.png",
        "angle-63.png",
        "scale-216-angle-0.png",
        "scale-216-angle-17.png",
    ] {
        let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let b = rrrah_decode::DecodeRequest::new(root.join(name));
        let result = compare_local_files_projective_gradient_scales_with_recipe(
            &a,
            &b,
            policy(),
            GradientCellRecipe::Interpolated,
            &factors,
            &budget,
            || false,
        )
        .unwrap();
        assert!(result.candidate, "{name}: {result:?}");
        drop(result);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn scale_list_cancellation_precedes_file_and_iterator_access() {
    let missing = rrrah_decode::DecodeRequest::new("/rrrah-missing-scale-fixture.png");
    let factors = [1., std::f64::consts::SQRT_2, 2.];
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0usize);
    let result = compare_local_files_projective_gradient_scales_with_recipe(
        &missing,
        &missing,
        policy(),
        GradientCellRecipe::Interpolated,
        &factors,
        &budget,
        || {
            let n = calls.get();
            calls.set(n + 1);
            n == 1
        },
    );
    assert!(matches!(
        result,
        Err(rrrah_dedup::local_scan::LocalFileError::Cancelled)
    ));
    assert_eq!(budget.peak(), 0);
    let config = rrrah_dedup::local_collection::ProjectiveGradientCollectionPolicy {
        search: policy(),
        recipe: GradientCellRecipe::Interpolated,
        budgets: rrrah_dedup::local_index::FileFeatureBudgets {
            max_files: 2,
            max_features: 4000,
            max_hits: 16000000,
            max_pair_counts: 1,
            max_pairs: 1,
        },
        max_retrieval_comparisons: 16000000,
    };
    calls.set(0);
    let files = std::iter::from_fn(|| -> Option<(u64, rrrah_decode::DecodeRequest)> {
        panic!("iterator must not be accessed before scale validation")
    });
    let result = rrrah_dedup::local_collection::scan_projective_local_collection_gradient_scales(
        files,
        config,
        &factors,
        &budget,
        || {
            let n = calls.get();
            calls.set(n + 1);
            n == 1
        },
    );
    assert!(matches!(result, Err(rrrah_dedup::scan::ScanError::Cancelled)));
    assert_eq!(budget.peak(), 0);
}

#[test]
fn spatial_collection_scale_edges_equal_fresh_pair_confirmation() {
    use rrrah_dedup::{
        local_collection::{
            ProjectiveGradientCollectionPolicy, scan_projective_local_collection_spatial_gradient_scales,
        },
        local_index::FileFeatureBudgets,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let files: Vec<_> = ["base.png", "scale-117-angle-0.png", "unrelated.png"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| (i as u64 + 1, rrrah_decode::DecodeRequest::new(root.join(name))))
        .collect();
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let grid = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 32,
    };
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let config = ProjectiveGradientCollectionPolicy {
        search: policy(),
        recipe: GradientCellRecipe::Interpolated,
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 6000,
            max_hits: 36000000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
        max_retrieval_comparisons: 36000000,
    };
    let report = scan_projective_local_collection_spatial_gradient_scales(
        files.clone(),
        config,
        &factors,
        grid,
        &budget,
        || false,
    )
    .unwrap();
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty()
    );
    assert!(
        report
            .local
            .pairs
            .iter()
            .any(|p| p.left == 1 && p.right == 2 && p.evidence.candidate)
    );
    assert!(
        report
            .local
            .pairs
            .iter()
            .filter(|p| p.right == 3)
            .all(|p| !p.evidence.candidate)
    );
    for pair in &report.local.pairs {
        let direct =
            rrrah_dedup::local_scan::compare_local_files_projective_spatial_gradient_scales_with_recipe(
                &files[(pair.left - 1) as usize].1,
                &files[(pair.right - 1) as usize].1,
                policy(),
                config.recipe,
                &factors,
                grid,
                &budget,
                || false,
            )
            .unwrap();
        assert_eq!(format!("{:?}", pair.evidence), format!("{direct:?}"));
        drop(direct);
    }
    drop(report);
    assert_eq!(budget.used(), 0);
}

#[test]
fn spatial_scale_regions_match_fresh_supplied_model_confirmation() {
    use rrrah_dedup::{
        local_collection::{
            ProjectiveGradientCollectionPolicy, ProjectiveSpatialGradientRegionCollectionPolicy,
            scan_projective_local_collection_spatial_gradient_scale_regions,
        },
        local_index::FileFeatureBudgets,
        local_scan::{
            ProjectivePyramidRegionsFilePolicy,
            compare_local_files_projective_bidirectional_region_grid_transform,
        },
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let left_path = dir.path().join("left.png");
    let right_path = dir.path().join("right.png");
    let saved = std::fs::read(root.join("scale-117-angle-0.png")).unwrap();
    std::fs::write(&left_path, std::fs::read(root.join("base.png")).unwrap()).unwrap();
    std::fs::write(&right_path, &saved).unwrap();
    let a = rrrah_decode::DecodeRequest::new(&left_path);
    let b = rrrah_decode::DecodeRequest::new(&right_path);
    let spatial = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 32,
    };
    let gradient = ProjectiveGradientCollectionPolicy {
        search: policy(),
        recipe: GradientCellRecipe::Interpolated,
        budgets: FileFeatureBudgets {
            max_files: 2,
            max_features: 4000,
            max_hits: 16000000,
            max_pair_counts: 1,
            max_pairs: 1,
        },
        max_retrieval_comparisons: 16000000,
    };
    let config = ProjectiveSpatialGradientRegionCollectionPolicy {
        gradient,
        spatial,
        grid: (2, 2),
        max_regions: 8,
        max_total_sample_pairs: 320000000,
    };
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let calls = std::cell::Cell::new(0usize);
    let report = scan_projective_local_collection_spatial_gradient_scale_regions(
        [(1, a.clone()), (2, b.clone())],
        config,
        &factors,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty()
    );
    assert_eq!(report.local.pairs.len(), 1);
    let e = &report.local.pairs[0].evidence;
    assert!(e.whole.candidate);
    let regions = e.regions.as_ref().unwrap();
    assert_eq!(regions.region_support_count(), 8);
    let direct = compare_local_files_projective_bidirectional_region_grid_transform(
        &a,
        &b,
        e.whole.geometry.as_ref().unwrap().transform,
        ProjectivePyramidRegionsFilePolicy {
            search: policy().search,
            max_regions: 8,
            max_total_sample_pairs: 288000000,
        },
        (2, 2),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(format!("{regions:?}"), format!("{direct:?}"));
    drop(direct);
    drop(report);
    assert_eq!(budget.used(), 0);
    let total_calls = calls.get();
    assert!(total_calls > 3);
    for stop in [1, total_calls / 2, total_calls] {
        let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        calls.set(0);
        let result = scan_projective_local_collection_spatial_gradient_scale_regions(
            [(1, a.clone()), (2, b.clone())],
            config,
            &factors,
            &budget,
            || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        );
        assert!(
            matches!(result, Err(rrrah_dedup::scan::ScanError::Cancelled)),
            "stop={stop}: {result:?}"
        );
        assert_eq!(budget.used(), 0, "stop={stop}");
    }
    for stop in [total_calls / 2, total_calls - 10] {
        use std::io::Write;
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let result = scan_projective_local_collection_spatial_gradient_scale_regions(
            [(1, a.clone()), (2, b.clone())],
            config,
            &factors,
            &budget,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    std::fs::OpenOptions::new()
                        .append(true)
                        .open(&right_path)
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
        assert!(result.local.source_issues.iter().any(|(id, _)| *id == 2));
        assert!(
            result.local.pairs.is_empty(),
            "changed source must remove whole and regional evidence"
        );
        drop(result);
        assert_eq!(budget.used(), 0);
        std::fs::write(&right_path, &saved).unwrap();
    }
    let retry = scan_projective_local_collection_spatial_gradient_scale_regions(
        [(2, b.clone()), (1, a.clone())],
        config,
        &factors,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(retry.local.pairs.len(), 1);
    assert!(retry.local.pairs[0].evidence.whole.candidate);
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
    let mut insufficient = config;
    insufficient.max_total_sample_pairs = 319999999;
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let files = std::iter::from_fn(|| -> Option<(u64, rrrah_decode::DecodeRequest)> {
        panic!("regional cumulative admission must precede source iteration")
    });
    assert!(matches!(
        scan_projective_local_collection_spatial_gradient_scale_regions(
            files,
            insufficient,
            &factors,
            &budget,
            || false,
        ),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert_eq!(budget.peak(), 0);
}

#[test]
fn distinct_managed_file_results_preserve_copy_refusal_and_last_owner() {
    use rrrah_dedup::local_scan::compare_local_files_projective_spatial_gradient_scales_distinct_managed as compare;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let a = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let spatial = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 32,
    };
    let mut p = policy();
    p.matching.max_comparisons = 8000000;
    for (name, expected) in [
        ("base.png", true),
        ("scale-117-angle-0.png", true),
        ("unrelated.png", false),
    ] {
        let b = rrrah_decode::DecodeRequest::new(root.join(name));
        let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        let result = compare(
            &a,
            &b,
            p,
            GradientCellRecipe::Interpolated,
            &factors,
            spatial,
            2.,
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(result.candidate, expected, "{name}: {result:?}");
        if expected {
            assert!(budget.used() > 0);
            assert!(!result.correspondences.is_empty());
        }
        let retained = budget.used();
        let clone = result.clone();
        drop(result);
        assert_eq!(budget.used(), retained);
        drop(clone);
        assert_eq!(budget.used(), 0);
    }
    let missing = rrrah_decode::DecodeRequest::new("/rrrah-missing-distinct-managed.png");
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    assert!(
        compare(
            &missing,
            &missing,
            p,
            GradientCellRecipe::Interpolated,
            &factors,
            spatial,
            f64::NAN,
            &budget,
            || false
        )
        .is_err()
    );
    assert_eq!(budget.peak(), 0);
}

#[test]
fn distinct_managed_file_cancel_mutation_and_restored_retry_release_credit() {
    use rrrah_dedup::local_scan::{
        LocalFileError, compare_local_files_projective_spatial_gradient_scales_distinct_managed as compare,
    };
    use std::io::Write;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.png");
    let right = dir.path().join("right.png");
    let saved = std::fs::read(root.join("scale-117-angle-0.png")).unwrap();
    std::fs::write(&left, std::fs::read(root.join("base.png")).unwrap()).unwrap();
    std::fs::write(&right, &saved).unwrap();
    let a = rrrah_decode::DecodeRequest::new(&left);
    let b = rrrah_decode::DecodeRequest::new(&right);
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let spatial = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 32,
    };
    let mut p = policy();
    p.matching.max_comparisons = 8000000;
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let result = compare(
        &a,
        &b,
        p,
        GradientCellRecipe::Interpolated,
        &factors,
        spatial,
        2.,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert!(result.candidate);
    let total = calls.get();
    drop(result);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        let result = compare(
            &a,
            &b,
            p,
            GradientCellRecipe::Interpolated,
            &factors,
            spatial,
            2.,
            &budget,
            || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        );
        assert!(matches!(result, Err(LocalFileError::Cancelled)), "stop={stop}");
        drop(result);
        assert_eq!(budget.used(), 0);
    }
    for stop in [total / 2, total - 10] {
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let result = compare(
            &a,
            &b,
            p,
            GradientCellRecipe::Interpolated,
            &factors,
            spatial,
            2.,
            &budget,
            || {
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
            },
        );
        assert!(changed.get());
        assert!(matches!(result, Err(LocalFileError::Source(_))), "{result:?}");
        drop(result);
        assert_eq!(budget.used(), 0);
        std::fs::write(&right, &saved).unwrap();
    }
    let retry = compare(
        &b,
        &a,
        p,
        GradientCellRecipe::Interpolated,
        &factors,
        spatial,
        2.,
        &budget,
        || false,
    )
    .unwrap();
    assert!(retry.candidate);
    drop(retry);
    assert_eq!(budget.used(), 0);
}

#[test]
fn distinct_managed_regions_common_source_lifecycle_and_admission() {
    use rrrah_dedup::local_scan::{
        LocalFileError, ProjectiveDistinctScaleRegionsFilePolicy,
        compare_local_files_projective_spatial_gradient_scale_regions_distinct_managed as compare,
    };
    use std::io::Write;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.png");
    let right = dir.path().join("right.png");
    let saved = std::fs::read(root.join("scale-117-angle-0.png")).unwrap();
    std::fs::write(&left, std::fs::read(root.join("base.png")).unwrap()).unwrap();
    std::fs::write(&right, &saved).unwrap();
    let a = rrrah_decode::DecodeRequest::new(&left);
    let b = rrrah_decode::DecodeRequest::new(&right);
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let mut gradient = policy();
    gradient.matching.max_comparisons = 8000000;
    let config = ProjectiveDistinctScaleRegionsFilePolicy {
        gradient,
        recipe: GradientCellRecipe::Interpolated,
        spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy {
            columns: 4,
            rows: 4,
            max_per_cell: 32,
        },
        competitor_radius: 2.,
        grid: (2, 2),
        max_regions: 8,
        max_total_sample_pairs: 320000000,
    };
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let baseline = compare(&a, &b, config, &factors, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert!(baseline.whole.candidate);
    assert_eq!(baseline.regions.as_ref().unwrap().region_support_count(), 8);
    let direct = rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
        &a,
        &b,
        baseline.whole.geometry.as_ref().unwrap().transform,
        rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {
            search: config.gradient.search,
            max_regions: 8,
            max_total_sample_pairs: 288000000,
        },
        (2, 2),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(
        format!("{:?}", baseline.regions.as_ref().unwrap()),
        format!("{direct:?}")
    );
    drop(direct);
    let total = calls.get();
    let region_clone = baseline.regions.as_ref().unwrap().regions.clone();
    let whole_clone = baseline.whole.clone();
    drop(baseline);
    assert!(budget.used() > 0);
    drop(region_clone);
    assert!(budget.used() > 0);
    drop(whole_clone);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        let result = compare(&a, &b, config, &factors, &budget, || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        });
        assert!(matches!(result, Err(LocalFileError::Cancelled)), "stop={stop}");
        drop(result);
        assert_eq!(budget.used(), 0);
    }
    for stop in [total / 2, total - 10] {
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let result = compare(&a, &b, config, &factors, &budget, || {
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
        drop(result);
        assert_eq!(budget.used(), 0);
        std::fs::write(&right, &saved).unwrap();
    }
    let retry = compare(&b, &a, config, &factors, &budget, || false).unwrap();
    assert!(retry.whole.candidate);
    assert_eq!(retry.regions.as_ref().unwrap().region_support_count(), 8);
    drop(retry);
    assert_eq!(budget.used(), 0);
    let missing = rrrah_decode::DecodeRequest::new("/rrrah-missing-distinct-region.png");
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let mut short = config;
    short.max_total_sample_pairs -= 1;
    assert!(matches!(
        compare(&missing, &missing, short, &factors, &budget, || false),
        Err(LocalFileError::Features(rrrah_dedup::local::LocalError::Budget))
    ));
    assert_eq!(budget.peak(), 0);
    let mut invalid = config;
    invalid.grid = (0, 2);
    assert!(matches!(
        compare(&missing, &missing, invalid, &factors, &budget, || false),
        Err(LocalFileError::InvalidPolicy)
    ));
    assert_eq!(budget.peak(), 0);
}

#[test]
fn distinct_managed_collection_matches_all_fresh_pairs_and_preiteration_admission() {
    use rrrah_dedup::{
        local_collection::{
            ProjectiveDistinctScaleRegionsCollectionPolicy,
            scan_projective_local_collection_spatial_gradient_scale_regions_distinct_managed as scan,
        },
        local_index::FileFeatureBudgets,
        local_scan::{
            ProjectiveDistinctScaleRegionsFilePolicy,
            compare_local_files_projective_spatial_gradient_scale_regions_distinct_managed as compare,
        },
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let files: Vec<_> = ["base.png", "scale-117-angle-0.png", "unrelated.png"]
        .into_iter()
        .enumerate()
        .map(|(i, n)| (i as u64 + 1, rrrah_decode::DecodeRequest::new(root.join(n))))
        .collect();
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let mut gradient = policy();
    gradient.matching.max_comparisons = 8000000;
    let search = ProjectiveDistinctScaleRegionsFilePolicy {
        gradient,
        recipe: GradientCellRecipe::Interpolated,
        spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy {
            columns: 4,
            rows: 4,
            max_per_cell: 32,
        },
        competitor_radius: 2.,
        grid: (2, 2),
        max_regions: 8,
        max_total_sample_pairs: 320000000,
    };
    let config = ProjectiveDistinctScaleRegionsCollectionPolicy {
        search,
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 6000,
            max_hits: 36000000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
        max_retrieval_comparisons: 36000000,
    };
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let report = scan(files.clone(), config, &factors, &budget, || false).unwrap();
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty()
    );
    for (left, right) in [(1, 2), (1, 3), (2, 3)] {
        let direct = compare(
            &files[left - 1].1,
            &files[right - 1].1,
            search,
            &factors,
            &budget,
            || false,
        )
        .unwrap();
        let local = direct.regions.as_ref().map_or(0, |r| r.region_support_count());
        let found = report
            .local
            .pairs
            .iter()
            .find(|p| p.left == left as u64 && p.right == right as u64);
        if direct.whole.candidate || local > 0 {
            assert!(found.is_some(), "retrieval lost ({left},{right})");
        }
        if let Some(pair) = found {
            assert_eq!(format!("{:?}", pair.evidence), format!("{direct:?}"));
        }
        if right == 3 {
            assert!(!direct.whole.candidate);
            assert_eq!(local, 0);
        }
        if (left, right) == (1, 2) {
            assert!(direct.whole.candidate);
            assert_eq!(local, 8);
        }
        drop(direct);
    }
    assert!(budget.used() > 0);
    drop(report);
    assert_eq!(budget.used(), 0);
    let mut short = config;
    short.search.max_total_sample_pairs -= 1;
    let files = std::iter::from_fn(|| -> Option<(u64, rrrah_decode::DecodeRequest)> {
        panic!("validation must precede iteration")
    });
    assert!(matches!(
        scan(files, short, &factors, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn distinct_managed_collection_cancel_changed_source_and_restored_retry() {
    use rrrah_dedup::{
        local_collection::{
            ProjectiveDistinctScaleRegionsCollectionPolicy,
            scan_projective_local_collection_spatial_gradient_scale_regions_distinct_managed as scan,
        },
        local_index::FileFeatureBudgets,
        local_scan::ProjectiveDistinctScaleRegionsFilePolicy,
    };
    use std::io::Write;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let saved = std::fs::read(root.join("base.png")).unwrap();
    let files: Vec<_> = (1..=3)
        .map(|id| {
            let path = dir.path().join(format!("{id}.png"));
            std::fs::write(&path, &saved).unwrap();
            (id, rrrah_decode::DecodeRequest::new(path))
        })
        .collect();
    let changed_path = dir.path().join("2.png");
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let mut gradient = policy();
    gradient.matching.max_comparisons = 8000000;
    let search = ProjectiveDistinctScaleRegionsFilePolicy {
        gradient,
        recipe: GradientCellRecipe::Interpolated,
        spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy {
            columns: 4,
            rows: 4,
            max_per_cell: 32,
        },
        competitor_radius: 2.,
        grid: (2, 2),
        max_regions: 8,
        max_total_sample_pairs: 320000000,
    };
    let config = ProjectiveDistinctScaleRegionsCollectionPolicy {
        search,
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 6000,
            max_hits: 36000000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
        max_retrieval_comparisons: 36000000,
    };
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0usize);
    let baseline = scan(files.clone(), config, &factors, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(baseline.local.pairs.len(), 3);
    assert!(
        baseline.local.pairs.iter().all(|p| p.evidence.whole.candidate
            && p.evidence.regions.as_ref().unwrap().region_support_count() == 8)
    );
    let total = calls.get();
    drop(baseline);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        let result = scan(files.clone(), config, &factors, &budget, || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        });
        assert!(
            matches!(result, Err(rrrah_dedup::scan::ScanError::Cancelled)),
            "stop={stop}: {result:?}"
        );
        drop(result);
        assert_eq!(budget.used(), 0);
    }
    for stop in [total / 2, total - 10] {
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let report = scan(files.clone(), config, &factors, &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&changed_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                changed.set(true);
            }
            false
        })
        .unwrap();
        assert!(changed.get(), "mutation checkpoint was not reached");
        assert!(
            report.local.source_issues.iter().any(|(id, _)| *id == 2),
            "{report:?}"
        );
        assert!(
            report.local.pairs.iter().all(|p| p.left != 2 && p.right != 2),
            "every changed-source edge must be removed"
        );
        assert!(
            report
                .local
                .pairs
                .iter()
                .any(|p| p.left == 1 && p.right == 3 && p.evidence.whole.candidate),
            "unchanged-source edge must survive"
        );
        drop(report);
        assert_eq!(budget.used(), 0);
        std::fs::write(&changed_path, &saved).unwrap();
    }
    let retry = scan(files.into_iter().rev(), config, &factors, &budget, || false).unwrap();
    assert!(
        retry.local.source_issues.is_empty() && retry.local.issues.is_empty() && retry.file_issues.is_empty()
    );
    assert_eq!(retry.local.pairs.len(), 3);
    assert!(
        retry.local.pairs.iter().all(|p| p.evidence.whole.candidate
            && p.evidence.regions.as_ref().unwrap().region_support_count() == 8)
    );
    drop(retry);
    assert_eq!(budget.used(), 0);
}

#[test]
fn area_distinct_managed_file_cancel_mutation_and_restored_retry_release_credit() {
    use rrrah_dedup::local_scan::{
        LocalFileError,
        compare_local_files_projective_spatial_gradient_scales_area_distinct_managed as compare,
    };
    use std::io::Write;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.png");
    let right = dir.path().join("right.png");
    let saved = std::fs::read(root.join("scale-117-angle-0.png")).unwrap();
    std::fs::write(&left, std::fs::read(root.join("base.png")).unwrap()).unwrap();
    std::fs::write(&right, &saved).unwrap();
    let a = rrrah_decode::DecodeRequest::new(&left);
    let b = rrrah_decode::DecodeRequest::new(&right);
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let spatial = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 32,
    };
    let mut p = policy();
    p.matching.max_comparisons = 8000000;
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let result = compare(
        &a,
        &b,
        p,
        GradientCellRecipe::Interpolated,
        &factors,
        spatial,
        2.,
        1000000,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert!(result.candidate);
    let total = calls.get();
    drop(result);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        let result = compare(
            &a,
            &b,
            p,
            GradientCellRecipe::Interpolated,
            &factors,
            spatial,
            2.,
            1000000,
            &budget,
            || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        );
        assert!(matches!(result, Err(LocalFileError::Cancelled)), "stop={stop}");
        drop(result);
        assert_eq!(budget.used(), 0);
    }
    for stop in [total / 2, total - 10] {
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let result = compare(
            &a,
            &b,
            p,
            GradientCellRecipe::Interpolated,
            &factors,
            spatial,
            2.,
            1000000,
            &budget,
            || {
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
            },
        );
        assert!(changed.get());
        assert!(matches!(result, Err(LocalFileError::Source(_))), "{result:?}");
        drop(result);
        assert_eq!(budget.used(), 0);
        std::fs::write(&right, &saved).unwrap();
    }
    let retry = compare(
        &b,
        &a,
        p,
        GradientCellRecipe::Interpolated,
        &factors,
        spatial,
        2.,
        1000000,
        &budget,
        || false,
    )
    .unwrap();
    assert!(retry.candidate);
    drop(retry);
    assert_eq!(budget.used(), 0);
}

#[test]
fn area_distinct_file_keeps_original_confirmation_and_preio_admission() {
    use rrrah_dedup::local_scan::{
        compare_local_files_projective_spatial_gradient_scales_area_distinct_managed as area,
        compare_local_files_projective_spatial_gradient_scales_distinct_managed as point,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let left = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let spatial = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 32,
    };
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let mut config = policy();
    config.matching.max_comparisons = 8000000;
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    for (name, expected) in [
        ("base.png", true),
        ("scale-117-angle-0.png", true),
        ("unrelated.png", false),
    ] {
        let right = rrrah_decode::DecodeRequest::new(root.join(name));
        let result = area(
            &left,
            &right,
            config,
            GradientCellRecipe::Interpolated,
            &factors,
            spatial,
            2.,
            1000000,
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(result.candidate, expected, "{name}");
        drop(result);
        assert_eq!(budget.used(), 0);
    }
    let a = area(
        &left,
        &left,
        config,
        GradientCellRecipe::Interpolated,
        &[1.],
        spatial,
        2.,
        0,
        &budget,
        || false,
    )
    .unwrap();
    let b = point(
        &left,
        &left,
        config,
        GradientCellRecipe::Interpolated,
        &[1.],
        spatial,
        2.,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    drop(a);
    drop(b);
    assert_eq!(budget.used(), 0);
    let missing = rrrah_decode::DecodeRequest::new("/rrrah-missing-area-source.png");
    let fresh = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    assert!(matches!(
        area(
            &missing,
            &missing,
            config,
            GradientCellRecipe::Interpolated,
            &factors,
            spatial,
            2.,
            0,
            &fresh,
            || false
        ),
        Err(rrrah_dedup::local_scan::LocalFileError::Features(
            rrrah_dedup::local::LocalError::Budget
        ))
    ));
    assert_eq!(fresh.peak(), 0);
}

#[test]
fn area_distinct_regions_common_source_lifecycle_and_admission() {
    use rrrah_dedup::local_scan::{
        LocalFileError, ProjectiveDistinctScaleRegionsFilePolicy,
        compare_local_files_projective_spatial_gradient_scale_regions_area_distinct_managed as compare,
    };
    use std::io::Write;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left.png");
    let right = dir.path().join("right.png");
    let saved = std::fs::read(root.join("scale-117-angle-0.png")).unwrap();
    std::fs::write(&left, std::fs::read(root.join("base.png")).unwrap()).unwrap();
    std::fs::write(&right, &saved).unwrap();
    let a = rrrah_decode::DecodeRequest::new(&left);
    let b = rrrah_decode::DecodeRequest::new(&right);
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let mut gradient = policy();
    gradient.matching.max_comparisons = 8000000;
    let config = ProjectiveDistinctScaleRegionsFilePolicy {
        gradient,
        recipe: GradientCellRecipe::Interpolated,
        spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy {
            columns: 4,
            rows: 4,
            max_per_cell: 32,
        },
        competitor_radius: 2.,
        grid: (2, 2),
        max_regions: 8,
        max_total_sample_pairs: 320000000,
    };
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let baseline = compare(&a, &b, config, &factors, 1000000, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert!(baseline.whole.candidate);
    assert_eq!(baseline.regions.as_ref().unwrap().region_support_count(), 8);
    let direct = rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform(
        &a,
        &b,
        baseline.whole.geometry.as_ref().unwrap().transform,
        rrrah_dedup::local_scan::ProjectivePyramidRegionsFilePolicy {
            search: config.gradient.search,
            max_regions: 8,
            max_total_sample_pairs: 288000000,
        },
        (2, 2),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(
        format!("{:?}", baseline.regions.as_ref().unwrap()),
        format!("{direct:?}")
    );
    drop(direct);
    let total = calls.get();
    let region_clone = baseline.regions.as_ref().unwrap().regions.clone();
    let whole_clone = baseline.whole.clone();
    drop(baseline);
    assert!(budget.used() > 0);
    drop(region_clone);
    assert!(budget.used() > 0);
    drop(whole_clone);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        let result = compare(&a, &b, config, &factors, 1000000, &budget, || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        });
        assert!(matches!(result, Err(LocalFileError::Cancelled)), "stop={stop}");
        drop(result);
        assert_eq!(budget.used(), 0);
    }
    for stop in [total / 2, total - 10] {
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let result = compare(&a, &b, config, &factors, 1000000, &budget, || {
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
        drop(result);
        assert_eq!(budget.used(), 0);
        std::fs::write(&right, &saved).unwrap();
    }
    let retry = compare(&b, &a, config, &factors, 1000000, &budget, || false).unwrap();
    assert!(retry.whole.candidate);
    assert_eq!(retry.regions.as_ref().unwrap().region_support_count(), 8);
    drop(retry);
    assert_eq!(budget.used(), 0);
    let missing = rrrah_decode::DecodeRequest::new("/rrrah-missing-distinct-region.png");
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let mut short = config;
    short.max_total_sample_pairs -= 1;
    assert!(matches!(
        compare(&missing, &missing, short, &factors, 1000000, &budget, || false),
        Err(LocalFileError::Features(rrrah_dedup::local::LocalError::Budget))
    ));
    assert_eq!(budget.peak(), 0);
    let mut invalid = config;
    invalid.grid = (0, 2);
    assert!(matches!(
        compare(&missing, &missing, invalid, &factors, 1000000, &budget, || false),
        Err(LocalFileError::InvalidPolicy)
    ));
    assert_eq!(budget.peak(), 0);
}

#[test]
fn area_distinct_collection_matches_all_fresh_pairs_and_preiteration_admission() {
    use rrrah_dedup::{
        local_collection::{
            ProjectiveDistinctScaleRegionsCollectionPolicy,
            scan_projective_local_collection_spatial_gradient_scale_regions_area_distinct_managed as scan,
        },
        local_index::FileFeatureBudgets,
        local_scan::{
            ProjectiveDistinctScaleRegionsFilePolicy,
            compare_local_files_projective_spatial_gradient_scale_regions_area_distinct_managed as compare,
        },
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let files: Vec<_> = ["base.png", "scale-117-angle-0.png", "unrelated.png"]
        .into_iter()
        .enumerate()
        .map(|(i, n)| (i as u64 + 1, rrrah_decode::DecodeRequest::new(root.join(n))))
        .collect();
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let mut gradient = policy();
    gradient.matching.max_comparisons = 8000000;
    let search = ProjectiveDistinctScaleRegionsFilePolicy {
        gradient,
        recipe: GradientCellRecipe::Interpolated,
        spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy {
            columns: 4,
            rows: 4,
            max_per_cell: 32,
        },
        competitor_radius: 2.,
        grid: (2, 2),
        max_regions: 8,
        max_total_sample_pairs: 320000000,
    };
    let config = ProjectiveDistinctScaleRegionsCollectionPolicy {
        search,
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 6000,
            max_hits: 36000000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
        max_retrieval_comparisons: 36000000,
    };
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let report = scan(files.clone(), config, &factors, 1000000, &budget, || false).unwrap();
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty()
    );
    for (left, right) in [(1, 2), (1, 3), (2, 3)] {
        let direct = compare(
            &files[left - 1].1,
            &files[right - 1].1,
            search,
            &factors,
            1000000,
            &budget,
            || false,
        )
        .unwrap();
        let local = direct.regions.as_ref().map_or(0, |r| r.region_support_count());
        let found = report
            .local
            .pairs
            .iter()
            .find(|p| p.left == left as u64 && p.right == right as u64);
        if direct.whole.candidate || local > 0 {
            assert!(found.is_some(), "retrieval lost ({left},{right})");
        }
        if let Some(pair) = found {
            assert_eq!(format!("{:?}", pair.evidence), format!("{direct:?}"));
        }
        if right == 3 {
            assert!(!direct.whole.candidate);
            assert_eq!(local, 0);
        }
        if (left, right) == (1, 2) {
            assert!(direct.whole.candidate);
            assert_eq!(local, 8);
        }
        drop(direct);
    }
    assert!(budget.used() > 0);
    drop(report);
    assert_eq!(budget.used(), 0);
    let no_files = std::iter::from_fn(|| -> Option<(u64, rrrah_decode::DecodeRequest)> {
        panic!("zero area work must refuse before iteration")
    });
    assert!(matches!(
        scan(no_files, config, &factors, 0, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    let mut short = config;
    short.search.max_total_sample_pairs -= 1;
    let files = std::iter::from_fn(|| -> Option<(u64, rrrah_decode::DecodeRequest)> {
        panic!("validation must precede iteration")
    });
    assert!(matches!(
        scan(files, short, &factors, 1000000, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn area_distinct_collection_cancel_changed_source_and_restored_retry() {
    use rrrah_dedup::{
        local_collection::{
            ProjectiveDistinctScaleRegionsCollectionPolicy,
            scan_projective_local_collection_spatial_gradient_scale_regions_area_distinct_managed as scan,
        },
        local_index::FileFeatureBudgets,
        local_scan::ProjectiveDistinctScaleRegionsFilePolicy,
    };
    use std::io::Write;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let dir = tempfile::tempdir().unwrap();
    let saved = std::fs::read(root.join("base.png")).unwrap();
    let files: Vec<_> = (1..=3)
        .map(|id| {
            let path = dir.path().join(format!("{id}.png"));
            std::fs::write(&path, &saved).unwrap();
            (id, rrrah_decode::DecodeRequest::new(path))
        })
        .collect();
    let changed_path = dir.path().join("2.png");
    let factors = [1., std::f64::consts::SQRT_2, 2., 2. * std::f64::consts::SQRT_2];
    let mut gradient = policy();
    gradient.matching.max_comparisons = 8000000;
    let search = ProjectiveDistinctScaleRegionsFilePolicy {
        gradient,
        recipe: GradientCellRecipe::Interpolated,
        spatial: rrrah_dedup::local_scan::SpatialFeaturePolicy {
            columns: 4,
            rows: 4,
            max_per_cell: 32,
        },
        competitor_radius: 2.,
        grid: (2, 2),
        max_regions: 8,
        max_total_sample_pairs: 320000000,
    };
    let config = ProjectiveDistinctScaleRegionsCollectionPolicy {
        search,
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 6000,
            max_hits: 36000000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
        max_retrieval_comparisons: 36000000,
    };
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0usize);
    let baseline = scan(files.clone(), config, &factors, 1000000, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(baseline.local.pairs.len(), 3);
    assert!(
        baseline.local.pairs.iter().all(|p| p.evidence.whole.candidate
            && p.evidence.regions.as_ref().unwrap().region_support_count() == 8)
    );
    let total = calls.get();
    drop(baseline);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        let result = scan(files.clone(), config, &factors, 1000000, &budget, || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        });
        assert!(
            matches!(result, Err(rrrah_dedup::scan::ScanError::Cancelled)),
            "stop={stop}: {result:?}"
        );
        drop(result);
        assert_eq!(budget.used(), 0);
    }
    for stop in [total / 2, total - 10] {
        calls.set(0);
        let changed = std::cell::Cell::new(false);
        let report = scan(files.clone(), config, &factors, 1000000, &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&changed_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                changed.set(true);
            }
            false
        })
        .unwrap();
        assert!(changed.get(), "mutation checkpoint was not reached");
        assert!(
            report.local.source_issues.iter().any(|(id, _)| *id == 2),
            "{report:?}"
        );
        assert!(
            report.local.pairs.iter().all(|p| p.left != 2 && p.right != 2),
            "every changed-source edge must be removed"
        );
        assert!(
            report
                .local
                .pairs
                .iter()
                .any(|p| p.left == 1 && p.right == 3 && p.evidence.whole.candidate),
            "unchanged-source edge must survive"
        );
        drop(report);
        assert_eq!(budget.used(), 0);
        std::fs::write(&changed_path, &saved).unwrap();
    }
    let retry = scan(
        files.into_iter().rev(),
        config,
        &factors,
        1000000,
        &budget,
        || false,
    )
    .unwrap();
    assert!(
        retry.local.source_issues.is_empty() && retry.local.issues.is_empty() && retry.file_issues.is_empty()
    );
    assert_eq!(retry.local.pairs.len(), 3);
    assert!(
        retry.local.pairs.iter().all(|p| p.evidence.whole.candidate
            && p.evidence.regions.as_ref().unwrap().region_support_count() == 8)
    );
    drop(retry);
    assert_eq!(budget.used(), 0);
}
