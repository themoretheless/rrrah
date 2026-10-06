#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::GeometryPolicy,
    local::{LocalPolicy, MatchPolicy},
    local_scan::{
        LocalFilePolicy, compare_local_files, compare_local_files_filtered, compare_local_files_photometric,
        compare_local_files_with_color_filter,
    },
    warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricPolicy, WarpPolicy},
};
use std::path::PathBuf;
fn policy() -> LocalFilePolicy {
    LocalFilePolicy {
        decode: AnimationBudget {
            max_frames: 10,
            max_pixels: 200_000,
            max_file_bytes: 1024 * 1024,
        },
        extract: LocalPolicy {
            max_pixels: 200_000,
            max_candidates: 200_000,
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
            max_source_pixels: 200_000,
        },
        minimum_compared_pixels: 1000,
        minimum_coverage_fraction: 0.3,
        minimum_matched_fraction: 0.9,
    }
}
fn photometric_policy() -> PhotometricPolicy {
    PhotometricPolicy {
        residual: policy().pixels,
        minimum_samples: 1000,
        minimum_variance: 1e-5,
        minimum_gain: 0.2,
        maximum_gain: 5.,
        maximum_offset: 0.1,
    }
}

#[test]
fn real_camera_preview_brightness_is_a_visual_candidate_with_strict_residuals_retained() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for id in [830, 898, 1084, 1294] {
        let a = DecodeRequest::new(root.join(format!("{id}-base.png")));
        let b = DecodeRequest::new(root.join(format!("{id}-brightness.png")));
        let strict = compare_local_files(&a, &b, policy(), &budget, || false).unwrap();
        assert!(!strict.candidate);
        assert!(strict.photometric.is_none());
        let fitted =
            compare_local_files_photometric(&a, &b, policy(), photometric_policy(), &budget, || false)
                .unwrap();
        assert!(fitted.candidate);
        assert_eq!(fitted.pixels, strict.pixels);
        let photo = fitted.photometric.unwrap();
        for p in [&photo.forward, &photo.reverse] {
            assert_eq!(p.pixels.matched_pixels, p.pixels.compared_pixels);
            assert!(p.fitted_samples >= 1000);
        }
        assert_eq!(budget.used(), 0);
    }
    for (a, b) in [
        (830, 898),
        (830, 1084),
        (830, 1294),
        (898, 1084),
        (898, 1294),
        (1084, 1294),
    ] {
        let e = compare_local_files_photometric(
            &DecodeRequest::new(root.join(format!("{a}-base.png"))),
            &DecodeRequest::new(root.join(format!("{b}-base.png"))),
            policy(),
            photometric_policy(),
            &budget,
            || false,
        )
        .unwrap();
        assert!(!e.candidate);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn invalid_fitting_policy_is_rejected_before_source_access() {
    let request = DecodeRequest::new("absent-photo.png");
    let budget = MemoryBudget::new(1);
    let mut invalid = photometric_policy();
    invalid.maximum_gain = f64::NAN;
    assert!(matches!(
        compare_local_files_photometric(&request, &request, policy(), invalid, &budget, || false),
        Err(rrrah_dedup::local_scan::LocalFileError::Pixels(
            rrrah_dedup::warp::WarpError::Invalid
        ))
    ));
    assert_eq!(budget.peak(), 0);
}

#[test]
fn real_photo_resize_crop_rotation_and_brightness_have_filtered_evidence() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for id in [830, 898, 1084, 1294] {
        for variant in ["resize", "crop", "rotate", "brightness"] {
            let e = compare_local_files_filtered(
                &DecodeRequest::new(root.join(format!("{id}-base.png"))),
                &DecodeRequest::new(root.join(format!("{id}-{variant}.png"))),
                policy(),
                photometric_policy(),
                FilterPolicy {
                    radius: 2,
                    max_sample_pairs: 20_000_000,
                },
                &budget,
                || false,
            )
            .unwrap();
            assert!(e.candidate, "{id} {variant}");
            assert!(e.pixels.is_some() && e.photometric.is_some());
            let filtered = e.filtered.unwrap();
            assert_eq!(filtered.filter.radius, 2);
            for p in [&filtered.evidence.forward, &filtered.evidence.reverse] {
                assert!(p.pixels.compared_pixels < p.pixels.source_pixels);
            }
            assert_eq!(budget.used(), 0);
        }
    }
}
#[test]
fn filtered_natural_photo_jpegs_are_candidates_and_other_scenes_remain_rejected() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    // The color-swatch JPEG (1084) is still a documented miss in the observational
    // 20-positive report; this test qualifies the three other JPEG fixtures.
    for id in [830, 898, 1294] {
        let e = compare_local_files_filtered(
            &DecodeRequest::new(root.join(format!("{id}-base.png"))),
            &DecodeRequest::new(root.join(format!("{id}-jpeg.jpg"))),
            policy(),
            photometric_policy(),
            FilterPolicy {
                radius: 2,
                max_sample_pairs: 20_000_000,
            },
            &budget,
            || false,
        )
        .unwrap();
        assert!(e.candidate, "{id} jpeg");
        assert!(e.filtered.is_some());
        assert_eq!(budget.used(), 0);
    }
    for (a, b) in [
        (830, 898),
        (830, 1084),
        (830, 1294),
        (898, 1084),
        (898, 1294),
        (1084, 1294),
    ] {
        let e = compare_local_files_filtered(
            &DecodeRequest::new(root.join(format!("{a}-base.png"))),
            &DecodeRequest::new(root.join(format!("{b}-base.png"))),
            policy(),
            photometric_policy(),
            FilterPolicy {
                radius: 2,
                max_sample_pairs: 20_000_000,
            },
            &budget,
            || false,
        )
        .unwrap();
        assert!(!e.candidate);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn filtered_file_evidence_is_discarded_on_late_cancellation_generation_or_mutation() {
    use rrrah_dedup::local_scan::LocalFileError;
    use std::{
        cell::Cell,
        io::Write,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };
    let dir = tempfile::tempdir().unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png");
    let left = dir.path().join("left.png");
    let right = dir.path().join("right.png");
    std::fs::copy(&source, &left).unwrap();
    std::fs::copy(source, &right).unwrap();
    let mut a = DecodeRequest::new(&left);
    let b = DecodeRequest::new(right);
    let generation = Arc::new(AtomicU64::new(7));
    a.cancellation = Some(rrrah_decode::GenerationToken::new(Arc::clone(&generation), 7));
    let mut p = policy();
    p.extract.max_features = 100;
    let filter = FilterPolicy {
        radius: 2,
        max_sample_pairs: 20_000_000,
    };
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0_usize);
    let result = compare_local_files_filtered(&a, &b, p, photometric_policy(), filter, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert!(result.candidate);
    assert_eq!(budget.used(), 0);
    let stop = calls.get();
    assert!(stop > 20);
    calls.set(0);
    assert!(matches!(
        compare_local_files_filtered(&a, &b, p, photometric_policy(), filter, &budget, || {
            calls.set(calls.get() + 1);
            calls.get() == stop - 1
        }),
        Err(LocalFileError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    calls.set(0);
    assert!(matches!(
        compare_local_files_filtered(&a, &b, p, photometric_policy(), filter, &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                generation.store(8, Ordering::Release);
            }
            false
        }),
        Err(LocalFileError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    generation.store(7, Ordering::Release);
    calls.set(0);
    let mutated = Cell::new(false);
    assert!(matches!(
        compare_local_files_filtered(&a, &b, p, photometric_policy(), filter, &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == stop - 20 {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&left)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                mutated.set(true);
            }
            false
        }),
        Err(LocalFileError::Source(rrrah_dedup::exact::SnapshotError::Changed))
    ));
    assert!(mutated.get());
    assert_eq!(budget.used(), 0);
}

#[test]
fn explicit_seven_by_seven_windows_cover_all_twenty_photo_derivatives_in_both_spaces() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for space in [FilterColorSpace::LinearSrgb, FilterColorSpace::EncodedSrgb] {
        let filter = ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 3,
                max_sample_pairs: 20_000_000,
            },
            color_space: space,
        };
        for id in [830, 898, 1084, 1294] {
            for variant in ["resize", "crop", "rotate", "brightness", "jpeg"] {
                let ext = if variant == "jpeg" { "jpg" } else { "png" };
                let e = compare_local_files_with_color_filter(
                    &DecodeRequest::new(root.join(format!("{id}-base.png"))),
                    &DecodeRequest::new(root.join(format!("{id}-{variant}.{ext}"))),
                    policy(),
                    photometric_policy(),
                    filter,
                    &budget,
                    || false,
                )
                .unwrap();
                assert!(e.candidate, "{id} {variant}");
                assert!(e.pixels.is_some() && e.photometric.is_some());
                let e = e.filtered.unwrap();
                assert_eq!(e.color_space, space);
                assert_eq!(e.filter, filter.filter);
                assert_eq!(budget.used(), 0);
            }
        }
        for (a, b) in [
            (830, 898),
            (830, 1084),
            (830, 1294),
            (898, 1084),
            (898, 1294),
            (1084, 1294),
        ] {
            let e = compare_local_files_with_color_filter(
                &DecodeRequest::new(root.join(format!("{a}-base.png"))),
                &DecodeRequest::new(root.join(format!("{b}-base.png"))),
                policy(),
                photometric_policy(),
                filter,
                &budget,
                || false,
            )
            .unwrap();
            assert!(!e.candidate);
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
fn indexed_natural_photo_collection_matches_exhaustive_filtered_candidates() {
    check_indexed_natural_derivative("crop");
}

#[test]
fn indexed_natural_rotations_and_resizes_preserve_labelled_scene_pairs() {
    for variant in ["rotate", "resize"] {
        check_indexed_natural_derivative(variant);
    }
}

fn check_indexed_natural_derivative(derivative: &str) {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_local_collection},
        local_index::FileFeatureBudgets,
        local_scan::{LocalComparisonMode, LocalSearchPolicy, scan_local_files_with_policy},
        warp::PhotometricFitMode,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let requests = [830, 898, 1084, 1294]
        .into_iter()
        .enumerate()
        .flat_map(|(i, scene)| {
            ["base", derivative].into_iter().enumerate().map({
                let root = root.clone();
                move |(j, variant)| {
                    (
                        (i * 2 + j) as u64,
                        DecodeRequest::new(root.join(format!("{scene}-{variant}.png"))),
                    )
                }
            })
        })
        .collect::<Vec<_>>();
    let search = LocalSearchPolicy {
        local: policy(),
        comparison: LocalComparisonMode::Filtered {
            photometric: photometric_policy(),
            filter: ColorFilterPolicy {
                filter: FilterPolicy {
                    radius: 3,
                    max_sample_pairs: 20_000_000,
                },
                color_space: FilterColorSpace::EncodedSrgb,
            },
            fit: PhotometricFitMode::RejectOutsidePolicy,
        },
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let reference = scan_local_files_with_policy(requests.clone(), search, 8, 28, &budget, || false).unwrap();
    let indexed = scan_local_collection(
        requests,
        LocalCollectionPolicy {
            search,
            budgets: FileFeatureBudgets {
                max_files: 8,
                max_features: 4000,
                max_hits: 16_000_000,
                max_pair_counts: 28,
                max_pairs: 28,
            },
        },
        &budget,
        || false,
    )
    .unwrap();
    let edges = |report: &rrrah_dedup::local_scan::LocalFileReport| {
        report
            .pairs
            .iter()
            .filter(|p| p.evidence.candidate)
            .map(|p| (p.left, p.right))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        edges(&reference),
        [(0, 1), (2, 3), (4, 5), (6, 7)],
        "{derivative}"
    );
    assert_eq!(edges(&indexed.local), edges(&reference));
    assert!(reference.issues.is_empty() && reference.source_issues.is_empty());
    assert!(
        indexed.file_issues.is_empty()
            && indexed.local.issues.is_empty()
            && indexed.local.source_issues.is_empty()
    );
    println!(
        "natural {derivative} collection: features={} hits={} proposals={} pixel_verifications={} exhaustive={}",
        indexed.indexed_features,
        indexed.descriptor_hits,
        indexed.proposed_pairs,
        indexed.pixel_verification_pairs,
        reference.pairs.len()
    );
    assert_eq!(indexed.analysed.len(), 8);
    assert_eq!(budget.used(), 0);
}

#[test]
fn strict_reflected_natural_files_keep_scene_pairs_and_reject_cross_scene_negatives() {
    use rrrah_dedup::local_scan::compare_reflected_local_files;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for source in [830, 898, 1084, 1294] {
        for target in [830, 898, 1084, 1294] {
            let a = DecodeRequest::new(root.join(format!("{source}-base.png")));
            let b = DecodeRequest::new(root.join(format!("{target}-mirror.png")));
            for (left, right) in [(&a, &b), (&b, &a)] {
                let result = compare_reflected_local_files(left, right, policy(), &budget, || false).unwrap();
                assert_eq!(
                    result.candidate,
                    source == target,
                    "source={source} target={target}"
                );
                if source == target {
                    assert!(result.geometry.is_some());
                    let pixels = result.pixels.unwrap();
                    for evidence in [pixels.forward, pixels.reverse] {
                        assert!(
                            evidence.matched_pixels * 100 >= evidence.source_pixels * 98,
                            "source={source} target={target}: {evidence:?}"
                        );
                    }
                }
                assert_eq!(budget.used(), 0);
            }
        }
    }
}

#[test]
fn indexed_reflected_natural_collection_matches_complete_pair_oracle() {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_reflected_local_collection},
        local_index::FileFeatureBudgets,
        local_scan::compare_reflected_local_files,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let requests = [830, 898, 1084, 1294]
        .into_iter()
        .enumerate()
        .flat_map(|(i, scene)| {
            let root = root.clone();
            ["base", "mirror"]
                .into_iter()
                .enumerate()
                .map(move |(j, variant)| {
                    (
                        (i * 2 + j) as u64,
                        DecodeRequest::new(root.join(format!("{scene}-{variant}.png"))),
                    )
                })
        })
        .collect::<Vec<_>>();
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let mut expected = Vec::new();
    for i in 0..requests.len() {
        for j in i + 1..requests.len() {
            if compare_reflected_local_files(&requests[i].1, &requests[j].1, policy(), &budget, || false)
                .unwrap()
                .candidate
            {
                expected.push((requests[i].0, requests[j].0));
            }
        }
    }
    assert_eq!(expected, [(0, 1), (2, 3), (4, 5), (6, 7)]);
    let report = scan_reflected_local_collection(
        requests,
        LocalCollectionPolicy {
            search: policy().into(),
            budgets: FileFeatureBudgets {
                max_files: 8,
                max_features: 8000,
                max_hits: 64_000_000,
                max_pair_counts: 28,
                max_pairs: 28,
            },
        },
        &budget,
        || false,
    )
    .unwrap();
    let actual = report
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(report.pixel_verification_pairs, 4);
    assert_eq!(report.analysed, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert!(report.file_issues.is_empty() && report.issues.is_empty() && report.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
    println!(
        "indexed_features={} descriptor_hits={} proposed_pairs={} pixel_verification_pairs={} peak_managed_bytes={}",
        report.indexed_features,
        report.descriptor_hits,
        report.proposed_pairs,
        report.pixel_verification_pairs,
        budget.peak()
    );
}

#[test]
fn reflected_natural_brightness_fitting_keeps_original_pixel_evidence() {
    use rrrah_dedup::local_scan::{compare_reflected_local_files, compare_reflected_local_files_photometric};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for id in [830, 898, 1084, 1294] {
        let a = DecodeRequest::new(root.join(format!("{id}-base.png")));
        let b = DecodeRequest::new(root.join(format!("{id}-mirror-brightness.png")));
        let strict = compare_reflected_local_files(&a, &b, policy(), &budget, || false).unwrap();
        assert!(!strict.candidate);
        assert!(strict.photometric.is_none());
        let fitted = compare_reflected_local_files_photometric(
            &a,
            &b,
            policy(),
            photometric_policy(),
            &budget,
            || false,
        )
        .unwrap();
        assert!(fitted.candidate, "{id}");
        assert_eq!(fitted.pixels, strict.pixels);
        assert!(fitted.photometric.is_some());
        let spatial = rrrah_dedup::local_scan::compare_reflected_local_files_spatial_photometric(
            &a,
            &b,
            policy(),
            rrrah_dedup::local_scan::SpatialFeaturePolicy {
                columns: 4,
                rows: 4,
                max_per_cell: 20,
            },
            photometric_policy(),
            &budget,
            || false,
        )
        .unwrap();
        assert!(
            spatial.candidate && spatial.photometric.is_some() && spatial.pixels.is_some(),
            "{id}"
        );

        assert_eq!(budget.used(), 0);
    }
    for (left, right) in [
        (830, 898),
        (830, 1084),
        (830, 1294),
        (898, 1084),
        (898, 1294),
        (1084, 1294),
    ] {
        let a = DecodeRequest::new(root.join(format!("{left}-base.png")));
        let b = DecodeRequest::new(root.join(format!("{right}-mirror-brightness.png")));
        assert!(
            !compare_reflected_local_files_photometric(
                &a,
                &b,
                policy(),
                photometric_policy(),
                &budget,
                || false
            )
            .unwrap()
            .candidate
        );
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn indexed_reflected_brightness_collection_preserves_fitted_and_strict_evidence() {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_reflected_local_collection},
        local_index::FileFeatureBudgets,
        local_scan::compare_reflected_local_files_photometric,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let requests = [830, 898, 1084, 1294]
        .into_iter()
        .enumerate()
        .flat_map(|(i, scene)| {
            let root = root.clone();
            ["base", "mirror-brightness"]
                .into_iter()
                .enumerate()
                .map(move |(j, variant)| {
                    (
                        (i * 2 + j) as u64,
                        DecodeRequest::new(root.join(format!("{scene}-{variant}.png"))),
                    )
                })
        })
        .collect::<Vec<_>>();
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let mut expected = Vec::new();
    for i in 0..requests.len() {
        for j in i + 1..requests.len() {
            if compare_reflected_local_files_photometric(
                &requests[i].1,
                &requests[j].1,
                policy(),
                photometric_policy(),
                &budget,
                || false,
            )
            .unwrap()
            .candidate
            {
                expected.push((requests[i].0, requests[j].0));
            }
        }
    }
    assert_eq!(expected, [(0, 1), (2, 3), (4, 5), (6, 7)]);
    let report = scan_reflected_local_collection(
        requests,
        LocalCollectionPolicy {
            search: rrrah_dedup::local_scan::LocalSearchPolicy {
                local: policy(),
                comparison: rrrah_dedup::local_scan::LocalComparisonMode::Photometric(photometric_policy()),
            },
            budgets: FileFeatureBudgets {
                max_files: 8,
                max_features: 8000,
                max_hits: 64_000_000,
                max_pair_counts: 28,
                max_pairs: 28,
            },
        },
        &budget,
        || false,
    )
    .unwrap();
    let actual = report
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    for pair in report.pairs.iter().filter(|pair| pair.evidence.candidate) {
        assert!(pair.evidence.photometric.is_some() && pair.evidence.pixels.is_some());
    }
    assert_eq!(report.pixel_verification_pairs, 4);
    assert_eq!(report.analysed, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert!(report.file_issues.is_empty() && report.issues.is_empty() && report.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
    println!(
        "indexed_features={} descriptor_hits={} proposed_pairs={} pixel_verification_pairs={} peak_managed_bytes={}",
        report.indexed_features,
        report.descriptor_hits,
        report.proposed_pairs,
        report.pixel_verification_pairs,
        budget.peak()
    );
}

#[test]
fn spatial_reflected_brightness_collection_matches_complete_spatial_oracle() {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_reflected_local_collection_spatial},
        local_index::FileFeatureBudgets,
        local_scan::compare_reflected_local_files_spatial_photometric,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let requests = [830, 898, 1084, 1294]
        .into_iter()
        .enumerate()
        .flat_map(|(i, scene)| {
            let root = root.clone();
            ["base", "mirror-brightness"]
                .into_iter()
                .enumerate()
                .map(move |(j, variant)| {
                    (
                        (i * 2 + j) as u64,
                        DecodeRequest::new(root.join(format!("{scene}-{variant}.png"))),
                    )
                })
        })
        .collect::<Vec<_>>();
    let grid = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 20,
    };
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let mut expected = Vec::new();
    for i in 0..requests.len() {
        for j in i + 1..requests.len() {
            let evidence = compare_reflected_local_files_spatial_photometric(
                &requests[i].1,
                &requests[j].1,
                policy(),
                grid,
                photometric_policy(),
                &budget,
                || false,
            )
            .unwrap();
            if i == 4 && j == 5 {
                let geometry = evidence.geometry.as_ref().unwrap();
                // Automatic selection must recover the authored mirror, rather
                // than increase support by distorting the exact majority.
                let transform = geometry.similarity.transform;
                assert!((transform.a - 1.).abs() < 0.001);
                assert!(transform.b.abs() < 0.001);
                for source in [[0., 0.], [319., 0.], [0., 239.], [319., 239.]] {
                    let predicted = geometry.apply(source);
                    assert!((predicted[0] - (319. - source[0])).abs() < 0.1);
                    assert!((predicted[1] - source[1]).abs() < 0.1);
                }
                eprintln!("1084 automatically selected reflected transform: {transform:?}");
            }
            if evidence.candidate {
                expected.push((requests[i].0, requests[j].0));
            }
        }
    }
    assert_eq!(expected, [(0, 1), (2, 3), (4, 5), (6, 7)]);
    let report = scan_reflected_local_collection_spatial(
        requests,
        LocalCollectionPolicy {
            search: rrrah_dedup::local_scan::LocalSearchPolicy {
                local: policy(),
                comparison: rrrah_dedup::local_scan::LocalComparisonMode::Photometric(photometric_policy()),
            },
            budgets: FileFeatureBudgets {
                max_files: 8,
                max_features: 8000,
                max_hits: 64_000_000,
                max_pair_counts: 28,
                max_pairs: 28,
            },
        },
        grid,
        &budget,
        || false,
    )
    .unwrap();
    let actual = report
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    for pair in report.pairs.iter().filter(|pair| pair.evidence.candidate) {
        assert!(pair.evidence.photometric.is_some() && pair.evidence.pixels.is_some());
    }
    assert_eq!(report.pixel_verification_pairs, 4);
    assert_eq!(report.analysed, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert!(report.file_issues.is_empty() && report.issues.is_empty() && report.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
    println!(
        "indexed_features={} descriptor_hits={} proposed_pairs={} pixel_verification_pairs={} peak_managed_bytes={}",
        report.indexed_features,
        report.descriptor_hits,
        report.proposed_pairs,
        report.pixel_verification_pairs,
        budget.peak()
    );
}

#[test]
fn recursive_reflected_brightness_keeps_fitted_mode_paths_and_frame_settings() {
    use rrrah_dedup::{
        local_collection::{
            LocalCollectionPolicy, scan_reflected_local_collection_roots,
            scan_reflected_local_collection_roots_with_requests,
        },
        local_index::FileFeatureBudgets,
        local_scan::{LocalComparisonMode, LocalSearchPolicy, compare_reflected_local_files_photometric},
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let folder = tempfile::tempdir().unwrap();
    let nested = folder.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    for id in [830, 898] {
        for variant in ["base", "mirror-brightness"] {
            let name = format!("{id}-{variant}.png");
            std::fs::copy(root.join(&name), nested.join(name)).unwrap();
        }
    }
    let p = LocalCollectionPolicy {
        search: LocalSearchPolicy {
            local: policy(),
            comparison: LocalComparisonMode::Photometric(photometric_policy()),
        },
        budgets: FileFeatureBudgets {
            max_files: 4,
            max_features: 4000,
            max_hits: 16_000_000,
            max_pair_counts: 6,
            max_pairs: 6,
        },
    };
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let roots = [folder.path().to_path_buf(), nested];
    let traversal = rrrah_dedup::exact::Options::default();
    let report = scan_reflected_local_collection_roots(&roots, &traversal, p, &budget, || false).unwrap();
    assert_eq!(report.files.len(), 4);
    assert_eq!(report.indexed.analysed.len(), 4);
    assert!(
        report.indexed.file_issues.is_empty()
            && report.indexed.issues.is_empty()
            && report.indexed.source_issues.is_empty()
    );
    let mut expected = Vec::new();
    for i in 0..4 {
        for j in i + 1..4 {
            if compare_reflected_local_files_photometric(
                &DecodeRequest::new(&report.files[i].1),
                &DecodeRequest::new(&report.files[j].1),
                policy(),
                photometric_policy(),
                &budget,
                || false,
            )
            .unwrap()
            .candidate
            {
                expected.push((report.files[i].0, report.files[j].0));
            }
        }
    }
    assert_eq!(expected.len(), 2);
    assert_eq!(
        report
            .indexed
            .pairs
            .iter()
            .filter(|pair| pair.evidence.candidate)
            .map(|pair| (pair.left, pair.right))
            .collect::<Vec<_>>(),
        expected
    );
    for pair in report.indexed.pairs.iter().filter(|pair| pair.evidence.candidate) {
        assert!(pair.evidence.photometric.is_some() && pair.evidence.pixels.is_some());
    }
    assert_eq!(budget.used(), 0);
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
}

#[test]
fn recursive_spatial_reflected_brightness_shares_pair_and_frame_policy() {
    use rrrah_dedup::{
        local_collection::{
            LocalCollectionPolicy, scan_reflected_local_collection_roots_spatial,
            scan_reflected_local_collection_roots_spatial_with_requests,
        },
        local_index::FileFeatureBudgets,
        local_scan::{
            LocalComparisonMode, LocalSearchPolicy, compare_reflected_local_files_spatial_photometric,
        },
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let folder = tempfile::tempdir().unwrap();
    let nested = folder.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    for id in [830, 898] {
        for variant in ["base", "mirror-brightness"] {
            let name = format!("{id}-{variant}.png");
            std::fs::copy(root.join(&name), nested.join(name)).unwrap();
        }
    }
    let p = LocalCollectionPolicy {
        search: LocalSearchPolicy {
            local: policy(),
            comparison: LocalComparisonMode::Photometric(photometric_policy()),
        },
        budgets: FileFeatureBudgets {
            max_files: 4,
            max_features: 4000,
            max_hits: 16_000_000,
            max_pair_counts: 6,
            max_pairs: 6,
        },
    };
    let grid = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 20,
    };
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let roots = [folder.path().to_path_buf(), nested];
    let traversal = rrrah_dedup::exact::Options::default();
    let report =
        scan_reflected_local_collection_roots_spatial(&roots, &traversal, p, grid, &budget, || false)
            .unwrap();
    assert_eq!(report.files.len(), 4);
    assert_eq!(report.indexed.analysed.len(), 4);
    assert!(
        report.indexed.file_issues.is_empty()
            && report.indexed.issues.is_empty()
            && report.indexed.source_issues.is_empty()
    );
    let mut expected = Vec::new();
    for i in 0..4 {
        for j in i + 1..4 {
            if compare_reflected_local_files_spatial_photometric(
                &DecodeRequest::new(&report.files[i].1),
                &DecodeRequest::new(&report.files[j].1),
                policy(),
                grid,
                photometric_policy(),
                &budget,
                || false,
            )
            .unwrap()
            .candidate
            {
                expected.push((report.files[i].0, report.files[j].0));
            }
        }
    }
    assert_eq!(expected.len(), 2);
    assert_eq!(
        report
            .indexed
            .pairs
            .iter()
            .filter(|pair| pair.evidence.candidate)
            .map(|pair| (pair.left, pair.right))
            .collect::<Vec<_>>(),
        expected
    );
    for pair in report.indexed.pairs.iter().filter(|pair| pair.evidence.candidate) {
        assert!(pair.evidence.photometric.is_some() && pair.evidence.pixels.is_some());
    }
    assert_eq!(budget.used(), 0);
    let unavailable = scan_reflected_local_collection_roots_spatial_with_requests(
        &roots,
        &traversal,
        p,
        grid,
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
}

#[test]
fn reflected_brightness_1084_known_fixture_geometry_passes_unchanged_pixel_policy() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    {
        let left = rrrah_dedup::decode::decode_selected_frame(
            &DecodeRequest::new(root.join("1084-base.png")),
            policy().extract.max_pixels,
            &budget,
            || false,
        )
        .unwrap();
        let right = rrrah_dedup::decode::decode_selected_frame(
            &DecodeRequest::new(root.join("1084-mirror-brightness.png")),
            policy().extract.max_pixels,
            &budget,
            || false,
        )
        .unwrap();
        let lv = left.view(|| false).unwrap();
        let rv = right.view(|| false).unwrap();
        let evidence = rrrah_dedup::warp::verify_reflected_photometric_bidirectional(
            &lv,
            &rv,
            rrrah_dedup::geometry::Transform {
                a: 1.,
                b: 0.,
                translation: [319., 0.],
            },
            photometric_policy(),
            || false,
        )
        .unwrap();
        for direction in [&evidence.forward, &evidence.reverse] {
            assert!(direction.pixels.compared_pixels >= policy().minimum_compared_pixels);
            assert!(
                direction.pixels.matched_pixels as f64 / direction.pixels.compared_pixels as f64
                    >= policy().minimum_matched_fraction
            );
        }
    }
    assert_eq!(budget.used(), 0);
}

fn reflected_filtered_search(
    color_space: FilterColorSpace,
    fit: rrrah_dedup::warp::PhotometricFitMode,
) -> rrrah_dedup::local_scan::LocalSearchPolicy {
    rrrah_dedup::local_scan::LocalSearchPolicy {
        local: policy(),
        comparison: rrrah_dedup::local_scan::LocalComparisonMode::Filtered {
            photometric: photometric_policy(),
            filter: ColorFilterPolicy {
                filter: FilterPolicy {
                    radius: 3,
                    max_sample_pairs: 20_000_000,
                },
                color_space,
            },
            fit,
        },
    }
}

#[test]
fn reflected_filtered_jpegs_preserve_original_evidence_in_both_spaces_and_fit_modes() {
    use rrrah_dedup::local_scan::compare_reflected_local_files_with_policy;
    use rrrah_dedup::warp::PhotometricFitMode;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for color_space in [FilterColorSpace::LinearSrgb, FilterColorSpace::EncodedSrgb] {
        for fit in [
            PhotometricFitMode::RejectOutsidePolicy,
            PhotometricFitMode::ConstrainedLeastSquares,
        ] {
            let search = reflected_filtered_search(color_space, fit);
            for scene in [830, 898, 1084, 1294] {
                let result = compare_reflected_local_files_with_policy(
                    &DecodeRequest::new(root.join(format!("{scene}-base.png"))),
                    &DecodeRequest::new(root.join(format!("{scene}-mirror-jpeg.png"))),
                    search,
                    None,
                    &budget,
                    || false,
                )
                .unwrap();
                assert!(result.candidate, "{scene} {color_space:?} {fit:?}: {result:?}");
                assert!(result.pixels.is_some());
                assert!(result.photometric.is_some() || result.photometric_failure.is_some());
                let filtered = result.filtered.unwrap();
                assert_eq!(filtered.color_space, color_space);
                assert_eq!(filtered.fit_mode, fit);
                for direction in [&filtered.evidence.forward, &filtered.evidence.reverse] {
                    assert!(
                        direction.pixels.matched_pixels as f64 / direction.pixels.compared_pixels as f64
                            >= search.local.minimum_matched_fraction
                    );
                }
                assert_eq!(budget.used(), 0);
            }
            for (left, right) in [(830, 898), (1084, 1294)] {
                let result = compare_reflected_local_files_with_policy(
                    &DecodeRequest::new(root.join(format!("{left}-base.png"))),
                    &DecodeRequest::new(root.join(format!("{right}-mirror-jpeg.png"))),
                    search,
                    None,
                    &budget,
                    || false,
                )
                .unwrap();
                assert!(!result.candidate);
                assert_eq!(budget.used(), 0);
            }
        }
    }
}

#[test]
fn reflected_filtered_collection_matches_spatial_file_oracle_in_both_spaces_and_fit_modes() {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_reflected_local_collection_spatial},
        local_index::FileFeatureBudgets,
        local_scan::{SpatialFeaturePolicy, compare_reflected_local_files_with_policy},
        warp::PhotometricFitMode,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let files = [
        (1, "830-base.png"),
        (2, "830-mirror-jpeg.png"),
        (3, "1084-base.png"),
        (4, "1084-mirror-jpeg.png"),
    ]
    .map(|(id, name)| (id, DecodeRequest::new(root.join(name))));
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 20,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for color_space in [FilterColorSpace::LinearSrgb, FilterColorSpace::EncodedSrgb] {
        for fit in [
            PhotometricFitMode::RejectOutsidePolicy,
            PhotometricFitMode::ConstrainedLeastSquares,
        ] {
            let search = reflected_filtered_search(color_space, fit);
            let mut expected = Vec::new();
            for i in 0..files.len() {
                for j in i + 1..files.len() {
                    let evidence = compare_reflected_local_files_with_policy(
                        &files[i].1,
                        &files[j].1,
                        search,
                        Some(grid),
                        &budget,
                        || false,
                    )
                    .unwrap();
                    if evidence.candidate {
                        expected.push((files[i].0, files[j].0));
                    }
                }
            }
            assert_eq!(expected, [(1, 2), (3, 4)], "{color_space:?} {fit:?}");
            let report = scan_reflected_local_collection_spatial(
                files.clone(),
                LocalCollectionPolicy {
                    search,
                    budgets: FileFeatureBudgets {
                        max_files: 4,
                        max_features: 4000,
                        max_hits: 16_000_000,
                        max_pair_counts: 6,
                        max_pairs: 6,
                    },
                },
                grid,
                &budget,
                || false,
            )
            .unwrap();
            assert!(
                report.issues.is_empty() && report.file_issues.is_empty() && report.source_issues.is_empty()
            );
            let accepted = report
                .pairs
                .iter()
                .filter(|p| p.evidence.candidate)
                .map(|p| (p.left, p.right))
                .collect::<Vec<_>>();
            assert_eq!(accepted, expected);
            for pair in report.pairs.iter().filter(|p| p.evidence.candidate) {
                assert!(pair.evidence.pixels.is_some());
                let filtered = pair.evidence.filtered.as_ref().unwrap();
                assert_eq!(filtered.color_space, color_space);
                assert_eq!(filtered.fit_mode, fit);
            }
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
fn reflected_filtered_file_guards_discard_late_results() {
    use rrrah_dedup::local_scan::LocalFileError;
    use std::{
        cell::Cell,
        io::Write,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };
    let dir = tempfile::tempdir().unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png");
    let left = dir.path().join("left.png");
    let right = dir.path().join("right.png");
    std::fs::copy(&source, &left).unwrap();
    std::fs::copy(source.with_file_name("mirrored.png"), &right).unwrap();
    let mut a = DecodeRequest::new(&left);
    let b = DecodeRequest::new(right);
    let generation = Arc::new(AtomicU64::new(7));
    a.cancellation = Some(rrrah_decode::GenerationToken::new(Arc::clone(&generation), 7));
    let mut p = policy();
    p.extract.max_features = 100;
    let filter = FilterPolicy {
        radius: 2,
        max_sample_pairs: 20_000_000,
    };
    let search = rrrah_dedup::local_scan::LocalSearchPolicy {
        local: p,
        comparison: rrrah_dedup::local_scan::LocalComparisonMode::Filtered {
            photometric: photometric_policy(),
            filter: ColorFilterPolicy {
                filter,
                color_space: FilterColorSpace::LinearSrgb,
            },
            fit: rrrah_dedup::warp::PhotometricFitMode::RejectOutsidePolicy,
        },
    };
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0_usize);
    let result = rrrah_dedup::local_scan::compare_reflected_local_files_with_policy(
        &a,
        &b,
        search,
        None,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert!(result.candidate);
    assert_eq!(budget.used(), 0);
    let mut limited_search = search;
    if let rrrah_dedup::local_scan::LocalComparisonMode::Filtered { ref mut filter, .. } =
        limited_search.comparison
    {
        filter.filter.max_sample_pairs = 1;
    }
    assert!(matches!(
        rrrah_dedup::local_scan::compare_reflected_local_files_with_policy(
            &a,
            &b,
            limited_search,
            None,
            &budget,
            || false
        ),
        Err(LocalFileError::Pixels(rrrah_dedup::warp::WarpError::Budget))
    ));
    assert_eq!(budget.used(), 0);
    let stop = calls.get();
    assert!(stop > 20);
    calls.set(0);
    assert!(matches!(
        rrrah_dedup::local_scan::compare_reflected_local_files_with_policy(
            &a,
            &b,
            search,
            None,
            &budget,
            || {
                calls.set(calls.get() + 1);
                calls.get() == stop - 1
            }
        ),
        Err(LocalFileError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    calls.set(0);
    assert!(matches!(
        rrrah_dedup::local_scan::compare_reflected_local_files_with_policy(
            &a,
            &b,
            search,
            None,
            &budget,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    generation.store(8, Ordering::Release);
                }
                false
            }
        ),
        Err(LocalFileError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    generation.store(7, Ordering::Release);
    calls.set(0);
    let mutated = Cell::new(false);
    assert!(matches!(
        rrrah_dedup::local_scan::compare_reflected_local_files_with_policy(
            &a,
            &b,
            search,
            None,
            &budget,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == stop - 20 {
                    std::fs::OpenOptions::new()
                        .append(true)
                        .open(&left)
                        .unwrap()
                        .write_all(&[0])
                        .unwrap();
                    mutated.set(true);
                }
                false
            }
        ),
        Err(LocalFileError::Source(rrrah_dedup::exact::SnapshotError::Changed))
    ));
    assert!(mutated.get());
    assert_eq!(budget.used(), 0);
}

#[test]
fn reflected_filtered_invalid_policy_precedes_source_and_collection_access() {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_reflected_local_collection},
        local_index::FileFeatureBudgets,
        local_scan::{LocalComparisonMode, LocalFileError, compare_reflected_local_files_with_policy},
        scan::ScanError,
        warp::{PhotometricFitMode, WarpError},
    };
    let missing = DecodeRequest::new("/this/source/does/not/exist.png");
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let mut search = reflected_filtered_search(
        FilterColorSpace::LinearSrgb,
        PhotometricFitMode::RejectOutsidePolicy,
    );
    if let LocalComparisonMode::Filtered { ref mut filter, .. } = search.comparison {
        filter.filter.radius = 0;
    }
    assert!(matches!(
        compare_reflected_local_files_with_policy(&missing, &missing, search, None, &budget, || false),
        Err(LocalFileError::Pixels(WarpError::Invalid))
    ));
    let files = std::iter::from_fn(|| -> Option<(u64, DecodeRequest)> {
        panic!("invalid policy must precede enumeration")
    });
    assert!(matches!(
        scan_reflected_local_collection(
            files,
            LocalCollectionPolicy {
                search,
                budgets: FileFeatureBudgets {
                    max_files: 2,
                    max_features: 2000,
                    max_hits: 10000,
                    max_pair_counts: 1,
                    max_pairs: 1
                },
            },
            &budget,
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(budget.peak(), 0);
}

fn recursive_filtered_fixture() -> (tempfile::TempDir, Vec<PathBuf>) {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let folder = tempfile::tempdir().unwrap();
    let nested = folder.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    for name in [
        "830-base.png",
        "830-mirror-jpeg.png",
        "1084-base.png",
        "1084-mirror-jpeg.png",
    ] {
        std::fs::copy(source.join(name), nested.join(name)).unwrap();
    }
    std::fs::write(folder.path().join("bad.png"), b"corrupt image").unwrap();
    #[cfg(unix)]
    {
        std::fs::hard_link(
            nested.join("830-base.png"),
            folder.path().join("830-base-alias.png"),
        )
        .unwrap();
        std::os::unix::fs::symlink(folder.path(), nested.join("loop")).unwrap();
    }
    let roots = vec![folder.path().to_path_buf(), nested];
    (folder, roots)
}

fn recursive_filtered_policy(
    search: rrrah_dedup::local_scan::LocalSearchPolicy,
) -> rrrah_dedup::local_collection::LocalCollectionPolicy {
    rrrah_dedup::local_collection::LocalCollectionPolicy {
        search,
        budgets: rrrah_dedup::local_index::FileFeatureBudgets {
            max_files: 5,
            max_features: 8000,
            max_hits: 64_000_000,
            max_pair_counts: 10,
            max_pairs: 10,
        },
    }
}

#[test]
fn recursive_filtered_reflections_preserve_oracle_aliases_errors_and_modes() {
    use rrrah_dedup::{
        local_collection::{
            scan_reflected_local_collection_roots_spatial_with_requests,
            scan_reflected_local_collection_roots_with_requests,
        },
        local_scan::{SpatialFeaturePolicy, compare_reflected_local_files_with_policy},
        warp::PhotometricFitMode,
    };
    let (_folder, roots) = recursive_filtered_fixture();
    let traversal = rrrah_dedup::exact::Options {
        follow_symlinks: true,
        ..Default::default()
    };
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 20,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for spatial in [None, Some(grid)] {
        for color_space in [FilterColorSpace::LinearSrgb, FilterColorSpace::EncodedSrgb] {
            for fit in [
                PhotometricFitMode::RejectOutsidePolicy,
                PhotometricFitMode::ConstrainedLeastSquares,
            ] {
                let search = reflected_filtered_search(color_space, fit);
                let settings = recursive_filtered_policy(search);
                let requests = std::cell::RefCell::new(Vec::new());
                let factory = |path: &std::path::Path| {
                    requests.borrow_mut().push(path.to_path_buf());
                    DecodeRequest::new(path)
                };
                let report = if let Some(grid) = spatial {
                    scan_reflected_local_collection_roots_spatial_with_requests(
                        &roots,
                        &traversal,
                        settings,
                        grid,
                        &budget,
                        factory,
                        || false,
                    )
                } else {
                    scan_reflected_local_collection_roots_with_requests(
                        &roots,
                        &traversal,
                        settings,
                        &budget,
                        factory,
                        || false,
                    )
                }
                .unwrap();
                assert_eq!(report.files.len(), 5);
                assert_eq!(requests.borrow().len(), 5);
                assert!(
                    report
                        .files
                        .iter()
                        .all(|(_, path)| requests.borrow().contains(path))
                );
                assert_eq!(report.indexed.analysed.len(), 4);
                assert_eq!(report.indexed.file_issues.len(), 1);
                let bad_id = report
                    .files
                    .iter()
                    .find(|(_, path)| path.ends_with("bad.png"))
                    .unwrap()
                    .0;
                assert_eq!(report.indexed.file_issues[0].0, bad_id);
                assert!(report.indexed.issues.is_empty() && report.indexed.source_issues.is_empty());
                #[cfg(unix)]
                assert!(report.aliases.iter().any(|aliases| {
                    aliases.paths.iter().any(|path| path.ends_with("830-base.png"))
                        && aliases
                            .paths
                            .iter()
                            .any(|path| path.ends_with("830-base-alias.png"))
                }));
                let healthy = report
                    .files
                    .iter()
                    .filter(|(id, _)| *id != bad_id)
                    .collect::<Vec<_>>();
                let mut expected = Vec::new();
                for i in 0..healthy.len() {
                    for j in i + 1..healthy.len() {
                        let left = DecodeRequest::new(&healthy[i].1);
                        let right = DecodeRequest::new(&healthy[j].1);
                        let evidence = compare_reflected_local_files_with_policy(
                            &left,
                            &right,
                            search,
                            spatial,
                            &budget,
                            || false,
                        )
                        .unwrap();
                        if evidence.candidate {
                            expected.push((healthy[i].0, healthy[j].0));
                        }
                    }
                }
                assert_eq!(expected.len(), 2);
                for &(left, right) in &expected {
                    let left = report
                        .files
                        .iter()
                        .find(|(id, _)| *id == left)
                        .unwrap()
                        .1
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap();
                    let right = report
                        .files
                        .iter()
                        .find(|(id, _)| *id == right)
                        .unwrap()
                        .1
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap();
                    assert_eq!(left.contains("mirror"), !right.contains("mirror"));
                    assert!(
                        left.starts_with("830") && right.starts_with("830")
                            || left.starts_with("1084") && right.starts_with("1084")
                    );
                }
                let mut scene_labels = expected
                    .iter()
                    .map(|(left, _)| {
                        let name = report
                            .files
                            .iter()
                            .find(|(id, _)| id == left)
                            .unwrap()
                            .1
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap();
                        if name.starts_with("830") { 830 } else { 1084 }
                    })
                    .collect::<Vec<_>>();
                scene_labels.sort_unstable();
                assert_eq!(
                    scene_labels,
                    [830, 1084],
                    "one independently labelled positive for each scene"
                );
                let actual = report
                    .indexed
                    .pairs
                    .iter()
                    .filter(|pair| pair.evidence.candidate)
                    .map(|pair| (pair.left, pair.right))
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected);
                for pair in report.indexed.pairs.iter().filter(|pair| pair.evidence.candidate) {
                    assert!(pair.evidence.pixels.is_some());
                    let filtered = pair.evidence.filtered.as_ref().unwrap();
                    assert_eq!(filtered.color_space, color_space);
                    assert_eq!(filtered.fit_mode, fit);
                }
                assert_eq!(budget.used(), 0);
                eprintln!(
                    "filtered roots spatial={} {color_space:?} {fit:?}: files={} aliases={} errors={} proposed={} confirmed={}",
                    spatial.is_some(),
                    report.files.len(),
                    report.aliases.len(),
                    report.indexed.file_issues.len(),
                    report.indexed.proposed_pairs,
                    report.indexed.pixel_verification_pairs
                );
            }
        }
    }
}

#[test]
fn recursive_filtered_reflection_keeps_frame_selection_and_factory_guards() {
    use rrrah_dedup::{
        local_collection::scan_reflected_local_collection_roots_spatial_with_requests,
        local_scan::{LocalComparisonMode, SpatialFeaturePolicy},
        scan::ScanError,
        warp::PhotometricFitMode,
    };
    let (_folder, roots) = recursive_filtered_fixture();
    let traversal = rrrah_dedup::exact::Options {
        follow_symlinks: true,
        ..Default::default()
    };
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 20,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let search = reflected_filtered_search(
        FilterColorSpace::EncodedSrgb,
        PhotometricFitMode::ConstrainedLeastSquares,
    );
    let settings = recursive_filtered_policy(search);
    let report = scan_reflected_local_collection_roots_spatial_with_requests(
        &roots,
        &traversal,
        settings,
        grid,
        &budget,
        |path| {
            let mut request = DecodeRequest::new(path);
            request.image_index = 1;
            request
        },
        || false,
    )
    .unwrap();
    assert_eq!(report.files.len(), 5);
    assert_eq!(report.indexed.file_issues.len(), 5);
    assert!(report.indexed.analysed.is_empty() && report.indexed.pairs.is_empty());
    assert_eq!(budget.used(), 0);
    let mut invalid = settings;
    if let LocalComparisonMode::Filtered { ref mut filter, .. } = invalid.search.comparison {
        filter.filter.radius = 0;
    }
    assert!(matches!(
        scan_reflected_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            invalid,
            grid,
            &budget,
            |_| panic!("invalid filter before factory"),
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert!(matches!(
        scan_reflected_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            settings,
            SpatialFeaturePolicy { columns: 0, ..grid },
            &budget,
            |_| panic!("invalid grid before factory"),
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert!(matches!(
        scan_reflected_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            settings,
            grid,
            &budget,
            |_| DecodeRequest::new("substituted.png"),
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert!(matches!(
        scan_reflected_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            settings,
            grid,
            &budget,
            |_| panic!("early cancellation before factory"),
            || true
        ),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    let mut too_few_files = settings;
    too_few_files.budgets.max_files = 4;
    assert!(matches!(
        scan_reflected_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            too_few_files,
            grid,
            &budget,
            |_| panic!("file budget before factory"),
            || false
        ),
        Err(ScanError::Budget)
    ));
    let too_few_features = rrrah_dedup::local_collection::LocalCollectionPolicy {
        budgets: rrrah_dedup::local_index::FileFeatureBudgets {
            max_features: 0,
            ..settings.budgets
        },
        ..settings
    };
    assert!(matches!(
        scan_reflected_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            too_few_features,
            grid,
            &budget,
            |path| DecodeRequest::new(path),
            || false
        ),
        Err(ScanError::Budget)
    ));
    let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(8));
    assert!(matches!(
        scan_reflected_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            settings,
            grid,
            &budget,
            |path| {
                let mut request = DecodeRequest::new(path);
                request.cancellation = Some(rrrah_decode::GenerationToken::new(
                    std::sync::Arc::clone(&generation),
                    7,
                ));
                request
            },
            || false
        ),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn ordinary_spatial_brightness_keeps_all_four_labelled_camera_scenes() {
    use rrrah_dedup::local_scan::{
        LocalComparisonMode, LocalSearchPolicy, SpatialFeaturePolicy, compare_local_files_spatial_with_policy,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 20,
    };
    for scene in [830, 898, 1084, 1294] {
        let evidence = compare_local_files_spatial_with_policy(
            &DecodeRequest::new(root.join(format!("{scene}-base.png"))),
            &DecodeRequest::new(root.join(format!("{scene}-brightness.png"))),
            LocalSearchPolicy {
                local: policy(),
                comparison: LocalComparisonMode::Photometric(photometric_policy()),
            },
            grid,
            &budget,
            || false,
        )
        .unwrap();
        eprintln!(
            "ordinary spatial brightness {scene}: matches={} geometry={:?} candidate={}",
            evidence.correspondences.len(),
            evidence.geometry.as_ref().map(|g| g.transform),
            evidence.candidate
        );
        assert!(
            evidence.candidate,
            "labelled brightness derivative {scene}: {evidence:?}"
        );
        assert!(evidence.pixels.is_some() && evidence.photometric.is_some());
        assert_eq!(budget.used(), 0);
    }
}

#[test]
#[allow(clippy::too_many_lines)] // Keep the exhaustive oracle and indexed labels together.
fn ordinary_spatial_brightness_collection_matches_complete_spatial_oracle() {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_local_collection_spatial},
        local_index::FileFeatureBudgets,
        local_scan::compare_local_files_spatial_with_policy,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let requests = [830, 898, 1084, 1294]
        .into_iter()
        .enumerate()
        .flat_map(|(i, scene)| {
            let root = root.clone();
            ["base", "brightness"]
                .into_iter()
                .enumerate()
                .map(move |(j, variant)| {
                    (
                        (i * 2 + j) as u64,
                        DecodeRequest::new(root.join(format!("{scene}-{variant}.png"))),
                    )
                })
        })
        .collect::<Vec<_>>();
    let grid = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 20,
    };
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let mut expected = Vec::new();
    for i in 0..requests.len() {
        for j in i + 1..requests.len() {
            let evidence = compare_local_files_spatial_with_policy(
                &requests[i].1,
                &requests[j].1,
                rrrah_dedup::local_scan::LocalSearchPolicy {
                    local: policy(),
                    comparison: rrrah_dedup::local_scan::LocalComparisonMode::Photometric(
                        photometric_policy(),
                    ),
                },
                grid,
                &budget,
                || false,
            )
            .unwrap();
            if i == 4 && j == 5 {
                let geometry = evidence.geometry.as_ref().unwrap();
                // Automatic selection must recover the authored identity, rather
                // than increase support by distorting the exact majority.
                let transform = geometry.transform;
                assert!((transform.a - 1.).abs() < 0.001);
                assert!(transform.b.abs() < 0.001);
                for source in [[0., 0.], [319., 0.], [0., 239.], [319., 239.]] {
                    let predicted = transform.apply(source);
                    assert!((predicted[0] - source[0]).abs() < 0.1);
                    assert!((predicted[1] - source[1]).abs() < 0.1);
                }
                eprintln!("1084 automatically selected ordinary transform: {transform:?}");
            }
            if evidence.candidate {
                expected.push((requests[i].0, requests[j].0));
            }
        }
    }
    assert_eq!(expected, [(0, 1), (2, 3), (4, 5), (6, 7)]);
    let report = scan_local_collection_spatial(
        requests,
        LocalCollectionPolicy {
            search: rrrah_dedup::local_scan::LocalSearchPolicy {
                local: policy(),
                comparison: rrrah_dedup::local_scan::LocalComparisonMode::Photometric(photometric_policy()),
            },
            budgets: FileFeatureBudgets {
                max_files: 8,
                max_features: 8000,
                max_hits: 64_000_000,
                max_pair_counts: 28,
                max_pairs: 28,
            },
        },
        grid,
        &budget,
        || false,
    )
    .unwrap();
    let actual = report
        .local
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    for pair in report.local.pairs.iter().filter(|pair| pair.evidence.candidate) {
        assert!(pair.evidence.photometric.is_some() && pair.evidence.pixels.is_some());
    }
    assert_eq!(report.pixel_verification_pairs, 4);
    assert_eq!(report.analysed, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty()
    );
    assert_eq!(budget.used(), 0);
    println!(
        "indexed_features={} descriptor_hits={} proposed_pairs={} pixel_verification_pairs={} peak_managed_bytes={}",
        report.indexed_features,
        report.descriptor_hits,
        report.proposed_pairs,
        report.pixel_verification_pairs,
        budget.peak()
    );
}

#[test]
fn reflected_range_and_display_collection_match_all_labelled_pairs() {
    for display in [false, true] {
        check_reflected_range_collection(display);
    }
}

#[allow(clippy::too_many_lines)] // Keep independent labels and the complete pair oracle together.
fn check_reflected_range_collection(display: bool) {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_reflected_local_collection_spatial},
        local_index::FileFeatureBudgets,
        local_scan::compare_reflected_local_files_with_policy,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    let range = rrrah_dedup::warp::FitSampleRange {
        minimum: 0.,
        maximum: 1.,
    };
    let comparison = if display {
        rrrah_dedup::local_scan::LocalComparisonMode::DisplayProjection {
            photometric: photometric_policy(),
            range,
        }
    } else {
        rrrah_dedup::local_scan::LocalComparisonMode::RangePhotometric {
            photometric: photometric_policy(),
            range,
        }
    };
    let search = rrrah_dedup::local_scan::LocalSearchPolicy {
        local: policy(),
        comparison,
    };
    let requests = [830, 898, 1084, 1294]
        .into_iter()
        .enumerate()
        .flat_map(|(i, scene)| {
            let root = root.clone();
            ["base", "mirror-brightness"]
                .into_iter()
                .enumerate()
                .map(move |(j, variant)| {
                    (
                        (i * 2 + j) as u64,
                        DecodeRequest::new(root.join(format!("{scene}-{variant}.png"))),
                    )
                })
        })
        .collect::<Vec<_>>();
    let grid = rrrah_dedup::local_scan::SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 20,
    };
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let mut expected = Vec::new();
    for i in 0..requests.len() {
        for j in i + 1..requests.len() {
            let evidence = compare_reflected_local_files_with_policy(
                &requests[i].1,
                &requests[j].1,
                search,
                Some(grid),
                &budget,
                || false,
            )
            .unwrap();
            if i == 4 && j == 5 {
                let geometry = evidence.geometry.as_ref().unwrap();
                // Automatic selection must recover the authored mirror, rather
                // than increase support by distorting the exact majority.
                let transform = geometry.similarity.transform;
                assert!((transform.a - 1.).abs() < 0.001);
                assert!(transform.b.abs() < 0.001);
                for source in [[0., 0.], [319., 0.], [0., 239.], [319., 239.]] {
                    let predicted = geometry.apply(source);
                    assert!((predicted[0] - (319. - source[0])).abs() < 0.1);
                    assert!((predicted[1] - source[1]).abs() < 0.1);
                }
                eprintln!("1084 automatically selected reflected transform: {transform:?}");
            }
            assert_eq!(evidence.photometric_fit_range, Some(range));
            assert_eq!(evidence.display_projection, display);
            if evidence.candidate {
                expected.push((requests[i].0, requests[j].0));
            }
        }
    }
    assert_eq!(expected, [(0, 1), (2, 3), (4, 5), (6, 7)]);
    let report = scan_reflected_local_collection_spatial(
        requests,
        LocalCollectionPolicy {
            search,
            budgets: FileFeatureBudgets {
                max_files: 8,
                max_features: 8000,
                max_hits: 64_000_000,
                max_pair_counts: 28,
                max_pairs: 28,
            },
        },
        grid,
        &budget,
        || false,
    )
    .unwrap();
    assert!(
        report
            .pairs
            .iter()
            .all(|p| p.evidence.photometric_fit_range == Some(range)
                && p.evidence.display_projection == display)
    );
    let actual = report
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    for pair in report.pairs.iter().filter(|pair| pair.evidence.candidate) {
        assert!(pair.evidence.photometric.is_some() && pair.evidence.pixels.is_some());
    }
    assert_eq!(report.pixel_verification_pairs, 4);
    assert_eq!(report.analysed, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert!(report.file_issues.is_empty() && report.issues.is_empty() && report.source_issues.is_empty());
    assert_eq!(budget.used(), 0);
    println!(
        "indexed_features={} descriptor_hits={} proposed_pairs={} pixel_verification_pairs={} peak_managed_bytes={}",
        report.indexed_features,
        report.descriptor_hits,
        report.proposed_pairs,
        report.pixel_verification_pairs,
        budget.peak()
    );
}

#[test]
#[allow(clippy::too_many_lines)] // Keep recursive discovery and independent scene oracle together.
fn recursive_ranged_reflections_preserve_oracle_aliases_errors_and_modes() {
    use rrrah_dedup::{
        local_collection::{
            scan_reflected_local_collection_roots_spatial_with_requests,
            scan_reflected_local_collection_roots_with_requests,
        },
        local_scan::{LocalComparisonMode, LocalSearchPolicy},
        local_scan::{SpatialFeaturePolicy, compare_reflected_local_files_with_policy},
        warp::FitSampleRange,
    };
    let (_folder, roots) = recursive_filtered_fixture();
    let photos = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos");
    for scene in [830, 1084] {
        std::fs::remove_file(roots[1].join(format!("{scene}-mirror-jpeg.png"))).unwrap();
        std::fs::copy(
            photos.join(format!("{scene}-mirror-brightness.png")),
            roots[1].join(format!("{scene}-mirror-brightness.png")),
        )
        .unwrap();
    }
    let range = FitSampleRange {
        minimum: 0.,
        maximum: 1.,
    };
    let traversal = rrrah_dedup::exact::Options {
        follow_symlinks: true,
        ..Default::default()
    };
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 20,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for spatial in [None, Some(grid)] {
        for display in [false, true] {
            let search = LocalSearchPolicy {
                local: policy(),
                comparison: if display {
                    LocalComparisonMode::DisplayProjection {
                        photometric: photometric_policy(),
                        range,
                    }
                } else {
                    LocalComparisonMode::RangePhotometric {
                        photometric: photometric_policy(),
                        range,
                    }
                },
            };
            let settings = recursive_filtered_policy(search);
            let requests = std::cell::RefCell::new(Vec::new());
            let factory = |path: &std::path::Path| {
                requests.borrow_mut().push(path.to_path_buf());
                DecodeRequest::new(path)
            };
            let report = if let Some(grid) = spatial {
                scan_reflected_local_collection_roots_spatial_with_requests(
                    &roots,
                    &traversal,
                    settings,
                    grid,
                    &budget,
                    factory,
                    || false,
                )
            } else {
                scan_reflected_local_collection_roots_with_requests(
                    &roots,
                    &traversal,
                    settings,
                    &budget,
                    factory,
                    || false,
                )
            }
            .unwrap();
            assert_eq!(report.files.len(), 5);
            assert_eq!(requests.borrow().len(), 5);
            assert!(
                report
                    .files
                    .iter()
                    .all(|(_, path)| requests.borrow().contains(path))
            );
            assert_eq!(report.indexed.analysed.len(), 4);
            assert_eq!(report.indexed.file_issues.len(), 1);
            let bad_id = report
                .files
                .iter()
                .find(|(_, path)| path.ends_with("bad.png"))
                .unwrap()
                .0;
            assert_eq!(report.indexed.file_issues[0].0, bad_id);
            assert!(report.indexed.issues.is_empty() && report.indexed.source_issues.is_empty());
            #[cfg(unix)]
            assert!(report.aliases.iter().any(|aliases| {
                aliases.paths.iter().any(|path| path.ends_with("830-base.png"))
                    && aliases
                        .paths
                        .iter()
                        .any(|path| path.ends_with("830-base-alias.png"))
            }));
            let healthy = report
                .files
                .iter()
                .filter(|(id, _)| *id != bad_id)
                .collect::<Vec<_>>();
            let mut expected = Vec::new();
            for i in 0..healthy.len() {
                for j in i + 1..healthy.len() {
                    let left = DecodeRequest::new(&healthy[i].1);
                    let right = DecodeRequest::new(&healthy[j].1);
                    let evidence = compare_reflected_local_files_with_policy(
                        &left,
                        &right,
                        search,
                        spatial,
                        &budget,
                        || false,
                    )
                    .unwrap();
                    if evidence.candidate {
                        expected.push((healthy[i].0, healthy[j].0));
                    }
                }
            }
            assert_eq!(expected.len(), 2);
            for &(left, right) in &expected {
                let left = report
                    .files
                    .iter()
                    .find(|(id, _)| *id == left)
                    .unwrap()
                    .1
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap();
                let right = report
                    .files
                    .iter()
                    .find(|(id, _)| *id == right)
                    .unwrap()
                    .1
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap();
                assert_eq!(left.contains("mirror"), !right.contains("mirror"));
                assert!(
                    left.starts_with("830") && right.starts_with("830")
                        || left.starts_with("1084") && right.starts_with("1084")
                );
            }
            let mut scene_labels = expected
                .iter()
                .map(|(left, _)| {
                    let name = report
                        .files
                        .iter()
                        .find(|(id, _)| id == left)
                        .unwrap()
                        .1
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap();
                    if name.starts_with("830") { 830 } else { 1084 }
                })
                .collect::<Vec<_>>();
            scene_labels.sort_unstable();
            assert_eq!(
                scene_labels,
                [830, 1084],
                "one independently labelled positive for each scene"
            );
            let actual = report
                .indexed
                .pairs
                .iter()
                .filter(|pair| pair.evidence.candidate)
                .map(|pair| (pair.left, pair.right))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            for pair in report.indexed.pairs.iter().filter(|pair| pair.evidence.candidate) {
                assert!(pair.evidence.pixels.is_some());
                assert!(pair.evidence.photometric.is_some());
                assert_eq!(pair.evidence.photometric_fit_range, Some(range));
                assert_eq!(pair.evidence.display_projection, display);
            }
            assert_eq!(budget.used(), 0);
            eprintln!(
                "ranged roots spatial={} display={display}: files={} aliases={} errors={} proposed={} confirmed={}",
                spatial.is_some(),
                report.files.len(),
                report.aliases.len(),
                report.indexed.file_issues.len(),
                report.indexed.proposed_pairs,
                report.indexed.pixel_verification_pairs
            );
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)] // Qualify all early guards for both explicit ranged modes.
fn recursive_ranged_reflection_keeps_frame_selection_and_factory_guards() {
    use rrrah_dedup::{
        local_collection::scan_reflected_local_collection_roots_spatial_with_requests,
        local_scan::{LocalComparisonMode, SpatialFeaturePolicy},
        scan::ScanError,
        warp::FitSampleRange,
    };
    let (_folder, roots) = recursive_filtered_fixture();
    let traversal = rrrah_dedup::exact::Options {
        follow_symlinks: true,
        ..Default::default()
    };
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 20,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for display in [false, true] {
        let range = FitSampleRange {
            minimum: 0.,
            maximum: 1.,
        };
        let search = rrrah_dedup::local_scan::LocalSearchPolicy {
            local: policy(),
            comparison: if display {
                LocalComparisonMode::DisplayProjection {
                    photometric: photometric_policy(),
                    range,
                }
            } else {
                LocalComparisonMode::RangePhotometric {
                    photometric: photometric_policy(),
                    range,
                }
            },
        };
        let settings = recursive_filtered_policy(search);
        let report = scan_reflected_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            settings,
            grid,
            &budget,
            |path| {
                let mut request = DecodeRequest::new(path);
                request.image_index = 1;
                request
            },
            || false,
        )
        .unwrap();
        assert_eq!(report.files.len(), 5);
        assert_eq!(report.indexed.file_issues.len(), 5);
        assert!(report.indexed.analysed.is_empty() && report.indexed.pairs.is_empty());
        assert_eq!(budget.used(), 0);
        let mut invalid = settings;
        match &mut invalid.search.comparison {
            LocalComparisonMode::RangePhotometric { range, .. }
            | LocalComparisonMode::DisplayProjection { range, .. } => range.maximum = range.minimum,
            _ => unreachable!(),
        }
        assert!(matches!(
            scan_reflected_local_collection_roots_spatial_with_requests(
                &roots,
                &traversal,
                invalid,
                grid,
                &budget,
                |_| panic!("invalid range before factory"),
                || false
            ),
            Err(ScanError::InvalidPolicy)
        ));
        assert!(matches!(
            scan_reflected_local_collection_roots_spatial_with_requests(
                &roots,
                &traversal,
                settings,
                SpatialFeaturePolicy { columns: 0, ..grid },
                &budget,
                |_| panic!("invalid grid before factory"),
                || false
            ),
            Err(ScanError::InvalidPolicy)
        ));
        assert!(matches!(
            scan_reflected_local_collection_roots_spatial_with_requests(
                &roots,
                &traversal,
                settings,
                grid,
                &budget,
                |_| DecodeRequest::new("substituted.png"),
                || false
            ),
            Err(ScanError::InvalidPolicy)
        ));
        assert!(matches!(
            scan_reflected_local_collection_roots_spatial_with_requests(
                &roots,
                &traversal,
                settings,
                grid,
                &budget,
                |_| panic!("early cancellation before factory"),
                || true
            ),
            Err(ScanError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        let mut too_few_files = settings;
        too_few_files.budgets.max_files = 4;
        assert!(matches!(
            scan_reflected_local_collection_roots_spatial_with_requests(
                &roots,
                &traversal,
                too_few_files,
                grid,
                &budget,
                |_| panic!("file budget before factory"),
                || false
            ),
            Err(ScanError::Budget)
        ));
        let too_few_features = rrrah_dedup::local_collection::LocalCollectionPolicy {
            budgets: rrrah_dedup::local_index::FileFeatureBudgets {
                max_features: 0,
                ..settings.budgets
            },
            ..settings
        };
        assert!(matches!(
            scan_reflected_local_collection_roots_spatial_with_requests(
                &roots,
                &traversal,
                too_few_features,
                grid,
                &budget,
                |path| DecodeRequest::new(path),
                || false
            ),
            Err(ScanError::Budget)
        ));
        let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(8));
        assert!(matches!(
            scan_reflected_local_collection_roots_spatial_with_requests(
                &roots,
                &traversal,
                settings,
                grid,
                &budget,
                |path| {
                    let mut request = DecodeRequest::new(path);
                    request.cancellation = Some(rrrah_decode::GenerationToken::new(
                        std::sync::Arc::clone(&generation),
                        7,
                    ));
                    request
                },
                || false
            ),
            Err(ScanError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
}
