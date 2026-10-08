#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::GeometryPolicy,
    local::{LocalPolicy, MatchPolicy},
    local_scan::{LocalFilePolicy, compare_local_files_projective},
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
fn automatic_region_grid_matches_declared_regions_and_lifecycle() {
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local_scan::{
            LocalFileError, ProjectivePyramidPhotometricFilePolicy,
            compare_local_files_projective_pyramid_photometric,
        },
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy},
    };
    use std::cell::Cell;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let a = DecodeRequest::new(root.join("2414-base.png"));
    let b = DecodeRequest::new(root.join("5495-base.png"));
    let mut local = policy();
    local.extract.max_features = 128;
    local.matching.max_comparisons = 384 * 384;
    local.matching.max_distance = 64;
    local.geometry.max_points = 384;
    local.geometry.max_hypotheses = 2048;
    local.geometry.min_inliers = 10;
    local.geometry.tolerance = 2.;
    local.pixels.tolerance = 0.03;
    local.minimum_coverage_fraction = 0.3;
    local.minimum_matched_fraction = 0.9;
    let p = ProjectivePyramidPhotometricFilePolicy {
        local,
        max_levels: 3,
        max_total_pixels: 400_000,
        max_total_features: 384,
        sampling: ProjectiveSamplingPolicy {
            trials: 2048,
            seed: 0x1234abcd,
        },
        photometric: PhotometricPolicy {
            residual: local.pixels,
            minimum_samples: 16,
            minimum_variance: 1e-5,
            minimum_gain: 0.2,
            maximum_gain: 5.,
            maximum_offset: 0.1,
        },
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 1,
                max_sample_pairs: 12_000_000,
            },
            color_space: FilterColorSpace::LinearSrgb,
        },
        fit_mode: PhotometricFitMode::ConstrainedLeastSquares,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);

    use rrrah_dedup::local_scan::{
        ProjectivePyramidRegionsFilePolicy, compare_local_files_projective_pyramid_region_grid,
        compare_local_files_projective_pyramid_regions,
    };
    let q = ProjectivePyramidRegionsFilePolicy {
        search: p,
        max_regions: 4,
        max_total_sample_pairs: 60_000_000,
    };
    let calls = Cell::new(0);
    let automatic = compare_local_files_projective_pyramid_region_grid(&a, &a, q, (2, 2), &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    assert!(automatic.whole.candidate);
    assert_eq!(automatic.regions.len(), 4);
    assert_eq!(automatic.region_support_count(), 4);
    assert!(automatic.regions.iter().all(|region| region.accepted_region));
    let domains = automatic.regions.iter().map(|r| r.domains).collect::<Vec<_>>();
    let declared =
        compare_local_files_projective_pyramid_regions(&a, &a, q, &domains, &budget, || false).unwrap();
    assert_eq!(automatic.whole.candidate, declared.whole.candidate);
    assert_eq!(
        automatic.whole.geometry.as_ref().unwrap().transform,
        declared.whole.geometry.as_ref().unwrap().transform
    );
    for (left, right) in automatic.regions.iter().zip(declared.regions.iter()) {
        assert_eq!(left.accepted_region, right.accepted_region);
        assert_eq!(left.domains, right.domains);
        assert_eq!(format!("{:?}", left.pixels), format!("{:?}", right.pixels));
        assert_eq!(left.fit_failure, right.fit_failure);
    }
    drop(automatic);
    drop(declared);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(matches!(
            compare_local_files_projective_pyramid_region_grid(&a, &a, q, (2, 2), &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(LocalFileError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    // Mutation after both source snapshots must invalidate all computed regional evidence.
    let directory = tempfile::tempdir().unwrap();
    let changed_path = directory.path().join("changed.png");
    std::fs::copy(root.join("2414-base.png"), &changed_path).unwrap();
    let changed_request = DecodeRequest::new(&changed_path);
    calls.set(0);
    drop(
        compare_local_files_projective_pyramid_region_grid(&a, &changed_request, q, (2, 2), &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    let change_at = calls.get() / 2;
    let changed = Cell::new(false);
    calls.set(0);
    let invalidated =
        compare_local_files_projective_pyramid_region_grid(&a, &changed_request, q, (2, 2), &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == change_at {
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
    assert!(matches!(
        invalidated,
        Err(LocalFileError::Source(rrrah_dedup::exact::SnapshotError::Changed))
    ));
    assert_eq!(budget.used(), 0);
    std::fs::copy(root.join("2414-base.png"), &changed_path).unwrap();
    drop(
        compare_local_files_projective_pyramid_region_grid(&a, &changed_request, q, (2, 2), &budget, || {
            false
        })
        .unwrap(),
    );
    assert_eq!(budget.used(), 0);
    let missing = DecodeRequest::new("missing-region-grid.png");
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    let mut short = q;
    short.max_total_sample_pairs -= 1;
    assert!(matches!(
        compare_local_files_projective_pyramid_region_grid(&missing, &missing, short, (2, 2), &fresh, || {
            false
        }),
        Err(LocalFileError::Pixels(rrrah_dedup::warp::WarpError::Budget))
    ));
    assert_eq!(fresh.peak(), 0);
    assert!(matches!(
        compare_local_files_projective_pyramid_region_grid(&missing, &missing, q, (0, 2), &fresh, || false),
        Err(LocalFileError::InvalidPolicy)
    ));
    assert_eq!(fresh.peak(), 0);
    let unrelated =
        compare_local_files_projective_pyramid_region_grid(&a, &b, q, (2, 2), &budget, || false).unwrap();
    assert!(!unrelated.whole.candidate);
    drop(unrelated);
    assert_eq!(budget.used(), 0);
    use rrrah_dedup::{
        local_collection::{
            ProjectiveRegionGridCollectionPolicy, scan_projective_local_collection_region_grid,
        },
        local_index::FileFeatureBudgets,
    };
    let config = ProjectiveRegionGridCollectionPolicy {
        search: q,
        grid: (2, 2),
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 1152,
            max_hits: 2_000_000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
    };
    let files = vec![(1, a.clone()), (2, changed_request.clone()), (3, b.clone())];
    calls.set(0);
    let report = scan_projective_local_collection_region_grid(files.clone(), config, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    assert_eq!(report.analysed, vec![1, 2, 3]);
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
            .any(|pair| pair.left == 1 && pair.right == 2 && pair.evidence.whole.candidate)
    );
    for pair in &report.local.pairs {
        let direct = compare_local_files_projective_pyramid_region_grid(
            &files[(pair.left - 1) as usize].1,
            &files[(pair.right - 1) as usize].1,
            q,
            (2, 2),
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(pair.evidence.whole.candidate, direct.whole.candidate);
        assert_eq!(
            format!("{:?}", pair.evidence.regions),
            format!("{:?}", direct.regions)
        );
    }
    let passes = |v: &rrrah_dedup::warp::WarpEvidence| {
        v.compared_pixels >= p.local.minimum_compared_pixels
            && v.compared_pixels as f64 >= v.source_pixels as f64 * p.local.minimum_coverage_fraction
            && v.matched_pixels as f64 >= v.compared_pixels as f64 * p.local.minimum_matched_fraction
    };
    for pair in &report.local.pairs {
        if pair.right == 3 {
            assert!(!pair.evidence.whole.candidate);
            assert_eq!(pair.evidence.region_support_count(), 0);
            assert!(!pair.evidence.regions.iter().any(|region| {
                region
                    .pixels
                    .as_ref()
                    .is_some_and(|e| passes(&e.fitted.forward.pixels) && passes(&e.fitted.reverse.pixels))
            }));
        }
    }
    drop(report);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(matches!(
            scan_projective_local_collection_region_grid(files.clone(), config, &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(rrrah_dedup::scan::ScanError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }

    changed.set(false);
    calls.set(0);
    let invalidated = scan_projective_local_collection_region_grid(files.clone(), config, &budget, || {
        calls.set(calls.get() + 1);
        if calls.get() == total / 2 {
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
    })
    .unwrap();
    assert!(changed.get());
    assert!(invalidated.local.source_issues.iter().any(|(id, _)| *id == 2));
    assert!(!invalidated.analysed.contains(&2));
    assert!(
        invalidated
            .local
            .pairs
            .iter()
            .all(|pair| pair.left != 2 && pair.right != 2)
    );
    drop(invalidated);
    assert_eq!(budget.used(), 0);
    std::fs::copy(root.join("2414-base.png"), &changed_path).unwrap();
    let mut reversed = files.clone();
    reversed.reverse();
    let retry = scan_projective_local_collection_region_grid(reversed, config, &budget, || false).unwrap();
    assert!(
        retry
            .local
            .pairs
            .iter()
            .any(|pair| pair.left == 1 && pair.right == 2 && pair.evidence.whole.candidate)
    );
    drop(retry);
    assert_eq!(budget.used(), 0);
    let mut invalid = config;
    invalid.grid = (0, 2);
    assert!(matches!(
        scan_projective_local_collection_region_grid([(1, missing.clone())], invalid, &fresh, || false),
        Err(rrrah_dedup::scan::ScanError::InvalidPolicy)
    ));
    assert_eq!(fresh.peak(), 0);
    let mut short = config;
    short.search.max_total_sample_pairs -= 1;
    assert!(matches!(
        scan_projective_local_collection_region_grid([(1, missing)], short, &fresh, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert_eq!(fresh.peak(), 0);
}
