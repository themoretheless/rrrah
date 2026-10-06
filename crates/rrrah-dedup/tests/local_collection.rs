#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::GeometryPolicy,
    local::{LocalPolicy, MatchPolicy},
    local_collection::{LocalCollectionPolicy, scan_local_collection},
    local_index::FileFeatureBudgets,
    local_scan::{LocalFilePolicy, scan_local_files},
    warp::WarpPolicy,
};
fn policy() -> LocalCollectionPolicy {
    LocalCollectionPolicy {
        search: LocalFilePolicy {
            decode: AnimationBudget {
                max_frames: 10,
                max_pixels: 50_000,
                max_file_bytes: 1024 * 1024,
            },
            extract: LocalPolicy {
                max_pixels: 50_000,
                max_candidates: 50_000,
                max_features: 500,
                minimum_corner_score: 0.0001,
            },
            matching: MatchPolicy {
                max_comparisons: 250_000,
                max_distance: 64,
            },
            geometry: GeometryPolicy {
                tolerance: 2.0,
                min_inliers: 10,
                max_points: 500,
                max_hypotheses: 125_000,
            },
            pixels: WarpPolicy {
                tolerance: 0.03,
                max_source_pixels: 50_000,
            },
            minimum_compared_pixels: 1000,
            minimum_coverage_fraction: 0.3,
            minimum_matched_fraction: 0.9,
        }
        .into(),
        budgets: FileFeatureBudgets {
            max_files: 5,
            max_features: 2500,
            max_hits: 6_250_000,
            max_pair_counts: 10,
            max_pairs: 10,
        },
    }
}
fn files() -> Vec<(u64, DecodeRequest)> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    [
        (1, "rotation/base.png"),
        (2, "local-files/crop.png"),
        (3, "rotation/scale-216-angle-17.png"),
        (4, "rotation/unrelated.png"),
    ]
    .map(|(id, path)| (id, DecodeRequest::new(root.join(path))))
    .to_vec()
}
#[test]
fn indexed_collection_retains_every_exhaustively_verified_crop_scale_rotation_pair() {
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let p = policy();
    let reference = scan_local_files(files(), p.search.local, 4, 6, &budget, || false).unwrap();
    let indexed = scan_local_collection(files(), p, &budget, || false).unwrap();
    let expected = reference
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<Vec<_>>();
    let actual = indexed
        .local
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<Vec<_>>();
    assert_eq!(expected, [(1, 2), (1, 3), (2, 3)]);
    assert_eq!(actual, expected);
    assert!(indexed.file_issues.is_empty());
    assert!(indexed.local.issues.is_empty());
    assert!(indexed.local.source_issues.is_empty());
    assert_eq!(indexed.analysed, [1, 2, 3, 4]);
    println!(
        "features={} hits={} proposed={} pixel_verifications={} exhaustive={}",
        indexed.indexed_features,
        indexed.descriptor_hits,
        indexed.proposed_pairs,
        indexed.pixel_verification_pairs,
        reference.pairs.len()
    );
    assert!(indexed.proposed_pairs <= 6);
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_arbitrary_angles_and_intermediate_scales_keep_labelled_pairs() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for derivative in [
        "angle-17.png",
        "angle--37.png",
        "angle-63.png",
        "scale-216-angle-0.png",
        "scale-117-angle-0.png",
        "scale-216-angle-17.png",
    ] {
        let requests = || {
            [(1, "base.png"), (2, derivative), (3, "unrelated.png")]
                .map(|(id, name)| (id, DecodeRequest::new(root.join(name))))
                .to_vec()
        };
        let p = policy();
        let exhaustive = scan_local_files(requests(), p.search.local, 3, 3, &budget, || false).unwrap();
        let indexed = scan_local_collection(requests(), p, &budget, || false).unwrap();
        let accepted = |pairs: &[rrrah_dedup::local_scan::LocalPair]| {
            pairs
                .iter()
                .filter(|pair| pair.evidence.candidate)
                .map(|pair| (pair.left, pair.right))
                .collect::<Vec<_>>()
        };
        assert_eq!(accepted(&exhaustive.pairs), [(1, 2)], "{derivative}");
        assert_eq!(accepted(&indexed.local.pairs), [(1, 2)], "{derivative}");
        assert!(indexed.file_issues.is_empty(), "{derivative}");
        assert!(indexed.local.issues.is_empty(), "{derivative}");
        assert!(indexed.local.source_issues.is_empty(), "{derivative}");
        assert_eq!(budget.used(), 0, "{derivative}");
    }
}
#[test]
fn indexed_watermark_and_large_edit_preserve_pixel_policy() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for (derivative, expected) in [
        ("../edits/small-watermark.png", vec![(1, 2)]),
        ("../edits/large-edit.png", vec![]),
    ] {
        let requests = || {
            [(1, "base.png"), (2, derivative), (3, "unrelated.png")]
                .map(|(id, name)| (id, DecodeRequest::new(root.join(name))))
                .to_vec()
        };
        let p = policy();
        let exhaustive = scan_local_files(requests(), p.search.local, 3, 3, &budget, || false).unwrap();
        let indexed = scan_local_collection(requests(), p, &budget, || false).unwrap();
        let accepted = |pairs: &[rrrah_dedup::local_scan::LocalPair]| {
            pairs
                .iter()
                .filter(|pair| pair.evidence.candidate)
                .map(|pair| (pair.left, pair.right))
                .collect::<Vec<_>>()
        };
        assert_eq!(accepted(&exhaustive.pairs), expected, "{derivative}");
        assert_eq!(accepted(&indexed.local.pairs), expected, "{derivative}");
        for pairs in [&exhaustive.pairs, &indexed.local.pairs] {
            let pair = pairs
                .iter()
                .find(|pair| (pair.left, pair.right) == (1, 2))
                .expect("edited pair must reach pixel confirmation");
            assert!(pair.evidence.geometry.is_some(), "{derivative}");
            assert!(pair.evidence.pixels.is_some(), "{derivative}");
        }
        assert!(indexed.file_issues.is_empty(), "{derivative}");
        assert!(indexed.local.issues.is_empty(), "{derivative}");
        assert!(indexed.local.source_issues.is_empty(), "{derivative}");
        assert_eq!(budget.used(), 0, "{derivative}");
    }
}
#[test]
fn indexed_collection_reports_bad_and_insufficient_sources_and_admission_errors() {
    let folder = tempfile::tempdir().unwrap();
    let bad = folder.path().join("bad.png");
    std::fs::write(&bad, b"bad image").unwrap();
    let mut requests = files();
    requests.push((5, DecodeRequest::new(bad)));
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let report = scan_local_collection(requests, policy(), &budget, || false).unwrap();
    assert_eq!(report.file_issues.len(), 1);
    assert_eq!(report.file_issues[0].0, 5);
    assert!(report.local.source_issues.is_empty());
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/information");
    let flat = [
        (1, DecodeRequest::new(root.join("black.png"))),
        (2, DecodeRequest::new(root.join("white.png"))),
    ];
    let report = scan_local_collection(flat, policy(), &budget, || false).unwrap();
    assert_eq!(report.insufficient_features, [1, 2]);
    assert!(report.local.pairs.is_empty());
    assert!(report.file_issues.is_empty());
    let mut invalid = policy();
    invalid.search.local.minimum_coverage_fraction = 2.0;
    let input =
        std::iter::from_fn(|| -> Option<(u64, DecodeRequest)> { panic!("invalid policy consumed input") });
    assert!(matches!(
        scan_local_collection(input, invalid, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::InvalidPolicy)
    ));
    let mut tiny = policy();
    tiny.budgets.max_features = 0;
    assert!(matches!(
        scan_local_collection(files(), tiny, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
}

fn stable_triple() -> (tempfile::TempDir, Vec<(u64, DecodeRequest)>) {
    let folder = tempfile::tempdir().unwrap();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let files = ["base.png", "base.png", "scale-216-angle-17.png"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let id = u64::try_from(i).unwrap() + 1;
            let path = folder.path().join(format!("{id}.png"));
            std::fs::copy(root.join(name), &path).unwrap();
            (id, DecodeRequest::new(path))
        })
        .collect();
    (folder, files)
}
#[test]
fn indexed_collection_changed_before_decode_is_invalidated_without_losing_healthy_pair() {
    use std::io::Write;
    let (_folder, requests) = stable_triple();
    let path = requests[1].1.path.clone();
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let changed = std::cell::Cell::new(false);
    let report = scan_local_collection(requests, policy(), &budget, || {
        // Initial snapshots precede decoding; mutate the second source during
        // the first decode, without relying on retained storage reaching zero.
        if budget.used() > 0 && !changed.replace(true) {
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
    assert_eq!(report.analysed, [1, 3]);
    assert_eq!(report.local.source_issues.len(), 1);
    assert_eq!(report.local.source_issues[0].0, 2);
    assert_eq!(report.local.pairs.len(), 1);
    assert_eq!((report.local.pairs[0].left, report.local.pairs[0].right), (1, 3));
    assert!(report.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
}
#[test]
fn indexed_collection_final_generation_and_one_shot_cancel_discard_report() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let (_folder, mut requests) = stable_triple();
    let generation = Arc::new(AtomicU64::new(7));
    for (_, request) in &mut requests {
        request.cancellation = Some(rrrah_decode::GenerationToken::new(Arc::clone(&generation), 7));
    }
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0usize);
    let report = scan_local_collection(requests.clone(), policy(), &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(report.local.pairs.len(), 3);
    let last = calls.get();
    calls.set(0);
    assert!(matches!(
        scan_local_collection(requests.clone(), policy(), &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == last {
                generation.store(8, Ordering::Release);
            }
            false
        }),
        Err(rrrah_dedup::scan::ScanError::Cancelled)
    ));
    generation.store(7, Ordering::Release);
    calls.set(0);
    assert!(matches!(
        scan_local_collection(requests, policy(), &budget, || {
            calls.set(calls.get() + 1);
            calls.get() == last - 1
        }),
        Err(rrrah_dedup::scan::ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn recursive_collection_preserves_aliases_errors_and_request_settings() {
    use rrrah_dedup::local_collection::{
        scan_local_collection_roots, scan_local_collection_roots_with_requests,
    };
    use rrrah_dedup::scan::ScanError;
    let folder = tempfile::tempdir().unwrap();
    let nested = folder.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let fixture =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/information/black.png");
    std::fs::copy(&fixture, nested.join("a.png")).unwrap();
    std::fs::write(folder.path().join("bad.png"), b"invalid image").unwrap();
    #[cfg(unix)]
    std::fs::hard_link(nested.join("a.png"), folder.path().join("alias.png")).unwrap();
    let roots = [folder.path().to_path_buf(), nested];
    let traversal = rrrah_dedup::exact::Options::default();
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let normal = scan_local_collection_roots(&roots, &traversal, policy(), &budget, || false).unwrap();
    assert_eq!(normal.files.len(), 2);
    assert_eq!(normal.indexed.file_issues.len(), 1);
    assert_eq!(normal.indexed.insufficient_features.len(), 1);
    assert!(normal.indexed.local.pairs.is_empty());
    #[cfg(unix)]
    assert!(!normal.aliases.is_empty());
    let unavailable = scan_local_collection_roots_with_requests(
        &roots,
        &traversal,
        policy(),
        &budget,
        |path| {
            let mut r = DecodeRequest::new(path);
            r.image_index = 1;
            r
        },
        || false,
    )
    .unwrap();
    assert!(unavailable.indexed.analysed.is_empty());
    assert_eq!(unavailable.indexed.file_issues.len(), 2);
    assert!(matches!(
        scan_local_collection_roots_with_requests(
            &roots,
            &traversal,
            policy(),
            &budget,
            |_| DecodeRequest::new("substituted.png"),
            || false,
        ),
        Err(ScanError::InvalidPolicy)
    ));
    let mut invalid = policy();
    invalid.search.local.minimum_coverage_fraction = 2.0;
    assert!(matches!(
        scan_local_collection_roots_with_requests(
            &roots,
            &traversal,
            invalid,
            &budget,
            |_| panic!("invalid policy must precede factory"),
            || false,
        ),
        Err(ScanError::InvalidPolicy)
    ));
    let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2));
    assert!(matches!(
        scan_local_collection_roots_with_requests(
            &roots,
            &traversal,
            policy(),
            &budget,
            |path| {
                let mut r = DecodeRequest::new(path);
                r.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
                r
            },
            || false,
        ),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn spatial_collection_matches_exhaustive_spatial_pair_acceptance() {
    use rrrah_dedup::{
        local_collection::scan_local_collection_spatial,
        local_scan::{SpatialFeaturePolicy, compare_local_files_spatial_with_policy},
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let p = policy();
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 3,
        max_per_cell: 40,
    };
    let requests = files();
    let mut expected = Vec::new();
    for (i, (left, a)) in requests.iter().enumerate() {
        for (right, b) in &requests[i + 1..] {
            let result =
                compare_local_files_spatial_with_policy(a, b, p.search, grid, &budget, || false).unwrap();
            if result.candidate {
                expected.push((*left, *right));
            }
        }
    }
    assert!(!expected.is_empty());
    let indexed = scan_local_collection_spatial(requests, p, grid, &budget, || false).unwrap();
    let actual = indexed
        .local
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert!(actual.iter().all(|&(a, b)| a != 4 && b != 4));
    assert!(indexed.file_issues.is_empty());
    assert!(indexed.local.issues.is_empty());
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        scan_local_collection_spatial(files(), p, grid, &budget, || true),
        Err(rrrah_dedup::scan::ScanError::Cancelled)
    ));
    let invalid = SpatialFeaturePolicy { columns: 0, ..grid };
    let never = std::iter::from_fn(|| -> Option<(u64, DecodeRequest)> {
        panic!("invalid grid must reject before input consumption")
    });
    assert!(matches!(
        scan_local_collection_spatial(never, p, invalid, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::InvalidPolicy)
    ));
}

#[test]
fn recursive_spatial_collection_preserves_requests_and_rejects_substitution() {
    use rrrah_dedup::{
        local_collection::scan_local_collection_roots_spatial_with_requests, local_scan::SpatialFeaturePolicy,
    };
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let source = &files()[0].1.path;
    std::fs::copy(source, dir.path().join("a.png")).unwrap();
    std::fs::copy(source, nested.join("b.png")).unwrap();
    std::fs::write(nested.join("bad.png"), b"broken").unwrap();
    let roots = [dir.path().to_path_buf()];
    let traversal = rrrah_dedup::exact::Options::default();
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 3,
        max_per_cell: 40,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let report = scan_local_collection_roots_spatial_with_requests(
        &roots,
        &traversal,
        policy(),
        grid,
        &budget,
        |path| {
            let mut r = DecodeRequest::new(path);
            r.assume_untagged_srgb = true;
            r
        },
        || false,
    )
    .unwrap();
    assert_eq!(report.files.len(), 3);
    assert_eq!(report.indexed.file_issues.len(), 1);
    assert_eq!(
        report
            .indexed
            .local
            .pairs
            .iter()
            .filter(|p| p.evidence.candidate)
            .count(),
        1
    );
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        scan_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            policy(),
            grid,
            &budget,
            |_| DecodeRequest::new("different.png"),
            || false
        ),
        Err(rrrah_dedup::scan::ScanError::InvalidPolicy)
    ));
    let invalid = SpatialFeaturePolicy { columns: 0, ..grid };
    assert!(matches!(
        scan_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            policy(),
            invalid,
            &budget,
            |_| panic!("invalid grid before factory"),
            || false
        ),
        Err(rrrah_dedup::scan::ScanError::InvalidPolicy)
    ));
}

#[test]
fn ranged_fit_collection_matches_pair_results_and_exposes_selected_domain() {
    check_fit_collection(false);
}

#[test]
fn display_projection_collection_matches_pairs_and_preserves_strict_evidence() {
    check_fit_collection(true);
}

fn check_fit_collection(display_projection: bool) {
    use rrrah_dedup::{
        local_scan::{LocalComparisonMode, compare_local_files_with_policy},
        warp::{FitSampleRange, PhotometricPolicy},
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut p = policy();
    let range = FitSampleRange {
        minimum: 0.0,
        maximum: 1.0,
    };
    let photo = PhotometricPolicy {
        residual: p.search.local.pixels,
        minimum_samples: 1000,
        minimum_variance: 1e-5,
        minimum_gain: 0.2,
        maximum_gain: 5.0,
        maximum_offset: 0.1,
    };
    p.search.comparison = if display_projection {
        LocalComparisonMode::DisplayProjection {
            photometric: photo,
            range,
        }
    } else {
        LocalComparisonMode::RangePhotometric {
            photometric: photo,
            range,
        }
    };
    let requests = files();
    let mut expected = Vec::new();
    for (i, (left, a)) in requests.iter().enumerate() {
        for (right, b) in &requests[i + 1..] {
            let result = compare_local_files_with_policy(a, b, p.search, &budget, || false).unwrap();
            assert_eq!(result.photometric_fit_range, Some(range));
            assert_eq!(result.display_projection, display_projection);
            if result.candidate {
                assert!(result.pixels.is_some());
                assert!(result.photometric.is_some());
                expected.push((*left, *right));
            }
        }
    }
    assert!(!expected.is_empty());
    let report = scan_local_collection(requests, p, &budget, || false).unwrap();
    let actual = report
        .local
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert!(
        report
            .local
            .pairs
            .iter()
            .all(|p| p.evidence.photometric_fit_range == Some(range)
                && p.evidence.display_projection == display_projection)
    );
    assert!(report.local.issues.is_empty());
    assert_eq!(budget.used(), 0);
    let invalid = FitSampleRange {
        minimum: 1.0,
        maximum: 0.0,
    };
    p.search.comparison = if display_projection {
        LocalComparisonMode::DisplayProjection {
            photometric: photo,
            range: invalid,
        }
    } else {
        LocalComparisonMode::RangePhotometric {
            photometric: photo,
            range: invalid,
        }
    };
    let never =
        std::iter::from_fn(|| -> Option<(u64, DecodeRequest)> { panic!("invalid range before consumption") });
    assert!(matches!(
        scan_local_collection(never, p, &budget, || false),
        Err(rrrah_dedup::scan::ScanError::InvalidPolicy)
    ));
}

#[test]
fn reflected_file_pair_confirms_pixels_and_releases_managed_features() {
    use rrrah_dedup::local_scan::{LocalFileError, compare_reflected_local_files};
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let base = DecodeRequest::new(root.join("base.png"));
    let mirror = DecodeRequest::new(root.join("mirrored.png"));
    let unrelated = DecodeRequest::new(root.join("unrelated.png"));
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for (left, right) in [(&base, &mirror), (&mirror, &base)] {
        let calls = std::cell::Cell::new(0);
        let result = compare_reflected_local_files(left, right, policy().search.local, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        assert!(result.candidate);
        assert!(result.geometry.is_some());
        let pixels = result.pixels.unwrap();
        assert_eq!(pixels.forward.matched_pixels, 25600);
        assert_eq!(pixels.reverse.matched_pixels, 25600);
        assert_eq!(budget.used(), 0);
        let current = std::cell::Cell::new(0);
        assert!(matches!(
            compare_reflected_local_files(left, right, policy().search.local, &budget, || {
                current.set(current.get() + 1);
                current.get() == calls.get()
            }),
            Err(LocalFileError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    assert!(
        !compare_reflected_local_files(&base, &unrelated, policy().search.local, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
    assert!(
        compare_reflected_local_files(
            &base,
            &mirror,
            policy().search.local,
            &MemoryBudget::new(0),
            || false
        )
        .is_err()
    );
}

#[test]
fn reflected_collection_retrieves_labelled_pairs_and_cancels_without_report() {
    use rrrah_dedup::{
        local_collection::scan_reflected_local_collection, local_scan::compare_reflected_local_files,
        scan::ScanError,
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let requests = || {
        [(1, "base.png"), (2, "mirrored.png"), (3, "unrelated.png")]
            .map(|(id, name)| (id, DecodeRequest::new(root.join(name))))
            .to_vec()
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut p = policy();
    p.budgets.max_features = 5000;
    let mut expected = Vec::new();
    let files = requests();
    for i in 0..3 {
        for j in i + 1..3 {
            if compare_reflected_local_files(&files[i].1, &files[j].1, p.search.local, &budget, || false)
                .unwrap()
                .candidate
            {
                expected.push((files[i].0, files[j].0));
            }
        }
    }
    assert_eq!(expected, [(1, 2)]);
    let calls = std::cell::Cell::new(0);
    let report = scan_reflected_local_collection(requests(), p, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let actual = report
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    let mut reversed = requests();
    reversed.reverse();
    let reordered = scan_reflected_local_collection(reversed, p, &budget, || false).unwrap();
    assert_eq!(reordered.analysed, report.analysed);
    assert_eq!(
        reordered
            .pairs
            .iter()
            .filter(|pair| pair.evidence.candidate)
            .map(|pair| (pair.left, pair.right))
            .collect::<Vec<_>>(),
        expected
    );

    assert_eq!(report.analysed, [1, 2, 3]);
    assert!(report.file_issues.is_empty() && report.issues.is_empty() && report.source_issues.is_empty());
    assert!(report.indexed_features > 0);
    assert_eq!(budget.used(), 0);
    let current = std::cell::Cell::new(0);
    assert!(matches!(
        scan_reflected_local_collection(requests(), p, &budget, || {
            current.set(current.get() + 1);
            current.get() == calls.get()
        }),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    p.budgets.max_features = 1;
    assert!(matches!(
        scan_reflected_local_collection(requests(), p, &budget, || false),
        Err(ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn reflected_collection_discards_changed_sources_batch_wide() {
    use rrrah_dedup::local_collection::scan_reflected_local_collection;
    use std::io::Write;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let folder = tempfile::tempdir().unwrap();
    let changed = folder.path().join("base.png");
    std::fs::copy(root.join("base.png"), &changed).unwrap();
    let requests = [
        (1, DecodeRequest::new(&changed)),
        (2, DecodeRequest::new(root.join("mirrored.png"))),
    ];
    let calls = std::cell::Cell::new(0);
    let mutated = std::cell::Cell::new(false);
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut p = policy();
    p.budgets.max_features = 5000;
    let report = scan_reflected_local_collection(requests, p, &budget, || {
        calls.set(calls.get() + 1);
        if calls.get() == 5000 {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&changed)
                .unwrap()
                .write_all(b"changed-source")
                .unwrap();
            mutated.set(true);
        }
        false
    })
    .unwrap();
    assert!(mutated.get());
    assert!(!report.analysed.contains(&1));
    assert!(report.source_issues.iter().any(|(id, _)| *id == 1));
    assert!(report.pairs.iter().all(|pair| pair.left != 1 && pair.right != 1));
    assert_eq!(budget.used(), 0);
}

#[test]
fn reflected_recursive_collection_keeps_aliases_paths_and_request_policy() {
    use rrrah_dedup::{
        local_collection::{
            scan_reflected_local_collection_roots, scan_reflected_local_collection_roots_with_requests,
        },
        scan::ScanError,
    };
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let folder = tempfile::tempdir().unwrap();
    let nested = folder.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::copy(fixture.join("base.png"), nested.join("base.png")).unwrap();
    std::fs::copy(fixture.join("mirrored.png"), nested.join("mirror.png")).unwrap();
    std::fs::copy(fixture.join("unrelated.png"), folder.path().join("other.png")).unwrap();
    std::fs::write(folder.path().join("bad.png"), b"corrupt image").unwrap();
    #[cfg(unix)]
    {
        std::fs::hard_link(nested.join("base.png"), folder.path().join("alias.png")).unwrap();
        std::os::unix::fs::symlink(folder.path(), nested.join("loop")).unwrap();
    }
    let roots = [folder.path().to_path_buf(), nested];
    let traversal = rrrah_dedup::exact::Options {
        follow_symlinks: true,
        ..Default::default()
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut p = policy();
    p.budgets.max_features = 5000;
    let report = scan_reflected_local_collection_roots(&roots, &traversal, p, &budget, || false).unwrap();
    assert_eq!(report.files.len(), 4);
    assert_eq!(
        report
            .indexed
            .pairs
            .iter()
            .filter(|p| p.evidence.candidate)
            .count(),
        1
    );
    assert_eq!(report.indexed.file_issues.len(), 1);
    assert!(report.indexed.issues.is_empty() && report.indexed.source_issues.is_empty());
    #[cfg(unix)]
    assert!(!report.aliases.is_empty());
    assert_eq!(budget.used(), 0);
    let grid = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 20,
    };
    let spatial = rrrah_dedup::local_collection::scan_reflected_local_collection_roots_spatial(
        &roots,
        &traversal,
        p,
        grid,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(spatial.files.len(), 4);
    assert_eq!(
        spatial
            .indexed
            .pairs
            .iter()
            .filter(|pair| pair.evidence.candidate)
            .count(),
        1
    );
    assert_eq!(spatial.indexed.file_issues.len(), 1);
    #[cfg(unix)]
    assert!(!spatial.aliases.is_empty());
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        rrrah_dedup::local_collection::scan_reflected_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            p,
            rrrah_dedup::local_scan::SpatialFeaturePolicy { columns: 0, ..grid },
            &budget,
            |_| panic!("invalid grid before factory"),
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));

    let unavailable = scan_reflected_local_collection_roots_with_requests(
        &roots,
        &traversal,
        p,
        &budget,
        |path| {
            let mut request = DecodeRequest::new(path);
            request.image_index = 1;
            request
        },
        || false,
    )
    .unwrap();
    assert!(unavailable.indexed.analysed.is_empty());
    assert_eq!(unavailable.indexed.file_issues.len(), 4);
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        scan_reflected_local_collection_roots_with_requests(
            &roots,
            &traversal,
            p,
            &budget,
            |_| DecodeRequest::new("substituted.png"),
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert!(matches!(
        scan_reflected_local_collection_roots(&roots, &traversal, p, &budget, || true),
        Err(ScanError::Cancelled)
    ));
    p.budgets.max_files = 1;
    assert!(matches!(
        scan_reflected_local_collection_roots(&roots, &traversal, p, &budget, || false),
        Err(ScanError::Budget)
    ));
}

#[test]
fn reflected_spatial_collection_matches_spatial_file_oracle() {
    use rrrah_dedup::{
        local_collection::scan_reflected_local_collection_spatial,
        local_scan::{SpatialFeaturePolicy, compare_reflected_local_files_spatial},
        scan::ScanError,
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let files = [(1, "base.png"), (2, "mirrored.png"), (3, "unrelated.png")]
        .map(|(id, name)| (id, DecodeRequest::new(root.join(name))))
        .to_vec();
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 20,
    };
    let mut p = policy();
    p.budgets.max_features = 5000;
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut expected = Vec::new();
    for i in 0..3 {
        for j in i + 1..3 {
            let evidence = compare_reflected_local_files_spatial(
                &files[i].1,
                &files[j].1,
                p.search.local,
                grid,
                &budget,
                || false,
            )
            .unwrap();
            if evidence.candidate {
                expected.push((files[i].0, files[j].0));
            }
        }
    }
    assert_eq!(expected, [(1, 2)]);
    {
        let left = rrrah_dedup::decode::decode_selected_frame(
            &files[0].1,
            p.search.local.extract.max_pixels,
            &budget,
            || false,
        )
        .unwrap();
        let right = rrrah_dedup::decode::decode_selected_frame(
            &files[1].1,
            p.search.local.extract.max_pixels,
            &budget,
            || false,
        )
        .unwrap();
        let left_view = left.view(|| false).unwrap();
        let right_view = right.view(|| false).unwrap();
        let lf = rrrah_dedup::local::extract_spatial_oriented(
            &left_view,
            p.search.local.extract,
            grid.columns,
            grid.rows,
            grid.max_per_cell,
            || false,
        )
        .unwrap();
        let rf = rrrah_dedup::local::extract_reflected_spatial_oriented(
            &right_view,
            p.search.local.extract,
            grid.columns,
            grid.rows,
            grid.max_per_cell,
            || false,
        )
        .unwrap();
        let reference =
            rrrah_dedup::local::match_features(&lf, &rf, p.search.local.matching, || false).unwrap();
        let uncapped = rrrah_dedup::local::extract_reflected_multiscale_oriented(
            &right_view,
            p.search.local.extract,
            || false,
        )
        .unwrap();
        let asymmetric =
            rrrah_dedup::local::match_features(&lf, &uncapped, p.search.local.matching, || false).unwrap();
        assert_ne!(
            reference.iter().map(|p| (p.source, p.target)).collect::<Vec<_>>(),
            asymmetric
                .iter()
                .map(|p| (p.source, p.target))
                .collect::<Vec<_>>(),
            "fixture must expose the former uncapped-right selection"
        );
        let actual = compare_reflected_local_files_spatial(
            &files[0].1,
            &files[1].1,
            p.search.local,
            grid,
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(
            actual
                .correspondences
                .iter()
                .map(|p| (p.source, p.target))
                .collect::<Vec<_>>(),
            reference.iter().map(|p| (p.source, p.target)).collect::<Vec<_>>()
        );
    }
    assert_eq!(budget.used(), 0);

    let report = scan_reflected_local_collection_spatial(files, p, grid, &budget, || false).unwrap();
    assert_eq!(
        report
            .pairs
            .iter()
            .filter(|pair| pair.evidence.candidate)
            .map(|pair| (pair.left, pair.right))
            .collect::<Vec<_>>(),
        expected
    );
    assert!(report.file_issues.is_empty() && report.issues.is_empty() && report.source_issues.is_empty());
    assert!(report.indexed_features <= 3 * 2 * 16 * 20);
    assert_eq!(budget.used(), 0);
    let unavailable = std::iter::from_fn(|| -> Option<(u64, DecodeRequest)> {
        panic!("invalid grid must refuse before admission")
    });
    assert!(matches!(
        scan_reflected_local_collection_spatial(
            unavailable,
            p,
            SpatialFeaturePolicy { columns: 0, ..grid },
            &budget,
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
}

#[test]
fn reflected_ranged_modes_validate_before_consuming_sources() {
    use rrrah_dedup::{
        local_collection::scan_reflected_local_collection,
        local_scan::LocalComparisonMode,
        warp::{FitSampleRange, PhotometricPolicy},
    };
    let mut p = policy();
    let fit = PhotometricPolicy {
        residual: p.search.local.pixels,
        minimum_samples: 1000,
        minimum_variance: 1e-5,
        minimum_gain: 0.2,
        maximum_gain: 5.,
        maximum_offset: 0.1,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for display in [false, true] {
        for range in [
            FitSampleRange {
                minimum: 1.,
                maximum: 0.,
            },
            FitSampleRange {
                minimum: f32::NAN.into(),
                maximum: 1.,
            },
        ] {
            p.search.comparison = if display {
                LocalComparisonMode::DisplayProjection {
                    photometric: fit,
                    range,
                }
            } else {
                LocalComparisonMode::RangePhotometric {
                    photometric: fit,
                    range,
                }
            };
            let never = std::iter::from_fn(|| -> Option<(u64, DecodeRequest)> {
                panic!("invalid reflected range must precede iterator consumption")
            });
            assert!(matches!(
                scan_reflected_local_collection(never, p, &budget, || false),
                Err(rrrah_dedup::scan::ScanError::InvalidPolicy)
            ));
            assert_eq!(budget.used(), 0);
        }
    }
}
