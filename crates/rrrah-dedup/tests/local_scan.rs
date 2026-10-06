#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::GeometryPolicy,
    local::{LocalPolicy, MatchPolicy},
    local_scan::{LocalFileError, LocalFilePolicy, compare_local_files, scan_local_files},
    scan::ScanError,
    warp::WarpPolicy,
};
use std::path::PathBuf;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
fn policy() -> LocalFilePolicy {
    LocalFilePolicy {
        decode: AnimationBudget {
            max_frames: 10,
            max_pixels: 50000,
            max_file_bytes: 1024 * 1024,
        },
        extract: LocalPolicy {
            max_pixels: 50000,
            max_candidates: 50000,
            max_features: 500,
            minimum_corner_score: 0.0001,
        },
        matching: MatchPolicy {
            max_comparisons: 250_000,
            max_distance: 64,
        },
        geometry: GeometryPolicy {
            tolerance: 2.,
            min_inliers: 10,
            max_points: 500,
            max_hypotheses: 125_000,
        },
        pixels: WarpPolicy {
            tolerance: 0.03,
            max_source_pixels: 50000,
        },
        minimum_compared_pixels: 1000,
        minimum_coverage_fraction: 0.3,
        minimum_matched_fraction: 0.9,
    }
}
fn files() -> Vec<(u64, DecodeRequest)> {
    vec![
        (1, DecodeRequest::new(root().join("rotation/base.png"))),
        (2, DecodeRequest::new(root().join("local-files/crop.png"))),
        (
            3,
            DecodeRequest::new(root().join("rotation/scale-216-angle-17.png")),
        ),
        (4, DecodeRequest::new(root().join("rotation/unrelated.png"))),
    ]
}
#[test]
fn full_file_search_finds_crop_scale_rotation_and_rejects_unrelated_and_corrupt_sources() {
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let mut requests = files();
    let dir = tempfile::tempdir().unwrap();
    let broken = dir.path().join("broken.png");
    std::fs::write(&broken, b"not an image").unwrap();
    requests.push((5, DecodeRequest::new(broken)));
    let report = scan_local_files(requests, policy(), 5, 10, &budget, || false).unwrap();
    let candidates = report
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<Vec<_>>();
    println!("candidates: {candidates:?}");
    assert_eq!(candidates, [(1, 2), (1, 3), (2, 3)]);
    for pair in report.pairs.iter().filter(|p| p.evidence.candidate) {
        let evidence = &pair.evidence;
        let geometry = evidence.geometry.as_ref().unwrap();
        assert!(geometry.inliers.len() >= 10);
        assert!(
            geometry
                .inliers
                .iter()
                .all(|&index| index < evidence.correspondences.len())
        );
        assert!(
            evidence.correspondences.iter().all(|point| point
                .source
                .iter()
                .chain(&point.target)
                .all(|v| v.is_finite()))
        );
    }

    assert_eq!(report.pairs.len(), 6);
    assert_eq!(report.issues.len(), 4);
    assert!(report.issues.iter().all(|&(_, right, _)| right == 5));
    assert!(report.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
}
#[test]
fn file_pair_limits_cancel_and_invalid_acceptance_are_inconclusive() {
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    assert!(matches!(
        scan_local_files(files(), policy(), 4, 5, &budget, || false),
        Err(ScanError::Budget)
    ));
    assert_eq!(budget.peak(), 0);
    assert!(matches!(
        scan_local_files(files(), policy(), 3, 6, &budget, || false),
        Err(ScanError::Budget)
    ));
    assert!(matches!(
        scan_local_files(files(), policy(), 4, 6, &budget, || true),
        Err(ScanError::Cancelled)
    ));
    let mut invalid = policy();
    invalid.minimum_matched_fraction = f64::NAN;
    assert!(matches!(
        compare_local_files(&files()[0].1, &files()[1].1, invalid, &budget, || false),
        Err(LocalFileError::InvalidPolicy)
    ));
    assert!(matches!(
        scan_local_files(files(), invalid, 4, 6, &budget, || false),
        Err(ScanError::InvalidPolicy)
    ));
    let mut duplicate = files();
    duplicate[1].0 = 1;
    assert!(matches!(
        scan_local_files(duplicate, policy(), 4, 6, &budget, || false),
        Err(ScanError::DuplicateId)
    ));
    assert_eq!(budget.used(), 0);
}

fn stable_triple() -> (tempfile::TempDir, Vec<(u64, DecodeRequest)>) {
    let folder = tempfile::tempdir().unwrap();
    let paths = [
        ("one.png", "rotation/base.png"),
        ("two.png", "rotation/base.png"),
        ("three.png", "rotation/scale-216-angle-17.png"),
    ];
    let files = paths
        .into_iter()
        .enumerate()
        .map(|(index, (name, source))| {
            let path = folder.path().join(name);
            std::fs::copy(root().join(source), &path).unwrap();
            (u64::try_from(index).unwrap() + 1, DecodeRequest::new(path))
        })
        .collect();
    (folder, files)
}
#[test]
fn batch_source_mutation_discards_all_stale_pairs_and_keeps_healthy_evidence() {
    use std::io::Write;
    let (_folder, requests) = stable_triple();
    let path = requests[0].1.path.clone();
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let active = std::cell::Cell::new(false);
    let changed = std::cell::Cell::new(false);
    let report = scan_local_files(requests, policy(), 3, 3, &budget, || {
        if budget.used() > 0 {
            active.set(true);
        } else if active.get() && !changed.replace(true) {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap()
                .write_all(&[0])
                .unwrap();
        }
        false
    })
    .unwrap();
    assert!(changed.get());
    assert_eq!(report.source_issues.len(), 1);
    assert_eq!(report.source_issues[0].0, 1);
    assert!(matches!(
        report.source_issues[0].1,
        rrrah_dedup::exact::SnapshotError::Changed
    ));
    assert_eq!(report.pairs.len(), 1);
    assert_eq!((report.pairs[0].left, report.pairs[0].right), (2, 3));
    assert!(report.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
}
#[test]
fn final_generation_change_and_one_shot_cancellation_return_no_partial_report() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let (_folder, mut requests) = stable_triple();
    let generation = Arc::new(AtomicU64::new(7));
    for (_, request) in &mut requests {
        request.cancellation = Some(rrrah_decode::GenerationToken::new(Arc::clone(&generation), 7));
    }
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = std::cell::Cell::new(0_usize);
    let report = scan_local_files(requests.clone(), policy(), 3, 3, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(report.pairs.len(), 3);
    let stop = calls.get();
    calls.set(0);
    assert!(matches!(
        scan_local_files(requests.clone(), policy(), 3, 3, &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                generation.store(8, Ordering::Release);
            }
            false
        }),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    generation.store(7, Ordering::Release);
    calls.set(0);
    assert!(matches!(
        scan_local_files(requests, policy(), 3, 3, &budget, || {
            calls.set(calls.get() + 1);
            calls.get() == stop - 1
        }),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn recursive_local_search_preserves_paths_aliases_and_decode_errors() {
    let folder = tempfile::tempdir().unwrap();
    let nested = folder.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let base = folder.path().join("base.png");
    std::fs::copy(root().join("rotation/base.png"), &base).unwrap();
    std::fs::copy(root().join("local-files/crop.png"), nested.join("crop.png")).unwrap();
    std::fs::copy(
        root().join("rotation/scale-216-angle-17.png"),
        nested.join("scaled.png"),
    )
    .unwrap();
    std::fs::write(nested.join("broken.png"), b"bad source").unwrap();
    std::fs::hard_link(&base, folder.path().join("alias.png")).unwrap();
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let traversal = rrrah_dedup::exact::Options {
        follow_symlinks: false,
        max_depth: 10,
        max_entries: 100,
        max_file_bytes: 1024 * 1024,
    };
    let report = rrrah_dedup::local_scan::scan_local_roots(
        &[folder.path().to_path_buf(), nested],
        &traversal,
        policy(),
        4,
        6,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.files.len(), 4);
    assert_eq!(report.aliases.len(), 1);
    assert!(report.traversal_issues.is_empty());
    assert_eq!(report.local.issues.len(), 3);
    assert_eq!(
        report.local.pairs.iter().filter(|p| p.evidence.candidate).count(),
        3
    );
    assert!(report.files.iter().all(|(_, path)| path.is_file()));
    assert_eq!(budget.used(), 0);
}

fn fitted_search(fit: rrrah_dedup::warp::PhotometricFitMode) -> rrrah_dedup::local_scan::LocalSearchPolicy {
    use rrrah_dedup::{
        local_scan::{LocalComparisonMode, LocalSearchPolicy},
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricPolicy},
    };
    LocalSearchPolicy {
        local: policy(),
        comparison: LocalComparisonMode::Filtered {
            photometric: PhotometricPolicy {
                residual: policy().pixels,
                minimum_samples: 1000,
                minimum_variance: 1e-5,
                minimum_gain: 0.2,
                maximum_gain: 5.0,
                maximum_offset: 0.1,
            },
            filter: ColorFilterPolicy {
                filter: FilterPolicy {
                    radius: 3,
                    max_sample_pairs: 20_000_000,
                },
                color_space: FilterColorSpace::EncodedSrgb,
            },
            fit,
        },
    }
}
#[test]
fn fitted_batch_and_roots_preserve_signal_selection_and_corrupt_file_diagnostics() {
    use rrrah_dedup::{
        local_scan::{scan_local_files_with_policy, scan_local_roots_with_policy},
        warp::PhotometricFitMode,
    };
    let folder = tempfile::tempdir().unwrap();
    let nested = folder.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let base_path = folder.path().join("base.png");
    let dark_path = nested.join("dark.png");
    let broken = nested.join("broken.png");
    std::fs::copy(root().join("photos/830-base.png"), &base_path).unwrap();
    std::fs::copy(root().join("photos/830-brightness.png"), &dark_path).unwrap();
    std::fs::write(&broken, b"corrupt").unwrap();
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for fit in [
        PhotometricFitMode::RejectOutsidePolicy,
        PhotometricFitMode::ConstrainedLeastSquares,
    ] {
        let mut search = fitted_search(fit);
        search.local.decode.max_pixels = 200_000;
        search.local.extract.max_pixels = 200_000;
        search.local.extract.max_candidates = 200_000;
        search.local.pixels.max_source_pixels = 200_000;
        if let rrrah_dedup::local_scan::LocalComparisonMode::Filtered {
            ref mut photometric, ..
        } = search.comparison
        {
            photometric.residual = search.local.pixels;
        }
        let requests = vec![
            (20, DecodeRequest::new(&dark_path)),
            (10, DecodeRequest::new(&base_path)),
            (30, DecodeRequest::new(&broken)),
        ];
        let report = scan_local_files_with_policy(requests, search, 3, 3, &budget, || false).unwrap();
        assert_eq!(report.pairs.len(), 1);
        assert_eq!(report.issues.len(), 2);
        let evidence = &report.pairs[0].evidence;
        assert!(evidence.candidate);
        assert_eq!((report.pairs[0].left, report.pairs[0].right), (10, 20));
        let filtered = evidence.filtered.as_ref().unwrap();
        assert_eq!(filtered.fit_mode, fit);
        assert!(evidence.pixels.is_some());
        let strict = evidence.pixels.as_ref().unwrap();
        assert!(strict.forward.matched_pixels * 10 < strict.forward.compared_pixels * 9);
        let traversal = rrrah_dedup::exact::Options {
            follow_symlinks: false,
            max_depth: 10,
            max_entries: 100,
            max_file_bytes: 1024 * 1024,
        };
        let report = scan_local_roots_with_policy(
            &[folder.path().to_path_buf(), nested.clone()],
            &traversal,
            search,
            3,
            3,
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(report.files.len(), 3);
        assert_eq!(report.local.pairs.len(), 1);
        assert_eq!(report.local.issues.len(), 2);
        assert!(report.local.pairs[0].evidence.candidate);
        assert_eq!(
            report.local.pairs[0].evidence.filtered.as_ref().unwrap().fit_mode,
            fit
        );
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn fitted_scan_validates_policy_before_consuming_input() {
    use rrrah_dedup::{
        local_scan::{LocalComparisonMode, scan_local_files_with_policy},
        warp::PhotometricFitMode,
    };
    let mut p = fitted_search(PhotometricFitMode::ConstrainedLeastSquares);
    if let LocalComparisonMode::Filtered { ref mut filter, .. } = p.comparison {
        filter.filter.radius = 0;
    }
    let input =
        std::iter::from_fn(|| -> Option<(u64, DecodeRequest)> { panic!("invalid policy consumed input") });
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    assert!(matches!(
        scan_local_files_with_policy(input, p, 3, 3, &budget, || false),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn constrained_batch_mutation_discards_stale_pairs() {
    use std::io::Write;
    let (_folder, requests) = stable_triple();
    let path = requests[0].1.path.clone();
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let active = std::cell::Cell::new(false);
    let changed = std::cell::Cell::new(false);
    let report = rrrah_dedup::local_scan::scan_local_files_with_policy(
        requests,
        fitted_search(rrrah_dedup::warp::PhotometricFitMode::ConstrainedLeastSquares),
        3,
        3,
        &budget,
        || {
            if budget.used() > 0 {
                active.set(true);
            } else if active.get() && !changed.replace(true) {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
            }
            false
        },
    )
    .unwrap();
    assert!(changed.get());
    assert_eq!(report.source_issues.len(), 1);
    assert_eq!(report.source_issues[0].0, 1);
    assert!(matches!(
        report.source_issues[0].1,
        rrrah_dedup::exact::SnapshotError::Changed
    ));
    assert_eq!(report.pairs.len(), 1);
    assert_eq!((report.pairs[0].left, report.pairs[0].right), (2, 3));
    assert!(report.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
}

#[test]
fn constrained_batch_final_generation_and_cancellation_discard_report() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let (_folder, mut requests) = stable_triple();
    let generation = Arc::new(AtomicU64::new(7));
    for (_, request) in &mut requests {
        request.cancellation = Some(rrrah_decode::GenerationToken::new(Arc::clone(&generation), 7));
    }
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = std::cell::Cell::new(0_usize);
    let report = rrrah_dedup::local_scan::scan_local_files_with_policy(
        requests.clone(),
        fitted_search(rrrah_dedup::warp::PhotometricFitMode::ConstrainedLeastSquares),
        3,
        3,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(report.pairs.len(), 3);
    let stop = calls.get();
    calls.set(0);
    assert!(matches!(
        rrrah_dedup::local_scan::scan_local_files_with_policy(
            requests.clone(),
            fitted_search(rrrah_dedup::warp::PhotometricFitMode::ConstrainedLeastSquares),
            3,
            3,
            &budget,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    generation.store(8, Ordering::Release);
                }
                false
            }
        ),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    generation.store(7, Ordering::Release);
    calls.set(0);
    assert!(matches!(
        rrrah_dedup::local_scan::scan_local_files_with_policy(
            requests,
            fitted_search(rrrah_dedup::warp::PhotometricFitMode::ConstrainedLeastSquares),
            3,
            3,
            &budget,
            || {
                calls.set(calls.get() + 1);
                calls.get() == stop - 1
            }
        ),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn unfiltered_photometric_batch_uses_fitted_residuals_and_retains_strict_pixels() {
    use rrrah_dedup::local_scan::{LocalComparisonMode, scan_local_files_with_policy};
    let mut p = fitted_search(rrrah_dedup::warp::PhotometricFitMode::RejectOutsidePolicy);
    let LocalComparisonMode::Filtered { mut photometric, .. } = p.comparison else {
        panic!("fixture policy")
    };
    p.local.decode.max_pixels = 200_000;
    p.local.extract.max_pixels = 200_000;
    p.local.extract.max_candidates = 200_000;
    p.local.pixels.max_source_pixels = 200_000;
    photometric.residual = p.local.pixels;
    p.comparison = LocalComparisonMode::Photometric(photometric);
    let requests = [
        (1, DecodeRequest::new(root().join("photos/830-base.png"))),
        (2, DecodeRequest::new(root().join("photos/830-brightness.png"))),
    ];
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let report = scan_local_files_with_policy(requests, p, 2, 1, &budget, || false).unwrap();
    assert!(report.issues.is_empty());
    assert_eq!(report.pairs.len(), 1);
    let e = &report.pairs[0].evidence;
    assert!(e.candidate);
    assert!(e.photometric.is_some());
    assert!(e.filtered.is_none());
    let strict = e.pixels.as_ref().unwrap();
    assert!(strict.forward.matched_pixels * 10 < strict.forward.compared_pixels * 9);
    assert_eq!(budget.used(), 0);
}

#[test]
fn spatial_file_policy_preserves_pixel_proof_errors_and_source_guards() {
    use rrrah_dedup::local_scan::{SpatialFeaturePolicy, compare_local_files_spatial_with_policy};
    use std::{cell::Cell, io::Write};
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    std::fs::copy(root().join("rotation/base.png"), &a).unwrap();
    std::fs::copy(&a, &b).unwrap();
    let left = DecodeRequest::new(&a);
    let right = DecodeRequest::new(&b);
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 3,
        max_per_cell: 40,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let calls = Cell::new(0);
    let same = compare_local_files_spatial_with_policy(&left, &right, policy().into(), grid, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert!(same.candidate && same.geometry.is_some() && same.pixels.is_some());
    let last = calls.get();
    calls.set(0);
    assert!(matches!(
        compare_local_files_spatial_with_policy(&left, &right, policy().into(), grid, &budget, || {
            calls.set(calls.get() + 1);
            calls.get() == last
        }),
        Err(LocalFileError::Cancelled)
    ));
    let unrelated = DecodeRequest::new(root().join("rotation/unrelated.png"));
    let different =
        compare_local_files_spatial_with_policy(&left, &unrelated, policy().into(), grid, &budget, || false)
            .unwrap();
    assert!(!different.candidate);
    let missing = DecodeRequest::new("missing-spatial-file.png");
    assert!(matches!(
        compare_local_files_spatial_with_policy(
            &missing,
            &missing,
            policy().into(),
            SpatialFeaturePolicy { columns: 0, ..grid },
            &budget,
            || false
        ),
        Err(LocalFileError::InvalidPolicy)
    ));
    let changed = Cell::new(false);
    let result =
        compare_local_files_spatial_with_policy(&left, &right, policy().into(), grid, &budget, || {
            if !changed.get() && budget.used() > 0 {
                changed.set(true);
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&a)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
            }
            false
        });
    assert!(changed.get());
    assert!(
        matches!(
            result,
            Err(LocalFileError::Source(rrrah_dedup::exact::SnapshotError::Changed)
                | LocalFileError::Decode(rrrah_dedup::decode::CachedError::Source(
                    rrrah_dedup::exact::SnapshotError::Changed
                )))
        ),
        "wrong mutation evidence: {result:?}"
    );
    assert_eq!(budget.used(), 0);
}
