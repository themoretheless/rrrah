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
fn pyramid_photometric_files_preserve_identity_refusals_and_cancel_retry() {
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
    let calls = Cell::new(0);
    let e = compare_local_files_projective_pyramid_photometric(&a, &a, p, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let checkpoints = calls.get();
    assert!(e.candidate, "{e:?}");
    assert!(e.pixels.is_some() && e.unfitted.is_some());
    assert!(e.registered_transform.is_none());
    assert_eq!(budget.used(), 0);
    assert!(
        !compare_local_files_projective_pyramid_photometric(&a, &b, p, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
    let zero = MemoryBudget::new(0);
    assert!(compare_local_files_projective_pyramid_photometric(&a, &a, p, &zero, || false).is_err());
    assert_eq!(zero.used(), 0);
    let mut invalid = p;
    invalid.max_levels = 0;
    let missing = DecodeRequest::new(root.join("absent.png"));
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    assert!(matches!(
        compare_local_files_projective_pyramid_photometric(&missing, &missing, invalid, &fresh, || false),
        Err(LocalFileError::InvalidPolicy)
    ));
    assert_eq!(fresh.peak(), 0);
    let mut no_work = p;
    no_work.filter.filter.max_sample_pairs = 0;
    assert!(matches!(
        compare_local_files_projective_pyramid_photometric(&a, &a, no_work, &budget, || false),
        Err(LocalFileError::Pixels(rrrah_dedup::warp::WarpError::Budget))
    ));
    assert_eq!(budget.used(), 0);
    for stop in [1, checkpoints / 2, checkpoints] {
        calls.set(0);
        assert!(
            matches!(
                compare_local_files_projective_pyramid_photometric(&a, &a, p, &budget, || {
                    calls.set(calls.get() + 1);
                    calls.get() >= stop
                }),
                Err(LocalFileError::Cancelled)
            ),
            "stop={stop}"
        );
        assert_eq!(budget.used(), 0);
    }
    assert!(
        compare_local_files_projective_pyramid_photometric(&a, &a, p, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
}
#[test]
fn real_file_projective_identity_and_terminal_refusals() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout/2414-base.png");
    let request = DecodeRequest::new(path);
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let e = compare_local_files_projective(&request, &request, policy(), &budget, || false).unwrap();
    assert!(e.candidate, "{e:?}");
    assert!(e.geometry.is_some());
    assert_eq!(budget.used(), 0);
    let mut limited = policy();
    limited.geometry.max_hypotheses = 0;
    assert!(matches!(
        compare_local_files_projective(&request, &request, limited, &budget, || false),
        Err(rrrah_dedup::local_scan::LocalFileError::Geometry(
            rrrah_dedup::geometry::GeometryError::Budget
        ))
    ));
    assert_eq!(budget.used(), 0);
    let cancelled = MemoryBudget::new(64 * 1024 * 1024);
    assert!(
        compare_local_files_projective(&request, &request, policy(), &cancelled, || cancelled.peak() > 0)
            .is_err()
    );
    assert_eq!(cancelled.used(), 0);
    assert!(
        compare_local_files_projective(&request, &request, policy(), &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
}
#[test]
fn real_preview_perspective_derivatives() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut p = policy();
    p.matching.max_distance = 64;
    p.geometry.tolerance = 2.0;
    p.geometry.min_inliers = 10;
    p.pixels.tolerance = 0.03;
    p.minimum_coverage_fraction = 0.3;
    p.minimum_matched_fraction = 0.9;
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut misses = Vec::new();
    for id in [2414, 2418, 2883, 5025, 1425, 5495] {
        let a = DecodeRequest::new(root.join(format!("photos-heldout/{id}-base.png")));
        let b = DecodeRequest::new(root.join(format!("photos-projective/{id}-perspective.png")));
        match rrrah_dedup::local_scan::compare_local_files_projective_registered(
            &a,
            &b,
            p,
            rrrah_dedup::warp::ColorFilterPolicy {
                filter: rrrah_dedup::warp::FilterPolicy {
                    radius: 1,
                    max_sample_pairs: 4_000_000,
                },
                color_space: rrrah_dedup::warp::FilterColorSpace::LinearSrgb,
            },
            rrrah_dedup::warp::ProjectiveRegistrationPolicy {
                radius: 1,
                stride: 8,
                rounds: 128,
                max_sample_pairs: 64_000_000,
            },
            &budget,
            || false,
        ) {
            Ok(e) if e.candidate => {}
            Ok(e) => misses.push(format!(
                "{id}: correspondences={} geometry={:?} pixels={:?}",
                e.correspondences.len(),
                e.geometry,
                e.filtered
            )),
            Err(error) => misses.push(format!("{id}: {error}")),
        }
        assert_eq!(budget.used(), 0);
    }
    assert!(misses.is_empty(), "{}", misses.join("\n"));
}
#[test]
fn perspective_generator_geometry_diagnostic() {
    use rrrah_dedup::{
        decode::decode_selected_frame, geometry::ProjectiveTransform, warp::verify_projective_bidirectional,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let inverse = ProjectiveTransform {
        matrix: [[1.0, 0.025, -4.0], [-0.012, 1.0, 2.0], [0.00015, -0.0001, 1.0]],
    };
    let mut forward = inverse.inverse().unwrap().matrix;
    // Pillow's transform addresses pixel centers at half-integer coordinates.
    for row in &mut forward {
        row[2] += 0.5 * (row[0] + row[1]);
    }
    for axis in 0..3 {
        forward[0][axis] -= 0.5 * forward[2][axis];
        forward[1][axis] -= 0.5 * forward[2][axis];
    }
    let transform = ProjectiveTransform { matrix: forward };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for id in [2414, 2418, 2883, 5025, 1425, 5495] {
        let a = decode_selected_frame(
            &DecodeRequest::new(root.join(format!("photos-heldout/{id}-base.png"))),
            200_000,
            &budget,
            || false,
        )
        .unwrap();
        let b = decode_selected_frame(
            &DecodeRequest::new(root.join(format!("photos-projective/{id}-perspective.png"))),
            200_000,
            &budget,
            || false,
        )
        .unwrap();
        let evidence = verify_projective_bidirectional(
            &a.view(|| false).unwrap(),
            &b.view(|| false).unwrap(),
            transform,
            WarpPolicy {
                tolerance: 0.03,
                max_source_pixels: 400_000,
            },
            || false,
        )
        .unwrap();
        assert!(evidence.forward.compared_pixels > 0 && evidence.reverse.compared_pixels > 0);
        eprintln!(
            "oracle-perspective {id}: forward={}/{} reverse={}/{}",
            evidence.forward.matched_pixels,
            evidence.forward.compared_pixels,
            evidence.reverse.matched_pixels,
            evidence.reverse.compared_pixels
        );
        for radius in [1, 2, 3] {
            let filtered = rrrah_dedup::warp::verify_projective_filtered(
                &a.view(|| false).unwrap(),
                &b.view(|| false).unwrap(),
                transform,
                WarpPolicy {
                    tolerance: 0.03,
                    max_source_pixels: 400_000,
                },
                rrrah_dedup::warp::ColorFilterPolicy {
                    filter: rrrah_dedup::warp::FilterPolicy {
                        radius,
                        max_sample_pairs: 20_000_000,
                    },
                    color_space: rrrah_dedup::warp::FilterColorSpace::LinearSrgb,
                },
                || false,
            )
            .unwrap();
            assert_eq!(filtered.strict, evidence);
            eprintln!(
                "oracle-filtered {id} radius={radius}: forward={}/{} reverse={}/{}",
                filtered.filtered.forward.matched_pixels,
                filtered.filtered.forward.compared_pixels,
                filtered.filtered.reverse.matched_pixels,
                filtered.filtered.reverse.compared_pixels
            );
        }
        drop(a);
        drop(b);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn filtered_projective_unrelated_pairs_and_lifecycle() {
    use rrrah_dedup::{
        local_scan::{LocalFileError, compare_local_files_projective_filtered},
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, WarpError},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let ids = [2414, 2418, 2883, 5025, 1425, 5495];
    let mut p = policy();
    p.matching.max_distance = 64;
    p.geometry.tolerance = 2.;
    p.geometry.min_inliers = 10;
    p.pixels.tolerance = 0.03;
    p.minimum_coverage_fraction = 0.3;
    p.minimum_matched_fraction = 0.9;
    let filter = ColorFilterPolicy {
        filter: FilterPolicy {
            radius: 1,
            max_sample_pairs: 4_000_000,
        },
        color_space: FilterColorSpace::LinearSrgb,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    for (i, left) in ids.iter().enumerate() {
        for right in &ids[i + 1..] {
            // Two views of the same garden have no independent unrelated-scene label.
            if (*left, *right) == (2414, 2418) {
                continue;
            }
            let a = DecodeRequest::new(root.join(format!("{left}-base.png")));
            let b = DecodeRequest::new(root.join(format!("{right}-base.png")));
            let evidence =
                compare_local_files_projective_filtered(&a, &b, p, filter, &budget, || false).unwrap();
            assert!(
                !evidence.candidate,
                "false candidate {left}/{right}: {evidence:?}"
            );
            assert_eq!(budget.used(), 0);
            let registered = rrrah_dedup::local_scan::compare_local_files_projective_registered(
                &a,
                &b,
                p,
                filter,
                rrrah_dedup::warp::ProjectiveRegistrationPolicy {
                    radius: 1,
                    stride: 8,
                    rounds: 128,
                    max_sample_pairs: 64_000_000,
                },
                &budget,
                || false,
            )
            .unwrap();
            assert!(
                !registered.candidate,
                "registered false candidate {left}/{right}: {registered:?}"
            );
            assert_eq!(budget.used(), 0);
        }
    }
    let a = DecodeRequest::new(root.join("2414-base.png"));
    let mut insufficient = filter;
    insufficient.filter.max_sample_pairs = 0;
    assert!(matches!(
        compare_local_files_projective_filtered(&a, &a, p, insufficient, &budget, || false),
        Err(LocalFileError::Pixels(WarpError::Budget))
    ));
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        compare_local_files_projective_filtered(&a, &a, p, filter, &budget, || budget.peak() > 0),
        Err(LocalFileError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    let retry = compare_local_files_projective_filtered(&a, &a, p, filter, &budget, || false).unwrap();
    assert!(retry.candidate);
    assert!(retry.filtered.is_some());
    assert_eq!(budget.used(), 0);
}

#[test]
fn registration_policy_is_validated_before_source_io() {
    use rrrah_dedup::{
        local_scan::{LocalFileError, compare_local_files_projective_registered},
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy, WarpError},
    };
    let missing = DecodeRequest::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/absent-registration-source.png"),
    );
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let filter = ColorFilterPolicy {
        filter: FilterPolicy {
            radius: 1,
            max_sample_pairs: 4_000_000,
        },
        color_space: FilterColorSpace::LinearSrgb,
    };
    for (stride, rounds) in [(0, 1), (1, 0), (1, 257)] {
        let result = compare_local_files_projective_registered(
            &missing,
            &missing,
            policy(),
            filter,
            ProjectiveRegistrationPolicy {
                radius: 0,
                stride,
                rounds,
                max_sample_pairs: 4_000_000,
            },
            &budget,
            || false,
        );
        assert!(matches!(result, Err(LocalFileError::Pixels(WarpError::Invalid))));
        assert_eq!(budget.used(), 0);
        assert_eq!(budget.peak(), 0);
    }
}

#[test]
fn registration_phase_cancel_and_budget_return_no_partial_file_candidate() {
    use rrrah_dedup::{
        local_scan::{LocalFileError, compare_local_files_projective_registered},
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy, WarpError},
    };
    let request = DecodeRequest::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout/2414-base.png"),
    );
    let budget = MemoryBudget::new(64 * 1024 * 1024);
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
        rounds: 1,
        max_sample_pairs: 2_000_000,
    };
    let calls = std::cell::Cell::new(0);
    let refusal = compare_local_files_projective_registered(
        &request,
        &request,
        policy(),
        filter,
        ProjectiveRegistrationPolicy {
            max_sample_pairs: 0,
            ..registration
        },
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    );
    assert!(matches!(refusal, Err(LocalFileError::Pixels(WarpError::Budget))));
    assert_eq!(budget.used(), 0);
    // Zero work reaches registration's first sampled grid point. An additional
    // hundred callbacks occur inside registration, before full pixel verification.
    let checkpoint = calls.get() + 100;
    let current = std::cell::Cell::new(0);
    let cancelled = compare_local_files_projective_registered(
        &request,
        &request,
        policy(),
        filter,
        registration,
        &budget,
        || {
            current.set(current.get() + 1);
            current.get() == checkpoint
        },
    );
    assert!(matches!(cancelled, Err(LocalFileError::Cancelled)));
    assert_eq!(current.get(), checkpoint);
    assert_eq!(budget.used(), 0);
    let retry = compare_local_files_projective_registered(
        &request,
        &request,
        policy(),
        filter,
        registration,
        &budget,
        || false,
    )
    .unwrap();
    assert!(retry.candidate);
    assert!(retry.registered_transform.is_some());
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_projective_collection_finds_derivative_and_rejects_other_scene() {
    use rrrah_dedup::{
        local_collection::scan_projective_local_collection,
        local_index::FileFeatureBudgets,
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let files = vec![
        (1, DecodeRequest::new(root.join("photos-heldout/5495-base.png"))),
        (
            2,
            DecodeRequest::new(root.join("photos-projective/5495-perspective.png")),
        ),
        (3, DecodeRequest::new(root.join("photos-heldout/2883-base.png"))),
    ];
    let mut p = policy();
    p.matching.max_distance = 64;
    p.geometry.tolerance = 2.;
    p.geometry.min_inliers = 10;
    p.pixels.tolerance = 0.03;
    p.minimum_coverage_fraction = 0.3;
    p.minimum_matched_fraction = 0.9;
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
    let limits = FileFeatureBudgets {
        max_files: 3,
        max_features: 96,
        max_hits: 100_000,
        max_pair_counts: 3,
        max_pairs: 3,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let report =
        scan_projective_local_collection(files, p, filter, registration, limits, &budget, || false).unwrap();
    assert_eq!(report.analysed, vec![1, 2, 3]);
    assert!(report.file_issues.is_empty());
    assert!(report.local.issues.is_empty());
    assert!(report.local.source_issues.is_empty());
    let candidates: Vec<_> = report
        .local
        .pairs
        .iter()
        .filter(|pair| pair.evidence.candidate)
        .map(|pair| (pair.left, pair.right))
        .collect();
    assert_eq!(candidates, vec![(1, 2)]);
    assert!(report.pixel_verification_pairs >= 1);
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_projective_collection_mutation_cancel_and_limits() {
    use rrrah_dedup::{
        local_collection::scan_projective_local_collection,
        local_index::FileFeatureBudgets,
        local_scan::LocalFileError,
        scan::ScanError,
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy, WarpError},
    };
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let source =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout/2414-base.png");
    let mut files: Vec<_> = (1..=3)
        .map(|id| {
            let path = dir.path().join(format!("{id}.png"));
            std::fs::copy(&source, &path).unwrap();
            (id, DecodeRequest::new(path))
        })
        .collect();
    let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(7));
    for (_, request) in &mut files {
        request.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 7));
    }
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
        rounds: 1,
        max_sample_pairs: 2_000_000,
    };
    let limits = FileFeatureBudgets {
        max_files: 3,
        max_features: 96,
        max_hits: 100_000,
        max_pair_counts: 3,
        max_pairs: 3,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let baseline = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(baseline.local.pairs.len(), 3);
    assert!(baseline.local.pairs.iter().all(|p| p.evidence.candidate));
    assert_eq!(budget.used(), 0);
    let last = calls.get();
    let current = std::cell::Cell::new(0);
    let final_calls = std::cell::Cell::new(0);
    let generation_result = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            final_calls.set(final_calls.get() + 1);
            if final_calls.get() == last {
                generation.store(8, std::sync::atomic::Ordering::Release);
            }
            false
        },
    );
    assert_eq!(final_calls.get(), last);
    assert!(matches!(generation_result, Err(ScanError::Cancelled)));
    assert_eq!(budget.used(), 0);
    generation.store(7, std::sync::atomic::Ordering::Release);
    let retry = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(retry.local.pairs.len(), 3);
    assert!(retry.local.pairs.iter().all(|pair| pair.evidence.candidate));
    assert_eq!(budget.used(), 0);

    // The final two calls are the per-source post-check and report check;
    // last-2 is the final source hash's check immediately before stamp validation.
    let late_calls = std::cell::Cell::new(0);
    let late_changed = std::cell::Cell::new(false);
    let late_path = files[2].1.path.clone();
    let late_report = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            late_calls.set(late_calls.get() + 1);
            if late_calls.get() == last - 2 {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&late_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                late_changed.set(true);
            }
            false
        },
    )
    .unwrap();
    assert!(late_changed.get());
    assert_eq!(late_report.analysed, vec![1, 2]);
    assert!(late_report.local.source_issues.iter().any(|(id, _)| *id == 3));
    assert_eq!(late_report.local.pairs.len(), 1);
    assert_eq!(
        (late_report.local.pairs[0].left, late_report.local.pairs[0].right),
        (1, 2)
    );
    assert!(late_report.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
    std::fs::copy(&source, &late_path).unwrap();
    assert!(matches!(
        scan_projective_local_collection(
            files.clone(),
            policy(),
            filter,
            registration,
            limits,
            &budget,
            || {
                current.set(current.get() + 1);
                current.get() == last
            }
        ),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        scan_projective_local_collection(
            files.clone(),
            policy(),
            filter,
            registration,
            FileFeatureBudgets {
                max_pairs: 0,
                ..limits
            },
            &budget,
            || false
        ),
        Err(ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    let refused = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        ProjectiveRegistrationPolicy {
            max_sample_pairs: 0,
            ..registration
        },
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert!(refused.local.pairs.is_empty());
    assert_eq!(refused.local.issues.len(), 3);
    assert!(
        refused
            .local
            .issues
            .iter()
            .all(|(_, _, e)| matches!(e, LocalFileError::Pixels(WarpError::Budget)))
    );
    assert_eq!(budget.used(), 0);
    let changed = std::cell::Cell::new(false);
    let changed_path = files[1].1.path.clone();
    let report =
        scan_projective_local_collection(files, policy(), filter, registration, limits, &budget, || {
            if budget.used() > 0 && !changed.replace(true) {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&changed_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
            }
            false
        })
        .unwrap();
    assert!(changed.get());
    assert_eq!(report.analysed, vec![1, 3]);
    assert!(report.local.source_issues.iter().any(|(id, _)| *id == 2));
    assert_eq!(report.local.pairs.len(), 1);
    assert_eq!((report.local.pairs[0].left, report.local.pairs[0].right), (1, 3));
    assert!(report.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_projective_collection_matches_all_pairs_on_six_source_corpus() {
    use rrrah_dedup::{
        local_collection::scan_projective_local_collection,
        local_index::FileFeatureBudgets,
        local_scan::compare_local_files_projective_registered,
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let files: Vec<_> = [2414, 2418, 2883, 5025, 1425, 5495]
        .into_iter()
        .enumerate()
        .flat_map(|(i, id)| {
            [
                (
                    2 * i as u64,
                    DecodeRequest::new(root.join(format!("photos-heldout/{id}-base.png"))),
                ),
                (
                    2 * i as u64 + 1,
                    DecodeRequest::new(root.join(format!("photos-projective/{id}-perspective.png"))),
                ),
            ]
        })
        .collect();
    let mut p = policy();
    p.matching.max_distance = 64;
    p.geometry.tolerance = 2.;
    p.geometry.min_inliers = 10;
    p.pixels.tolerance = 0.03;
    p.minimum_coverage_fraction = 0.3;
    p.minimum_matched_fraction = 0.9;
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
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut exhaustive = std::collections::BTreeSet::new();
    for (i, (left, a)) in files.iter().enumerate() {
        for (right, b) in &files[i + 1..] {
            let evidence =
                compare_local_files_projective_registered(a, b, p, filter, registration, &budget, || false)
                    .unwrap();
            if evidence.candidate {
                exhaustive.insert((*left, *right));
            }
            assert_eq!(budget.used(), 0);
        }
    }
    for i in 0..6 {
        assert!(
            exhaustive.contains(&(2 * i, 2 * i + 1)),
            "missing authored positive {i}"
        );
    }
    let report = scan_projective_local_collection(
        files,
        p,
        filter,
        registration,
        FileFeatureBudgets {
            max_files: 12,
            max_features: 384,
            max_hits: 200_000,
            max_pair_counts: 66,
            max_pairs: 66,
        },
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.analysed.len(), 12);
    assert!(report.local.issues.is_empty());
    assert!(report.file_issues.is_empty());
    assert!(report.local.source_issues.is_empty());
    let indexed: std::collections::BTreeSet<_> = report
        .local
        .pairs
        .iter()
        .filter(|pair| pair.evidence.candidate)
        .map(|pair| (pair.left, pair.right))
        .collect();
    assert_eq!(indexed, exhaustive);
    assert_eq!(budget.used(), 0);
}

#[test]
fn recursive_projective_collection_preserves_aliases_errors_and_requests() {
    use rrrah_dedup::{
        local_collection::{
            ProjectiveCollectionPolicy, scan_projective_local_collection_roots,
            scan_projective_local_collection_roots_with_requests,
        },
        local_index::FileFeatureBudgets,
        scan::ScanError,
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy},
    };
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let source =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout/2414-base.png");
    std::fs::copy(&source, dir.path().join("a.png")).unwrap();
    std::fs::copy(&source, nested.join("b.png")).unwrap();
    std::fs::write(dir.path().join("bad.png"), b"invalid image").unwrap();
    #[cfg(unix)]
    std::fs::hard_link(dir.path().join("a.png"), nested.join("alias.png")).unwrap();
    let roots = [dir.path().to_path_buf(), nested];
    let traversal = rrrah_dedup::exact::Options::default();
    let p = ProjectiveCollectionPolicy {
        local: policy(),
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 1,
                max_sample_pairs: 4_000_000,
            },
            color_space: FilterColorSpace::LinearSrgb,
        },
        registration: ProjectiveRegistrationPolicy {
            radius: 1,
            stride: 8,
            rounds: 1,
            max_sample_pairs: 2_000_000,
        },
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 96,
            max_hits: 100_000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let report = scan_projective_local_collection_roots(&roots, &traversal, p, &budget, || false).unwrap();
    assert_eq!(report.files.len(), 3);
    assert_eq!(report.indexed.file_issues.len(), 1);
    assert_eq!(report.indexed.local.pairs.len(), 1);
    assert!(report.indexed.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
    #[cfg(unix)]
    assert!(!report.aliases.is_empty());
    let selected = scan_projective_local_collection_roots_with_requests(
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
    assert!(selected.indexed.analysed.is_empty());
    assert_eq!(selected.indexed.file_issues.len(), 3);
    assert_eq!(budget.used(), 0);
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    assert!(matches!(
        scan_projective_local_collection_roots_with_requests(
            &roots,
            &traversal,
            p,
            &fresh,
            |_| DecodeRequest::new("substituted.png"),
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(fresh.peak(), 0);
    let mut insufficient = p;
    insufficient.budgets.max_files = 2;
    assert!(matches!(
        scan_projective_local_collection_roots(&roots, &traversal, insufficient, &fresh, || false),
        Err(ScanError::Budget)
    ));
    assert_eq!(fresh.peak(), 0);
    use rrrah_dedup::{
        local_collection::{
            scan_projective_local_collection_roots_spatial,
            scan_projective_local_collection_roots_spatial_with_requests,
        },
        local_scan::SpatialFeaturePolicy,
    };
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 2,
    };
    let spatial =
        scan_projective_local_collection_roots_spatial(&roots, &traversal, p, grid, &budget, || false)
            .unwrap();
    assert_eq!(spatial.files.len(), report.files.len());
    assert_eq!(spatial.indexed.file_issues.len(), 1);
    assert_eq!(spatial.indexed.local.pairs.len(), 1);
    assert!(spatial.indexed.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
    #[cfg(unix)]
    assert_eq!(spatial.aliases.len(), report.aliases.len());
    let selected = scan_projective_local_collection_roots_spatial_with_requests(
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
    assert!(selected.indexed.analysed.is_empty());
    assert_eq!(selected.indexed.file_issues.len(), 3);
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        scan_projective_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            p,
            grid,
            &fresh,
            |_| DecodeRequest::new("substituted.png"),
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(fresh.peak(), 0);
    let calls = std::cell::Cell::new(0);
    assert!(matches!(
        scan_projective_local_collection_roots_spatial_with_requests(
            &roots,
            &traversal,
            p,
            SpatialFeaturePolicy { columns: 0, ..grid },
            &fresh,
            |path| {
                calls.set(calls.get() + 1);
                DecodeRequest::new(path)
            },
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(calls.get(), 0);
    assert_eq!(fresh.peak(), 0);
    assert!(matches!(
        scan_projective_local_collection_roots_spatial(
            &roots,
            &traversal,
            insufficient,
            grid,
            &fresh,
            || false
        ),
        Err(ScanError::Budget)
    ));
    assert_eq!(fresh.peak(), 0);
    assert!(matches!(
        scan_projective_local_collection_roots_spatial(&roots, &traversal, p, grid, &budget, || true),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local_collection::{
            ProjectiveSampledCollectionPolicy, scan_projective_local_collection_roots_sampled,
            scan_projective_local_collection_roots_sampled_with_requests,
        },
        local_scan::ProjectiveSampledFilePolicy,
    };
    let sampled = ProjectiveSampledCollectionPolicy {
        search: ProjectiveSampledFilePolicy {
            local: p.local,
            filter: p.filter,
            registration: p.registration,
            spatial: Some(grid),
            sampling: ProjectiveSamplingPolicy {
                trials: 256,
                seed: 17,
            },
        },
        budgets: p.budgets,
    };
    let r = scan_projective_local_collection_roots_sampled(&roots, &traversal, sampled, &budget, || false)
        .unwrap();
    assert_eq!(r.files.len(), 3);
    assert_eq!(r.indexed.file_issues.len(), 1);
    assert_eq!(r.indexed.local.pairs.len(), 1);
    assert!(r.indexed.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
    #[cfg(unix)]
    assert_eq!(r.aliases.len(), report.aliases.len());
    let r = scan_projective_local_collection_roots_sampled_with_requests(
        &roots,
        &traversal,
        sampled,
        &budget,
        |path| {
            let mut r = DecodeRequest::new(path);
            r.image_index = 1;
            r
        },
        || false,
    )
    .unwrap();
    assert!(r.indexed.analysed.is_empty());
    assert_eq!(r.indexed.file_issues.len(), 3);
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        scan_projective_local_collection_roots_sampled_with_requests(
            &roots,
            &traversal,
            sampled,
            &fresh,
            |_| DecodeRequest::new("substituted.png"),
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(fresh.peak(), 0);
    let mut invalid = sampled;
    invalid.search.sampling.seed = 0;
    calls.set(0);
    assert!(matches!(
        scan_projective_local_collection_roots_sampled_with_requests(
            &roots,
            &traversal,
            invalid,
            &fresh,
            |path| {
                calls.set(calls.get() + 1);
                DecodeRequest::new(path)
            },
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(calls.get(), 0);
    assert_eq!(fresh.peak(), 0);
    assert!(matches!(
        scan_projective_local_collection_roots_sampled(&roots, &traversal, sampled, &budget, || true),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn spatial_projective_registration_preserves_identity_and_admission() {
    use rrrah_dedup::{
        local_scan::{
            LocalFileError, SpatialFeaturePolicy, compare_local_files_projective_registered_spatial,
        },
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy},
    };
    let request = DecodeRequest::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout/2414-base.png"),
    );
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
        rounds: 1,
        max_sample_pairs: 2_000_000,
    };
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 2,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let evidence = compare_local_files_projective_registered_spatial(
        &request,
        &request,
        policy(),
        filter,
        registration,
        grid,
        &budget,
        || false,
    )
    .unwrap();
    assert!(evidence.candidate);
    assert!(evidence.correspondences.len() <= 32);
    assert!(evidence.filtered.is_some());
    assert_eq!(budget.used(), 0);
    let missing = DecodeRequest::new("missing-spatial-projective.png");
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    assert!(matches!(
        compare_local_files_projective_registered_spatial(
            &missing,
            &missing,
            policy(),
            filter,
            registration,
            SpatialFeaturePolicy { columns: 0, ..grid },
            &fresh,
            || false
        ),
        Err(LocalFileError::InvalidPolicy)
    ));
    assert_eq!(fresh.peak(), 0);
    assert!(matches!(
        compare_local_files_projective_registered_spatial(
            &request,
            &request,
            policy(),
            filter,
            registration,
            grid,
            &budget,
            || true
        ),
        Err(LocalFileError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn indexed_spatial_projective_matches_direct_pairs() {
    use rrrah_dedup::{
        local_collection::{ProjectiveCollectionPolicy, scan_projective_local_collection_spatial},
        local_index::FileFeatureBudgets,
        local_scan::{SpatialFeaturePolicy, compare_local_files_projective_registered_spatial},
        scan::ScanError,
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let files = vec![
        (1, DecodeRequest::new(root.join("2414-base.png"))),
        (2, DecodeRequest::new(root.join("2414-base.png"))),
        (3, DecodeRequest::new(root.join("2883-base.png"))),
    ];
    let config = ProjectiveCollectionPolicy {
        local: policy(),
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 1,
                max_sample_pairs: 4_000_000,
            },
            color_space: FilterColorSpace::LinearSrgb,
        },
        registration: ProjectiveRegistrationPolicy {
            radius: 1,
            stride: 8,
            rounds: 1,
            max_sample_pairs: 2_000_000,
        },
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 96,
            max_hits: 100_000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
    };
    let grid = SpatialFeaturePolicy {
        columns: 4,
        rows: 4,
        max_per_cell: 2,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut direct = std::collections::BTreeSet::new();
    for left in 0..files.len() {
        for right in left + 1..files.len() {
            let evidence = compare_local_files_projective_registered_spatial(
                &files[left].1,
                &files[right].1,
                config.local,
                config.filter,
                config.registration,
                grid,
                &budget,
                || false,
            )
            .unwrap();
            if evidence.candidate {
                direct.insert((files[left].0, files[right].0));
            }
            assert_eq!(budget.used(), 0);
        }
    }
    assert_eq!(direct, std::collections::BTreeSet::from([(1, 2)]));
    let report =
        scan_projective_local_collection_spatial(files.clone(), config, grid, &budget, || false).unwrap();
    assert!(report.file_issues.is_empty());
    assert!(report.local.issues.is_empty());
    assert!(report.local.source_issues.is_empty());
    let indexed = report
        .local
        .pairs
        .iter()
        .filter(|pair| pair.evidence.candidate)
        .map(|pair| (pair.left, pair.right))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(indexed, direct);
    assert_eq!(budget.used(), 0);
    let invalid = SpatialFeaturePolicy { columns: 0, ..grid };
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    assert!(matches!(
        scan_projective_local_collection_spatial(
            vec![(1, DecodeRequest::new("missing-spatial-collection.png"))],
            config,
            invalid,
            &fresh,
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(fresh.peak(), 0);
    assert!(matches!(
        scan_projective_local_collection_spatial(files, config, grid, &budget, || true),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
}

#[test]
fn sampled_projective_files_preserve_pixels_and_terminal_refusals() {
    use rrrah_dedup::{
        geometry::{GeometryError, ProjectiveSamplingPolicy},
        local_scan::{LocalFileError, ProjectiveSampledFilePolicy, compare_local_files_projective_sampled},
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let a = DecodeRequest::new(root.join("2414-base.png"));
    let b = DecodeRequest::new(root.join("2883-base.png"));
    let mut local = policy();
    local.extract.max_features = 128;
    local.geometry.max_points = 128;
    local.geometry.max_hypotheses = 256;
    local.matching.max_comparisons = 16_384;
    let p = ProjectiveSampledFilePolicy {
        local,
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 1,
                max_sample_pairs: 4_000_000,
            },
            color_space: FilterColorSpace::LinearSrgb,
        },
        registration: ProjectiveRegistrationPolicy {
            radius: 1,
            stride: 8,
            rounds: 1,
            max_sample_pairs: 2_000_000,
        },
        spatial: None,
        sampling: ProjectiveSamplingPolicy {
            trials: 256,
            seed: 17,
        },
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let evidence = compare_local_files_projective_sampled(&a, &a, p, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert!(evidence.candidate);
    assert_eq!(evidence.geometry.as_ref().unwrap().hypotheses, 256);
    assert!(evidence.pixels.is_some());
    assert!(evidence.filtered.is_some());
    assert_eq!(budget.used(), 0);
    let baseline_calls = calls.get();
    for stop in [1, baseline_calls / 2, baseline_calls] {
        let current = std::cell::Cell::new(0);
        let result = compare_local_files_projective_sampled(&a, &a, p, &budget, || {
            current.set(current.get() + 1);
            current.get() == stop
        });
        assert!(
            matches!(result, Err(LocalFileError::Cancelled)),
            "stop {stop}: {result:?}"
        );
        assert_eq!(budget.used(), 0);
    }
    assert!(
        !compare_local_files_projective_sampled(&a, &b, p, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
    let missing = DecodeRequest::new("missing-sampled-planar.png");
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    assert!(matches!(
        compare_local_files_projective_sampled(
            &missing,
            &missing,
            ProjectiveSampledFilePolicy {
                sampling: ProjectiveSamplingPolicy {
                    seed: 0,
                    ..p.sampling
                },
                ..p
            },
            &fresh,
            || false
        ),
        Err(LocalFileError::InvalidPolicy)
    ));
    assert_eq!(fresh.peak(), 0);
    assert!(matches!(
        compare_local_files_projective_sampled(
            &missing,
            &missing,
            ProjectiveSampledFilePolicy {
                sampling: ProjectiveSamplingPolicy {
                    trials: 257,
                    ..p.sampling
                },
                ..p
            },
            &fresh,
            || false
        ),
        Err(LocalFileError::Geometry(GeometryError::Budget))
    ));
    assert_eq!(fresh.peak(), 0);
    let empty = MemoryBudget::new(0);
    assert!(compare_local_files_projective_sampled(&a, &a, p, &empty, || false).is_err());
    assert_eq!(empty.used(), 0);
    assert!(
        compare_local_files_projective_sampled(&a, &a, p, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
    use rrrah_dedup::{
        local_collection::scan_projective_local_collection_sampled, local_index::FileFeatureBudgets,
        scan::ScanError,
    };
    let limits = FileFeatureBudgets {
        max_files: 3,
        max_features: 384,
        max_hits: 100_000,
        max_pair_counts: 3,
        max_pairs: 3,
    };
    let report = scan_projective_local_collection_sampled(
        vec![(1, a.clone()), (2, a.clone()), (3, b)],
        p,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert!(report.file_issues.is_empty());
    assert!(report.local.issues.is_empty());
    assert!(report.local.source_issues.is_empty());
    let candidates = report
        .local
        .pairs
        .iter()
        .filter(|pair| pair.evidence.candidate)
        .map(|pair| (pair.left, pair.right))
        .collect::<Vec<_>>();
    assert_eq!(candidates, vec![(1, 2)]);
    assert_eq!(budget.used(), 0);
    let invalid = ProjectiveSampledFilePolicy {
        sampling: ProjectiveSamplingPolicy {
            seed: 0,
            ..p.sampling
        },
        ..p
    };
    assert!(matches!(
        scan_projective_local_collection_sampled(vec![(1, missing)], invalid, limits, &fresh, || false),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(fresh.peak(), 0);
}

#[test]
fn sampled_projective_collection_matches_all_pairs_on_six_source_corpus() {
    use rrrah_dedup::{
        local_collection::scan_projective_local_collection_sampled,
        local_index::FileFeatureBudgets,
        local_scan::{ProjectiveSampledFilePolicy, compare_local_files_projective_sampled},
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let files: Vec<_> = [2414, 2418, 2883, 5025, 1425, 5495]
        .into_iter()
        .enumerate()
        .flat_map(|(i, id)| {
            [
                (
                    2 * i as u64,
                    DecodeRequest::new(root.join(format!("photos-heldout/{id}-base.png"))),
                ),
                (
                    2 * i as u64 + 1,
                    DecodeRequest::new(root.join(format!("photos-projective/{id}-perspective.png"))),
                ),
            ]
        })
        .collect();
    let mut p = policy();
    p.matching.max_distance = 64;
    p.geometry.tolerance = 2.;
    p.geometry.min_inliers = 10;
    p.pixels.tolerance = 0.03;
    p.minimum_coverage_fraction = 0.3;
    p.minimum_matched_fraction = 0.9;
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
    p.extract.max_features = 500;
    p.geometry.max_points = 500;
    p.geometry.max_hypotheses = 2048;
    p.matching.max_comparisons = 250_000;
    let search = ProjectiveSampledFilePolicy {
        local: p,
        filter,
        registration,
        spatial: None,
        sampling: rrrah_dedup::geometry::ProjectiveSamplingPolicy {
            trials: 2048,
            seed: 0x1234abcd,
        },
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut exhaustive = std::collections::BTreeSet::new();
    for (i, (left, a)) in files.iter().enumerate() {
        for (right, b) in &files[i + 1..] {
            let evidence = compare_local_files_projective_sampled(a, b, search, &budget, || false).unwrap();
            // Even IDs are independently labelled source captures. Garden views 0/2
            // are related and intentionally excluded from the unrelated-scene label.
            if left % 2 == 0 && right % 2 == 0 && (*left, *right) != (0, 2) {
                assert!(
                    !evidence.candidate,
                    "sampled false candidate on unrelated captures {left}/{right}: {evidence:?}"
                );
            }
            if evidence.candidate {
                exhaustive.insert((*left, *right));
            }
            assert_eq!(budget.used(), 0);
        }
    }
    for i in 0..6 {
        assert!(
            exhaustive.contains(&(2 * i, 2 * i + 1)),
            "missing authored positive {i}"
        );
    }
    let report = scan_projective_local_collection_sampled(
        files,
        search,
        FileFeatureBudgets {
            max_files: 12,
            max_features: 6000,
            max_hits: 20_000_000,
            max_pair_counts: 66,
            max_pairs: 66,
        },
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.analysed.len(), 12);
    assert!(report.local.issues.is_empty());
    assert!(report.file_issues.is_empty());
    assert!(report.local.source_issues.is_empty());
    let indexed: std::collections::BTreeSet<_> = report
        .local
        .pairs
        .iter()
        .filter(|pair| pair.evidence.candidate)
        .map(|pair| (pair.left, pair.right))
        .collect();
    assert_eq!(indexed, exhaustive);
    assert_eq!(budget.used(), 0);
}

#[test]
fn sampled_projective_collection_mutation_cancel_and_limits() {
    use rrrah_dedup::{
        local_index::FileFeatureBudgets,
        local_scan::LocalFileError,
        scan::ScanError,
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy, WarpError},
    };
    fn scan_projective_local_collection(
        files: impl IntoIterator<Item = (u64, DecodeRequest)>,
        local: LocalFilePolicy,
        filter: ColorFilterPolicy,
        registration: ProjectiveRegistrationPolicy,
        limits: FileFeatureBudgets,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<
        rrrah_dedup::local_collection::LocalCollectionReport<rrrah_dedup::local_scan::ProjectiveFileEvidence>,
        ScanError,
    > {
        rrrah_dedup::local_collection::scan_projective_local_collection_sampled(
            files,
            rrrah_dedup::local_scan::ProjectiveSampledFilePolicy {
                local,
                filter,
                registration,
                spatial: None,
                sampling: rrrah_dedup::geometry::ProjectiveSamplingPolicy {
                    trials: 256,
                    seed: 17,
                },
            },
            limits,
            budget,
            cancel,
        )
    }
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let source =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout/2414-base.png");
    let mut files: Vec<_> = (1..=3)
        .map(|id| {
            let path = dir.path().join(format!("{id}.png"));
            std::fs::copy(&source, &path).unwrap();
            (id, DecodeRequest::new(path))
        })
        .collect();
    let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(7));
    for (_, request) in &mut files {
        request.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 7));
    }
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
        rounds: 1,
        max_sample_pairs: 2_000_000,
    };
    let limits = FileFeatureBudgets {
        max_files: 3,
        max_features: 96,
        max_hits: 100_000,
        max_pair_counts: 3,
        max_pairs: 3,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let baseline = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(baseline.local.pairs.len(), 3);
    assert!(baseline.local.pairs.iter().all(|p| p.evidence.candidate));
    assert_eq!(budget.used(), 0);
    let last = calls.get();
    let current = std::cell::Cell::new(0);
    let final_calls = std::cell::Cell::new(0);
    let generation_result = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            final_calls.set(final_calls.get() + 1);
            if final_calls.get() == last {
                generation.store(8, std::sync::atomic::Ordering::Release);
            }
            false
        },
    );
    assert_eq!(final_calls.get(), last);
    assert!(matches!(generation_result, Err(ScanError::Cancelled)));
    assert_eq!(budget.used(), 0);
    generation.store(7, std::sync::atomic::Ordering::Release);
    let retry = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(retry.local.pairs.len(), 3);
    assert!(retry.local.pairs.iter().all(|pair| pair.evidence.candidate));
    assert_eq!(budget.used(), 0);

    // The final two calls are the per-source post-check and report check;
    // last-2 is the final source hash's check immediately before stamp validation.
    let late_calls = std::cell::Cell::new(0);
    let late_changed = std::cell::Cell::new(false);
    let late_path = files[2].1.path.clone();
    let late_report = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            late_calls.set(late_calls.get() + 1);
            if late_calls.get() == last - 2 {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&late_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                late_changed.set(true);
            }
            false
        },
    )
    .unwrap();
    assert!(late_changed.get());
    assert_eq!(late_report.analysed, vec![1, 2]);
    assert!(late_report.local.source_issues.iter().any(|(id, _)| *id == 3));
    assert_eq!(late_report.local.pairs.len(), 1);
    assert_eq!(
        (late_report.local.pairs[0].left, late_report.local.pairs[0].right),
        (1, 2)
    );
    assert!(late_report.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
    std::fs::copy(&source, &late_path).unwrap();
    assert!(matches!(
        scan_projective_local_collection(
            files.clone(),
            policy(),
            filter,
            registration,
            limits,
            &budget,
            || {
                current.set(current.get() + 1);
                current.get() == last
            }
        ),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        scan_projective_local_collection(
            files.clone(),
            policy(),
            filter,
            registration,
            FileFeatureBudgets {
                max_pairs: 0,
                ..limits
            },
            &budget,
            || false
        ),
        Err(ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    let refused = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        ProjectiveRegistrationPolicy {
            max_sample_pairs: 0,
            ..registration
        },
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert!(refused.local.pairs.is_empty());
    assert_eq!(refused.local.issues.len(), 3);
    assert!(
        refused
            .local
            .issues
            .iter()
            .all(|(_, _, e)| matches!(e, LocalFileError::Pixels(WarpError::Budget)))
    );
    assert_eq!(budget.used(), 0);
    let changed = std::cell::Cell::new(false);
    let changed_path = files[1].1.path.clone();
    let report =
        scan_projective_local_collection(files, policy(), filter, registration, limits, &budget, || {
            if budget.used() > 0 && !changed.replace(true) {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&changed_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
            }
            false
        })
        .unwrap();
    assert!(changed.get());
    assert_eq!(report.analysed, vec![1, 3]);
    assert!(report.local.source_issues.iter().any(|(id, _)| *id == 2));
    assert_eq!(report.local.pairs.len(), 1);
    assert_eq!((report.local.pairs[0].left, report.local.pairs[0].right), (1, 3));
    assert!(report.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
}

#[test]
fn anchored_projective_file_objective_never_bypasses_pixel_acceptance() {
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local_scan::{LocalFileError, ProjectiveAnchoredFilePolicy, compare_local_files_projective_anchored},
        warp::{
            ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricPolicy,
            ProjectiveRegistrationPolicy, ProjectiveRegistrationTrustPolicy, WarpError,
        },
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let a = DecodeRequest::new(root.join("2414-base.png"));
    let light = DecodeRequest::new(root.join("2414-brightness.png"));
    let other = DecodeRequest::new(root.join("2883-base.png"));
    let mut local = policy();
    local.extract.max_features = 128;
    local.geometry.max_points = 128;
    local.geometry.max_hypotheses = 256;
    local.matching.max_comparisons = 16_384;
    local.matching.max_distance = 64;
    local.pixels.tolerance = 0.03;
    let registration = ProjectiveRegistrationTrustPolicy {
        registration: ProjectiveRegistrationPolicy {
            radius: 1,
            stride: 8,
            rounds: 8,
            max_sample_pairs: 4_000_000,
        },
        photometric: PhotometricPolicy {
            residual: local.pixels,
            minimum_samples: 16,
            minimum_variance: 1e-5,
            minimum_gain: 0.2,
            maximum_gain: 5.,
            maximum_offset: 0.1,
        },
        maximum_corner_shift: 1.,
    };
    let p = ProjectiveAnchoredFilePolicy {
        local,
        registration,
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 1,
                max_sample_pairs: 4_000_000,
            },
            color_space: FilterColorSpace::LinearSrgb,
        },
        spatial: None,
        sampling: Some(ProjectiveSamplingPolicy {
            trials: 256,
            seed: 17,
        }),
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    assert!(
        compare_local_files_projective_anchored(&a, &a, p, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
    let e = compare_local_files_projective_anchored(&a, &light, p, &budget, || false).unwrap();
    assert!(e.geometry.is_some());
    assert!(e.registered_transform.is_some());
    assert!(e.filtered.is_some());
    assert!(!e.candidate, "nuisance fit bypassed residual policy: {e:?}");
    assert_eq!(budget.used(), 0);
    assert!(
        !compare_local_files_projective_anchored(&a, &other, p, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
    let missing = DecodeRequest::new("missing-anchored.png");
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    let invalid = ProjectiveAnchoredFilePolicy {
        registration: ProjectiveRegistrationTrustPolicy {
            maximum_corner_shift: f64::NAN,
            ..registration
        },
        ..p
    };
    assert!(compare_local_files_projective_anchored(&missing, &missing, invalid, &fresh, || false).is_err());
    assert_eq!(fresh.peak(), 0);
    let insufficient = ProjectiveAnchoredFilePolicy {
        registration: ProjectiveRegistrationTrustPolicy {
            registration: ProjectiveRegistrationPolicy {
                max_sample_pairs: 0,
                ..registration.registration
            },
            ..registration
        },
        ..p
    };
    assert!(matches!(
        compare_local_files_projective_anchored(&a, &a, insufficient, &budget, || false),
        Err(LocalFileError::Pixels(WarpError::Budget))
    ));
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        compare_local_files_projective_anchored(&a, &a, p, &budget, || true),
        Err(LocalFileError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    assert!(
        compare_local_files_projective_anchored(&a, &a, p, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
    use rrrah_dedup::{
        local_collection::{
            ProjectiveAnchoredCollectionPolicy, scan_projective_local_collection_anchored,
            scan_projective_local_collection_roots_anchored,
            scan_projective_local_collection_roots_anchored_with_requests,
        },
        local_index::FileFeatureBudgets,
        scan::ScanError,
    };
    let config = ProjectiveAnchoredCollectionPolicy {
        search: p,
        budgets: FileFeatureBudgets {
            max_files: 4,
            max_features: 512,
            max_hits: 200_000,
            max_pair_counts: 6,
            max_pairs: 6,
        },
    };
    let files = vec![(1, a.clone()), (2, a.clone()), (3, light), (4, other)];
    let mut direct = std::collections::BTreeSet::new();
    for left in 0..files.len() {
        for right in left + 1..files.len() {
            let e =
                compare_local_files_projective_anchored(&files[left].1, &files[right].1, p, &budget, || {
                    false
                })
                .unwrap();
            if e.candidate {
                direct.insert((files[left].0, files[right].0));
            }
            assert_eq!(budget.used(), 0);
        }
    }
    assert_eq!(direct, std::collections::BTreeSet::from([(1, 2)]));
    let indexed = scan_projective_local_collection_anchored(files, config, &budget, || false).unwrap();
    assert!(indexed.file_issues.is_empty());
    assert!(indexed.local.issues.is_empty());
    assert!(indexed.local.source_issues.is_empty());
    assert_eq!(
        indexed
            .local
            .pairs
            .iter()
            .filter(|x| x.evidence.candidate)
            .map(|x| (x.left, x.right))
            .collect::<std::collections::BTreeSet<_>>(),
        direct
    );
    assert_eq!(budget.used(), 0);
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(&a.path, dir.path().join("a.png")).unwrap();
    std::fs::copy(&a.path, dir.path().join("b.png")).unwrap();
    std::fs::write(dir.path().join("bad.png"), b"corrupt").unwrap();
    #[cfg(unix)]
    std::fs::hard_link(dir.path().join("a.png"), dir.path().join("alias.png")).unwrap();
    let roots = [dir.path().to_path_buf()];
    let traversal = rrrah_dedup::exact::Options::default();
    let r = scan_projective_local_collection_roots_anchored(&roots, &traversal, config, &budget, || false)
        .unwrap();
    assert_eq!(r.files.len(), 3);
    assert_eq!(r.indexed.file_issues.len(), 1);
    assert_eq!(r.indexed.local.pairs.len(), 1);
    assert!(r.indexed.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
    #[cfg(unix)]
    assert!(!r.aliases.is_empty());
    let r = scan_projective_local_collection_roots_anchored_with_requests(
        &roots,
        &traversal,
        config,
        &budget,
        |path| {
            let mut r = DecodeRequest::new(path);
            r.image_index = 1;
            r
        },
        || false,
    )
    .unwrap();
    assert!(r.indexed.analysed.is_empty());
    assert_eq!(r.indexed.file_issues.len(), 3);
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        scan_projective_local_collection_roots_anchored_with_requests(
            &roots,
            &traversal,
            config,
            &fresh,
            |_| DecodeRequest::new("substitute.png"),
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(fresh.peak(), 0);
    let calls = std::cell::Cell::new(0);
    let bad_config = ProjectiveAnchoredCollectionPolicy {
        search: invalid,
        ..config
    };
    assert!(matches!(
        scan_projective_local_collection_roots_anchored_with_requests(
            &roots,
            &traversal,
            bad_config,
            &fresh,
            |path| {
                calls.set(calls.get() + 1);
                DecodeRequest::new(path)
            },
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(calls.get(), 0);
    assert_eq!(fresh.peak(), 0);
}

#[test]
fn anchored_projective_collection_matches_all_pairs_on_six_source_corpus() {
    use rrrah_dedup::{
        local_collection::{ProjectiveAnchoredCollectionPolicy, scan_projective_local_collection_anchored},
        local_index::FileFeatureBudgets,
        local_scan::{ProjectiveAnchoredFilePolicy, compare_local_files_projective_anchored},
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let files: Vec<_> = [2414, 2418, 2883, 5025, 1425, 5495]
        .into_iter()
        .enumerate()
        .flat_map(|(i, id)| {
            [
                (
                    2 * i as u64,
                    DecodeRequest::new(root.join(format!("photos-heldout/{id}-base.png"))),
                ),
                (
                    2 * i as u64 + 1,
                    DecodeRequest::new(root.join(format!("photos-projective/{id}-perspective.png"))),
                ),
            ]
        })
        .collect();
    let mut p = policy();
    p.matching.max_distance = 64;
    p.geometry.tolerance = 2.;
    p.geometry.min_inliers = 10;
    p.pixels.tolerance = 0.03;
    p.minimum_coverage_fraction = 0.3;
    p.minimum_matched_fraction = 0.9;
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
    p.extract.max_features = 500;
    p.geometry.max_points = 500;
    p.geometry.max_hypotheses = 2048;
    p.matching.max_comparisons = 250_000;
    let registration = rrrah_dedup::warp::ProjectiveRegistrationTrustPolicy {
        registration,
        photometric: rrrah_dedup::warp::PhotometricPolicy {
            residual: p.pixels,
            minimum_samples: 16,
            minimum_variance: 1e-5,
            minimum_gain: 0.2,
            maximum_gain: 5.,
            maximum_offset: 0.1,
        },
        maximum_corner_shift: 1.,
    };
    let search = ProjectiveAnchoredFilePolicy {
        local: p,
        filter,
        registration,
        spatial: None,
        sampling: Some(rrrah_dedup::geometry::ProjectiveSamplingPolicy {
            trials: 2048,
            seed: 0x1234abcd,
        }),
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut exhaustive = std::collections::BTreeSet::new();
    for (i, (left, a)) in files.iter().enumerate() {
        for (right, b) in &files[i + 1..] {
            let evidence = compare_local_files_projective_anchored(a, b, search, &budget, || false).unwrap();
            // Even IDs are independently labelled source captures. Garden views 0/2
            // are related and intentionally excluded from the unrelated-scene label.
            if left % 2 == 0 && right % 2 == 0 && (*left, *right) != (0, 2) {
                assert!(
                    !evidence.candidate,
                    "sampled false candidate on unrelated captures {left}/{right}: {evidence:?}"
                );
            }
            if evidence.candidate {
                exhaustive.insert((*left, *right));
            }
            assert_eq!(budget.used(), 0);
        }
    }
    for i in 0..6 {
        assert!(
            exhaustive.contains(&(2 * i, 2 * i + 1)),
            "missing authored positive {i}"
        );
    }
    let report = scan_projective_local_collection_anchored(
        files,
        ProjectiveAnchoredCollectionPolicy {
            search,
            budgets: FileFeatureBudgets {
                max_files: 12,
                max_features: 6000,
                max_hits: 20_000_000,
                max_pair_counts: 66,
                max_pairs: 66,
            },
        },
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.analysed.len(), 12);
    assert!(report.local.issues.is_empty());
    assert!(report.file_issues.is_empty());
    assert!(report.local.source_issues.is_empty());
    let indexed: std::collections::BTreeSet<_> = report
        .local
        .pairs
        .iter()
        .filter(|pair| pair.evidence.candidate)
        .map(|pair| (pair.left, pair.right))
        .collect();
    assert_eq!(indexed, exhaustive);
    assert_eq!(budget.used(), 0);
}

#[test]
fn anchored_projective_collection_mutation_cancel_and_limits() {
    use rrrah_dedup::{
        local_index::FileFeatureBudgets,
        local_scan::LocalFileError,
        scan::ScanError,
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy, WarpError},
    };
    fn scan_projective_local_collection(
        files: impl IntoIterator<Item = (u64, DecodeRequest)>,
        local: LocalFilePolicy,
        filter: ColorFilterPolicy,
        registration: ProjectiveRegistrationPolicy,
        limits: FileFeatureBudgets,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<
        rrrah_dedup::local_collection::LocalCollectionReport<rrrah_dedup::local_scan::ProjectiveFileEvidence>,
        ScanError,
    > {
        let trust = rrrah_dedup::warp::ProjectiveRegistrationTrustPolicy {
            registration,
            photometric: rrrah_dedup::warp::PhotometricPolicy {
                residual: local.pixels,
                minimum_samples: 16,
                minimum_variance: 1e-5,
                minimum_gain: 0.2,
                maximum_gain: 5.,
                maximum_offset: 0.1,
            },
            maximum_corner_shift: 1.,
        };
        rrrah_dedup::local_collection::scan_projective_local_collection_anchored(
            files,
            rrrah_dedup::local_collection::ProjectiveAnchoredCollectionPolicy {
                search: rrrah_dedup::local_scan::ProjectiveAnchoredFilePolicy {
                    local,
                    filter,
                    registration: trust,
                    spatial: None,
                    sampling: Some(rrrah_dedup::geometry::ProjectiveSamplingPolicy {
                        trials: 256,
                        seed: 17,
                    }),
                },
                budgets: limits,
            },
            budget,
            cancel,
        )
    }
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let source =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout/2414-base.png");
    let mut files: Vec<_> = (1..=3)
        .map(|id| {
            let path = dir.path().join(format!("{id}.png"));
            std::fs::copy(&source, &path).unwrap();
            (id, DecodeRequest::new(path))
        })
        .collect();
    let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(7));
    for (_, request) in &mut files {
        request.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 7));
    }
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
        rounds: 1,
        max_sample_pairs: 2_000_000,
    };
    let limits = FileFeatureBudgets {
        max_files: 3,
        max_features: 96,
        max_hits: 100_000,
        max_pair_counts: 3,
        max_pairs: 3,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let baseline = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(baseline.local.pairs.len(), 3);
    assert!(baseline.local.pairs.iter().all(|p| p.evidence.candidate));
    assert_eq!(budget.used(), 0);
    let last = calls.get();
    let current = std::cell::Cell::new(0);
    let final_calls = std::cell::Cell::new(0);
    let generation_result = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            final_calls.set(final_calls.get() + 1);
            if final_calls.get() == last {
                generation.store(8, std::sync::atomic::Ordering::Release);
            }
            false
        },
    );
    assert_eq!(final_calls.get(), last);
    assert!(matches!(generation_result, Err(ScanError::Cancelled)));
    assert_eq!(budget.used(), 0);
    generation.store(7, std::sync::atomic::Ordering::Release);
    let retry = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(retry.local.pairs.len(), 3);
    assert!(retry.local.pairs.iter().all(|pair| pair.evidence.candidate));
    assert_eq!(budget.used(), 0);

    // The final two calls are the per-source post-check and report check;
    // last-2 is the final source hash's check immediately before stamp validation.
    let late_calls = std::cell::Cell::new(0);
    let late_changed = std::cell::Cell::new(false);
    let late_path = files[2].1.path.clone();
    let late_report = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            late_calls.set(late_calls.get() + 1);
            if late_calls.get() == last - 2 {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&late_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                late_changed.set(true);
            }
            false
        },
    )
    .unwrap();
    assert!(late_changed.get());
    assert_eq!(late_report.analysed, vec![1, 2]);
    assert!(late_report.local.source_issues.iter().any(|(id, _)| *id == 3));
    assert_eq!(late_report.local.pairs.len(), 1);
    assert_eq!(
        (late_report.local.pairs[0].left, late_report.local.pairs[0].right),
        (1, 2)
    );
    assert!(late_report.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
    std::fs::copy(&source, &late_path).unwrap();
    assert!(matches!(
        scan_projective_local_collection(
            files.clone(),
            policy(),
            filter,
            registration,
            limits,
            &budget,
            || {
                current.set(current.get() + 1);
                current.get() == last
            }
        ),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        scan_projective_local_collection(
            files.clone(),
            policy(),
            filter,
            registration,
            FileFeatureBudgets {
                max_pairs: 0,
                ..limits
            },
            &budget,
            || false
        ),
        Err(ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    let refused = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        ProjectiveRegistrationPolicy {
            max_sample_pairs: 0,
            ..registration
        },
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert!(refused.local.pairs.is_empty());
    assert_eq!(refused.local.issues.len(), 3);
    assert!(
        refused
            .local
            .issues
            .iter()
            .all(|(_, _, e)| matches!(e, LocalFileError::Pixels(WarpError::Budget)))
    );
    assert_eq!(budget.used(), 0);
    let changed = std::cell::Cell::new(false);
    let changed_path = files[1].1.path.clone();
    let report =
        scan_projective_local_collection(files, policy(), filter, registration, limits, &budget, || {
            if budget.used() > 0 && !changed.replace(true) {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&changed_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
            }
            false
        })
        .unwrap();
    assert!(changed.get());
    assert_eq!(report.analysed, vec![1, 3]);
    assert!(report.local.source_issues.iter().any(|(id, _)| *id == 2));
    assert_eq!(report.local.pairs.len(), 1);
    assert_eq!((report.local.pairs[0].left, report.local.pairs[0].right), (1, 3));
    assert!(report.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
}

#[test]
fn projective_photometric_file_fit_keeps_strict_errors_and_refusals() {
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local_scan::{
            LocalFileError, ProjectiveAnchoredFilePolicy, ProjectivePhotometricFilePolicy,
            compare_local_files_projective_photometric,
        },
        warp::{
            ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy,
            ProjectiveRegistrationPolicy, ProjectiveRegistrationTrustPolicy, WarpError,
        },
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let a = DecodeRequest::new(root.join("2414-base.png"));
    let light = DecodeRequest::new(root.join("2414-brightness.png"));
    let other = DecodeRequest::new(root.join("2883-base.png"));
    let mut local = policy();
    local.extract.max_features = 128;
    local.geometry.max_points = 128;
    local.geometry.max_hypotheses = 256;
    local.matching.max_comparisons = 16_384;
    local.matching.max_distance = 64;
    local.pixels.tolerance = 0.03;
    local.minimum_matched_fraction = 0.95;
    let photometric = PhotometricPolicy {
        residual: local.pixels,
        minimum_samples: 16,
        minimum_variance: 1e-5,
        minimum_gain: 0.2,
        maximum_gain: 5.,
        maximum_offset: 0.1,
    };
    let registration = ProjectiveRegistrationTrustPolicy {
        registration: ProjectiveRegistrationPolicy {
            radius: 1,
            stride: 8,
            rounds: 8,
            max_sample_pairs: 4_000_000,
        },
        photometric,
        maximum_corner_shift: 1.,
    };
    let search = ProjectiveAnchoredFilePolicy {
        local,
        registration,
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 1,
                max_sample_pairs: 12_000_000,
            },
            color_space: FilterColorSpace::EncodedSrgb,
        },
        spatial: None,
        sampling: Some(ProjectiveSamplingPolicy {
            trials: 256,
            seed: 17,
        }),
    };
    let p = ProjectivePhotometricFilePolicy {
        search,
        photometric,
        fit_mode: PhotometricFitMode::RejectOutsidePolicy,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    assert!(
        compare_local_files_projective_photometric(&a, &a, p, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
    let e = compare_local_files_projective_photometric(&a, &light, p, &budget, || false).unwrap();
    assert!(e.candidate, "{e:?}");
    assert!(e.fit_failure.is_none());
    let strict = &e.unfitted.unwrap().strict.forward;
    assert!(strict.matched_pixels < strict.compared_pixels / 2);
    assert_eq!(budget.used(), 0);
    assert!(
        !compare_local_files_projective_photometric(&a, &other, p, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
    use rrrah_dedup::{
        local_collection::{
            ProjectivePhotometricCollectionPolicy, scan_projective_local_collection_photometric,
        },
        local_index::FileFeatureBudgets,
    };
    let files = vec![
        (1, a.clone()),
        (2, a.clone()),
        (3, light.clone()),
        (4, other.clone()),
    ];
    let mut direct = std::collections::BTreeSet::new();
    for left in 0..files.len() {
        for right in left + 1..files.len() {
            if compare_local_files_projective_photometric(&files[left].1, &files[right].1, p, &budget, || {
                false
            })
            .unwrap()
            .candidate
            {
                direct.insert((files[left].0, files[right].0));
            }
            assert_eq!(budget.used(), 0);
        }
    }
    assert_eq!(direct, std::collections::BTreeSet::from([(1, 2), (1, 3), (2, 3)]));
    let collection = ProjectivePhotometricCollectionPolicy {
        search: p,
        budgets: FileFeatureBudgets {
            max_files: 4,
            max_features: 512,
            max_hits: 200_000,
            max_pair_counts: 6,
            max_pairs: 6,
        },
    };
    let report =
        scan_projective_local_collection_photometric(files.clone(), collection, &budget, || false).unwrap();
    assert!(report.local.issues.is_empty(), "{:?}", report.local.issues);
    let indexed = report
        .local
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(indexed, direct);
    assert_eq!(budget.used(), 0);
    assert!(scan_projective_local_collection_photometric(files, collection, &budget, || true).is_err());
    assert_eq!(budget.used(), 0);
    use rrrah_dedup::local_collection::{
        scan_projective_local_collection_roots_photometric,
        scan_projective_local_collection_roots_photometric_with_requests,
    };
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(&a.path, dir.path().join("a.png")).unwrap();
    std::fs::copy(&light.path, dir.path().join("light.png")).unwrap();
    std::fs::write(dir.path().join("bad.png"), b"corrupt").unwrap();
    #[cfg(unix)]
    std::fs::hard_link(dir.path().join("a.png"), dir.path().join("alias.png")).unwrap();
    let roots = [dir.path().to_path_buf(), dir.path().to_path_buf()];
    let traversal = rrrah_dedup::exact::Options::default();
    let r =
        scan_projective_local_collection_roots_photometric(&roots, &traversal, collection, &budget, || false)
            .unwrap();
    assert_eq!(r.files.len(), 3);
    assert_eq!(r.indexed.file_issues.len(), 1);
    assert_eq!(r.indexed.local.pairs.len(), 1);
    assert!(r.indexed.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
    #[cfg(unix)]
    assert!(!r.aliases.is_empty());
    let r = scan_projective_local_collection_roots_photometric_with_requests(
        &roots,
        &traversal,
        collection,
        &budget,
        |path| {
            let mut r = DecodeRequest::new(path);
            r.image_index = 1;
            r
        },
        || false,
    )
    .unwrap();
    assert!(r.indexed.analysed.is_empty());
    assert_eq!(r.indexed.file_issues.len(), 3);
    assert_eq!(budget.used(), 0);
    assert!(
        scan_projective_local_collection_roots_photometric_with_requests(
            &roots,
            &traversal,
            collection,
            &budget,
            |_| DecodeRequest::new("substitute.png"),
            || false
        )
        .is_err()
    );
    assert_eq!(budget.used(), 0);
    let calls = std::cell::Cell::new(0);
    let bad = ProjectivePhotometricCollectionPolicy {
        search: ProjectivePhotometricFilePolicy {
            photometric: PhotometricPolicy {
                minimum_gain: f64::NAN,
                ..photometric
            },
            ..p
        },
        ..collection
    };
    assert!(
        scan_projective_local_collection_roots_photometric_with_requests(
            &roots,
            &traversal,
            bad,
            &budget,
            |path| {
                calls.set(calls.get() + 1);
                DecodeRequest::new(path)
            },
            || false
        )
        .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(budget.used(), 0);
    let missing = DecodeRequest::new("missing-projective-photometric.png");
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    let invalid = ProjectivePhotometricFilePolicy {
        photometric: PhotometricPolicy {
            minimum_gain: f64::NAN,
            ..photometric
        },
        ..p
    };
    assert!(
        compare_local_files_projective_photometric(&missing, &missing, invalid, &fresh, || false).is_err()
    );
    assert_eq!(fresh.peak(), 0);
    assert!(matches!(
        compare_local_files_projective_photometric(&a, &a, p, &budget, || true),
        Err(LocalFileError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    let limited = ProjectivePhotometricFilePolicy {
        search: ProjectiveAnchoredFilePolicy {
            filter: ColorFilterPolicy {
                filter: FilterPolicy {
                    max_sample_pairs: 0,
                    ..search.filter.filter
                },
                ..search.filter
            },
            ..search
        },
        ..p
    };
    assert!(matches!(
        compare_local_files_projective_photometric(&a, &a, limited, &budget, || false),
        Err(LocalFileError::Pixels(WarpError::Budget))
    ));
    assert_eq!(budget.used(), 0);
    assert!(
        compare_local_files_projective_photometric(&a, &a, p, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn projective_photometric_collection_mutation_cancel_and_limits() {
    use rrrah_dedup::{
        local_index::FileFeatureBudgets,
        local_scan::LocalFileError,
        scan::ScanError,
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy, WarpError},
    };
    fn scan_projective_local_collection(
        files: impl IntoIterator<Item = (u64, DecodeRequest)>,
        local: LocalFilePolicy,
        filter: ColorFilterPolicy,
        registration: ProjectiveRegistrationPolicy,
        limits: FileFeatureBudgets,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<
        rrrah_dedup::local_collection::LocalCollectionReport<
            rrrah_dedup::local_scan::ProjectivePhotometricFileEvidence,
        >,
        ScanError,
    > {
        let trust = rrrah_dedup::warp::ProjectiveRegistrationTrustPolicy {
            registration,
            photometric: rrrah_dedup::warp::PhotometricPolicy {
                residual: local.pixels,
                minimum_samples: 16,
                minimum_variance: 1e-5,
                minimum_gain: 0.2,
                maximum_gain: 5.,
                maximum_offset: 0.1,
            },
            maximum_corner_shift: 1.,
        };
        let search = rrrah_dedup::local_scan::ProjectiveAnchoredFilePolicy {
            local,
            filter,
            registration: trust,
            spatial: None,
            sampling: Some(rrrah_dedup::geometry::ProjectiveSamplingPolicy {
                trials: 256,
                seed: 17,
            }),
        };
        let search = rrrah_dedup::local_scan::ProjectivePhotometricFilePolicy {
            search,
            photometric: trust.photometric,
            fit_mode: rrrah_dedup::warp::PhotometricFitMode::RejectOutsidePolicy,
        };
        rrrah_dedup::local_collection::scan_projective_local_collection_photometric(
            files,
            rrrah_dedup::local_collection::ProjectivePhotometricCollectionPolicy {
                search,
                budgets: limits,
            },
            budget,
            cancel,
        )
    }
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let source =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout/2414-base.png");
    let mut files: Vec<_> = (1..=3)
        .map(|id| {
            let path = dir.path().join(format!("{id}.png"));
            std::fs::copy(&source, &path).unwrap();
            (id, DecodeRequest::new(path))
        })
        .collect();
    let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(7));
    for (_, request) in &mut files {
        request.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 7));
    }
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
        rounds: 1,
        max_sample_pairs: 2_000_000,
    };
    let limits = FileFeatureBudgets {
        max_files: 3,
        max_features: 96,
        max_hits: 100_000,
        max_pair_counts: 3,
        max_pairs: 3,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let baseline = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(baseline.local.pairs.len(), 3);
    assert!(baseline.local.pairs.iter().all(|p| p.evidence.candidate));
    assert_eq!(budget.used(), 0);
    let last = calls.get();
    let current = std::cell::Cell::new(0);
    let final_calls = std::cell::Cell::new(0);
    let generation_result = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            final_calls.set(final_calls.get() + 1);
            if final_calls.get() == last {
                generation.store(8, std::sync::atomic::Ordering::Release);
            }
            false
        },
    );
    assert_eq!(final_calls.get(), last);
    assert!(matches!(generation_result, Err(ScanError::Cancelled)));
    assert_eq!(budget.used(), 0);
    generation.store(7, std::sync::atomic::Ordering::Release);
    let retry = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(retry.local.pairs.len(), 3);
    assert!(retry.local.pairs.iter().all(|pair| pair.evidence.candidate));
    assert_eq!(budget.used(), 0);

    // The final two calls are the per-source post-check and report check;
    // last-2 is the final source hash's check immediately before stamp validation.
    let late_calls = std::cell::Cell::new(0);
    let late_changed = std::cell::Cell::new(false);
    let late_path = files[2].1.path.clone();
    let late_report = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            late_calls.set(late_calls.get() + 1);
            if late_calls.get() == last - 2 {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&late_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                late_changed.set(true);
            }
            false
        },
    )
    .unwrap();
    assert!(late_changed.get());
    assert_eq!(late_report.analysed, vec![1, 2]);
    assert!(late_report.local.source_issues.iter().any(|(id, _)| *id == 3));
    assert_eq!(late_report.local.pairs.len(), 1);
    assert_eq!(
        (late_report.local.pairs[0].left, late_report.local.pairs[0].right),
        (1, 2)
    );
    assert!(late_report.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
    std::fs::copy(&source, &late_path).unwrap();
    assert!(matches!(
        scan_projective_local_collection(
            files.clone(),
            policy(),
            filter,
            registration,
            limits,
            &budget,
            || {
                current.set(current.get() + 1);
                current.get() == last
            }
        ),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        scan_projective_local_collection(
            files.clone(),
            policy(),
            filter,
            registration,
            FileFeatureBudgets {
                max_pairs: 0,
                ..limits
            },
            &budget,
            || false
        ),
        Err(ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    let refused = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        ProjectiveRegistrationPolicy {
            max_sample_pairs: 0,
            ..registration
        },
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert!(refused.local.pairs.is_empty());
    assert_eq!(refused.local.issues.len(), 3);
    assert!(
        refused
            .local
            .issues
            .iter()
            .all(|(_, _, e)| matches!(e, LocalFileError::Pixels(WarpError::Budget)))
    );
    assert_eq!(budget.used(), 0);
    let changed = std::cell::Cell::new(false);
    let changed_path = files[1].1.path.clone();
    let report =
        scan_projective_local_collection(files, policy(), filter, registration, limits, &budget, || {
            if budget.used() > 0 && !changed.replace(true) {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&changed_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
            }
            false
        })
        .unwrap();
    assert!(changed.get());
    assert_eq!(report.analysed, vec![1, 3]);
    assert!(report.local.source_issues.iter().any(|(id, _)| *id == 2));
    assert_eq!(report.local.pairs.len(), 1);
    assert_eq!((report.local.pairs[0].left, report.local.pairs[0].right), (1, 3));
    assert!(report.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
}

#[test]
fn combined_perspective_light_photos_require_all_six_positives_and_unrelated_rejection() {
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local_scan::{
            ProjectiveAnchoredFilePolicy, ProjectivePhotometricFilePolicy,
            compare_local_files_projective_photometric,
        },
        warp::{
            ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy,
            ProjectiveRegistrationPolicy, ProjectiveRegistrationTrustPolicy,
        },
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let ids = [2414, 2418, 2883, 5025, 1425, 5495];
    let mut local = policy();
    local.matching.max_distance = 64;
    local.geometry.tolerance = 2.;
    local.geometry.min_inliers = 10;
    local.pixels.tolerance = 0.03;
    local.minimum_coverage_fraction = 0.3;
    local.minimum_matched_fraction = 0.9;
    local.extract.max_features = 500;
    local.geometry.max_points = 500;
    local.geometry.max_hypotheses = 2048;
    local.matching.max_comparisons = 250_000;
    let photometric = PhotometricPolicy {
        residual: local.pixels,
        minimum_samples: 16,
        minimum_variance: 1e-5,
        minimum_gain: 0.2,
        maximum_gain: 5.,
        maximum_offset: 0.1,
    };
    let search = ProjectiveAnchoredFilePolicy {
        local,
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 1,
                max_sample_pairs: 12_000_000,
            },
            color_space: FilterColorSpace::EncodedSrgb,
        },
        registration: ProjectiveRegistrationTrustPolicy {
            registration: ProjectiveRegistrationPolicy {
                radius: 1,
                stride: 8,
                rounds: 128,
                max_sample_pairs: 64_000_000,
            },
            photometric,
            maximum_corner_shift: 1.,
        },
        spatial: None,
        sampling: Some(ProjectiveSamplingPolicy {
            trials: 2048,
            seed: 0x1234abcd,
        }),
    };
    let policy = ProjectivePhotometricFilePolicy {
        search,
        photometric,
        fit_mode: PhotometricFitMode::ConstrainedLeastSquares,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut failures = Vec::new();
    for id in ids {
        let a = DecodeRequest::new(root.join(format!("photos-heldout/{id}-base.png")));
        let b = DecodeRequest::new(root.join(format!("photos-projective-light/{id}-perspective-light.png")));
        if id == 2414 {
            let refusal = compare_local_files_projective_photometric(
                &a,
                &b,
                ProjectivePhotometricFilePolicy {
                    fit_mode: PhotometricFitMode::RejectOutsidePolicy,
                    ..policy
                },
                &budget,
                || false,
            )
            .unwrap();
            assert!(!refusal.candidate);
            assert!(matches!(
                refusal.fit_failure,
                Some(rrrah_dedup::warp::PhotometricFitFailure::OutsidePolicy { .. })
            ));
            assert_eq!(budget.used(), 0);
        }
        match compare_local_files_projective_photometric(&a, &b, policy, &budget, || false) {
            Ok(e) => {
                eprintln!(
                    "combined-positive {id}: candidate={} matches={} fit_failure={:?}",
                    e.candidate,
                    e.correspondences.len(),
                    e.fit_failure
                );
                if !e.candidate || e.pixels.is_none() || e.unfitted.is_none() || e.fit_failure.is_some() {
                    failures.push(format!("required combined case {id}: geometry={} registration={} fit_failure={:?} fitted={:?}",e.geometry.is_some(),e.registered_transform.is_some(),e.fit_failure,e.pixels));
                }
            }
            Err(error) => failures.push(format!("required combined case {id}: {error}")),
        }
        assert_eq!(budget.used(), 0);
    }
    for (i, left) in ids.iter().enumerate() {
        for right in &ids[i + 1..] {
            if (*left == 2414 && *right == 2418) || (*left == 2418 && *right == 2414) {
                continue;
            }
            let a = DecodeRequest::new(root.join(format!("photos-heldout/{left}-base.png")));
            let b = DecodeRequest::new(
                root.join(format!("photos-projective-light/{right}-perspective-light.png")),
            );
            match compare_local_files_projective_photometric(&a, &b, policy, &budget, || false) {
                Ok(e) => {
                    if e.candidate {
                        failures.push(format!("unrelated combined {left}/{right} accepted"));
                    }
                }
                Err(error) => failures.push(format!("unrelated combined {left}/{right}: {error}")),
            }
            assert_eq!(budget.used(), 0);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn registration_portfolio_files_preserve_both_residuals_and_atomic_refusals() {
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local_scan::{
            LocalFileError, ProjectivePortfolioFilePolicy, compare_local_files_projective_portfolio,
        },
        warp::{
            ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricPolicy,
            ProjectiveRegistrationPolicy, ProjectiveRegistrationPortfolioPolicy,
            ProjectiveRegistrationTrustPolicy, WarpError,
        },
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let a = DecodeRequest::new(root.join("2414-base.png"));
    let other = DecodeRequest::new(root.join("2883-base.png"));
    let mut local = policy();
    local.extract.max_features = 128;
    local.geometry.max_points = 128;
    local.geometry.max_hypotheses = 256;
    local.matching.max_comparisons = 16_384;
    local.matching.max_distance = 64;
    local.pixels.tolerance = 0.03;
    let lane = ProjectiveRegistrationPolicy {
        radius: 1,
        stride: 8,
        rounds: 8,
        max_sample_pairs: 4_000_000,
    };
    let trust = ProjectiveRegistrationTrustPolicy {
        registration: lane,
        photometric: PhotometricPolicy {
            residual: local.pixels,
            minimum_samples: 16,
            minimum_variance: 1e-5,
            minimum_gain: 0.2,
            maximum_gain: 5.,
            maximum_offset: 0.1,
        },
        maximum_corner_shift: 1.,
    };
    let p = ProjectivePortfolioFilePolicy {
        local,
        filter: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 1,
                max_sample_pairs: 8_000_000,
            },
            color_space: FilterColorSpace::LinearSrgb,
        },
        registration: ProjectiveRegistrationPortfolioPolicy {
            anchored: trust,
            unanchored: lane,
            max_sample_pairs: 8_000_000,
        },
        spatial: None,
        sampling: Some(ProjectiveSamplingPolicy {
            trials: 256,
            seed: 17,
        }),
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let e = compare_local_files_projective_portfolio(&a, &a, p, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert!(e.candidate);
    let pixels = e.pixels.unwrap();
    for lane in [&pixels.anchored, &pixels.unanchored] {
        assert_eq!(
            lane.filtered.forward.matched_pixels,
            lane.filtered.forward.compared_pixels
        );
        assert_eq!(
            lane.filtered.reverse.matched_pixels,
            lane.filtered.reverse.compared_pixels
        );
    }
    assert_eq!(budget.used(), 0);
    assert!(
        !compare_local_files_projective_portfolio(&a, &other, p, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
    for checkpoint in [1, calls.get() / 2, calls.get()] {
        let count = std::cell::Cell::new(0);
        assert!(matches!(
            compare_local_files_projective_portfolio(&a, &a, p, &budget, || {
                count.set(count.get() + 1);
                count.get() == checkpoint
            }),
            Err(LocalFileError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    use rrrah_dedup::{
        local_collection::{ProjectivePortfolioCollectionPolicy, scan_projective_local_collection_portfolio},
        local_index::FileFeatureBudgets,
    };
    let files = vec![(1, a.clone()), (2, a.clone()), (3, other.clone())];
    let mut direct = std::collections::BTreeSet::new();
    for left in 0..files.len() {
        for right in left + 1..files.len() {
            if compare_local_files_projective_portfolio(&files[left].1, &files[right].1, p, &budget, || false)
                .unwrap()
                .candidate
            {
                direct.insert((files[left].0, files[right].0));
            }
            assert_eq!(budget.used(), 0);
        }
    }
    assert_eq!(direct, std::collections::BTreeSet::from([(1, 2)]));
    let collection = ProjectivePortfolioCollectionPolicy {
        search: p,
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 384,
            max_hits: 100_000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
    };
    let report = scan_projective_local_collection_portfolio(files, collection, &budget, || false).unwrap();
    assert!(report.file_issues.is_empty());
    assert!(report.local.issues.is_empty());
    let indexed = report
        .local
        .pairs
        .iter()
        .filter(|p| p.evidence.candidate)
        .map(|p| (p.left, p.right))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(indexed, direct);
    assert_eq!(budget.used(), 0);
    use rrrah_dedup::local_collection::{
        scan_projective_local_collection_roots_portfolio,
        scan_projective_local_collection_roots_portfolio_with_requests,
    };
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(&a.path, dir.path().join("a.png")).unwrap();
    std::fs::copy(&a.path, dir.path().join("b.png")).unwrap();
    std::fs::write(dir.path().join("bad.png"), b"corrupt").unwrap();
    #[cfg(unix)]
    std::fs::hard_link(dir.path().join("a.png"), dir.path().join("alias.png")).unwrap();
    let roots = [dir.path().to_path_buf(), dir.path().to_path_buf()];
    let traversal = rrrah_dedup::exact::Options::default();
    let r =
        scan_projective_local_collection_roots_portfolio(&roots, &traversal, collection, &budget, || false)
            .unwrap();
    assert_eq!(r.files.len(), 3);
    assert_eq!(r.indexed.file_issues.len(), 1);
    assert_eq!(r.indexed.local.pairs.len(), 1);
    assert!(r.indexed.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
    #[cfg(unix)]
    assert!(!r.aliases.is_empty());
    let r = scan_projective_local_collection_roots_portfolio_with_requests(
        &roots,
        &traversal,
        collection,
        &budget,
        |path| {
            let mut r = DecodeRequest::new(path);
            r.image_index = 1;
            r
        },
        || false,
    )
    .unwrap();
    assert!(r.indexed.analysed.is_empty());
    assert_eq!(r.indexed.file_issues.len(), 3);
    assert_eq!(budget.used(), 0);
    assert!(
        scan_projective_local_collection_roots_portfolio_with_requests(
            &roots,
            &traversal,
            collection,
            &budget,
            |_| DecodeRequest::new("substitute.png"),
            || false
        )
        .is_err()
    );
    assert_eq!(budget.used(), 0);
    let factory_calls = std::cell::Cell::new(0);
    let bad_collection = ProjectivePortfolioCollectionPolicy {
        search: ProjectivePortfolioFilePolicy {
            registration: ProjectiveRegistrationPortfolioPolicy {
                max_sample_pairs: 7_999_999,
                ..p.registration
            },
            ..p
        },
        ..collection
    };
    assert!(matches!(
        scan_projective_local_collection_roots_portfolio_with_requests(
            &roots,
            &traversal,
            bad_collection,
            &budget,
            |path| {
                factory_calls.set(factory_calls.get() + 1);
                DecodeRequest::new(path)
            },
            || false
        ),
        Err(rrrah_dedup::scan::ScanError::Budget)
    ));
    assert_eq!(factory_calls.get(), 0);
    assert_eq!(budget.used(), 0);
    let missing = DecodeRequest::new("missing-portfolio.png");
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    let bad = ProjectivePortfolioFilePolicy {
        registration: ProjectiveRegistrationPortfolioPolicy {
            max_sample_pairs: 7_999_999,
            ..p.registration
        },
        ..p
    };
    assert!(matches!(
        compare_local_files_projective_portfolio(&missing, &missing, bad, &fresh, || false),
        Err(LocalFileError::Pixels(WarpError::Budget))
    ));
    assert_eq!(fresh.peak(), 0);
    let exhausted = ProjectivePortfolioFilePolicy {
        registration: ProjectiveRegistrationPortfolioPolicy {
            unanchored: ProjectiveRegistrationPolicy {
                max_sample_pairs: 0,
                ..lane
            },
            ..p.registration
        },
        ..p
    };
    assert!(matches!(
        compare_local_files_projective_portfolio(&a, &a, exhausted, &budget, || false),
        Err(LocalFileError::Pixels(WarpError::Budget))
    ));
    assert_eq!(budget.used(), 0);
    assert!(
        compare_local_files_projective_portfolio(&a, &a, p, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn registration_portfolio_collection_mutation_cancel_and_limits() {
    use rrrah_dedup::{
        local_index::FileFeatureBudgets,
        local_scan::LocalFileError,
        scan::ScanError,
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy, WarpError},
    };
    fn scan_projective_local_collection(
        files: impl IntoIterator<Item = (u64, DecodeRequest)>,
        local: LocalFilePolicy,
        filter: ColorFilterPolicy,
        registration: ProjectiveRegistrationPolicy,
        limits: FileFeatureBudgets,
        budget: &MemoryBudget,
        cancel: impl Fn() -> bool,
    ) -> Result<
        rrrah_dedup::local_collection::LocalCollectionReport<
            rrrah_dedup::local_scan::ProjectivePortfolioFileEvidence,
        >,
        ScanError,
    > {
        let trust = rrrah_dedup::warp::ProjectiveRegistrationTrustPolicy {
            registration,
            photometric: rrrah_dedup::warp::PhotometricPolicy {
                residual: local.pixels,
                minimum_samples: 16,
                minimum_variance: 1e-5,
                minimum_gain: 0.2,
                maximum_gain: 5.,
                maximum_offset: 0.1,
            },
            maximum_corner_shift: 1.,
        };
        let search = rrrah_dedup::local_scan::ProjectivePortfolioFilePolicy {
            local,
            filter,
            registration: rrrah_dedup::warp::ProjectiveRegistrationPortfolioPolicy {
                anchored: trust,
                unanchored: registration,
                max_sample_pairs: registration.max_sample_pairs.checked_mul(2).unwrap(),
            },
            spatial: None,
            sampling: Some(rrrah_dedup::geometry::ProjectiveSamplingPolicy {
                trials: 256,
                seed: 17,
            }),
        };
        rrrah_dedup::local_collection::scan_projective_local_collection_portfolio(
            files,
            rrrah_dedup::local_collection::ProjectivePortfolioCollectionPolicy {
                search,
                budgets: limits,
            },
            budget,
            cancel,
        )
    }
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let source =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout/2414-base.png");
    let mut files: Vec<_> = (1..=3)
        .map(|id| {
            let path = dir.path().join(format!("{id}.png"));
            std::fs::copy(&source, &path).unwrap();
            (id, DecodeRequest::new(path))
        })
        .collect();
    let generation = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(7));
    for (_, request) in &mut files {
        request.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 7));
    }
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
        rounds: 1,
        max_sample_pairs: 2_000_000,
    };
    let limits = FileFeatureBudgets {
        max_files: 3,
        max_features: 96,
        max_hits: 100_000,
        max_pair_counts: 3,
        max_pairs: 3,
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let calls = std::cell::Cell::new(0);
    let baseline = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(baseline.local.pairs.len(), 3);
    assert!(baseline.local.pairs.iter().all(|p| p.evidence.candidate));
    assert_eq!(budget.used(), 0);
    let last = calls.get();
    let current = std::cell::Cell::new(0);
    let final_calls = std::cell::Cell::new(0);
    let generation_result = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            final_calls.set(final_calls.get() + 1);
            if final_calls.get() == last {
                generation.store(8, std::sync::atomic::Ordering::Release);
            }
            false
        },
    );
    assert_eq!(final_calls.get(), last);
    assert!(matches!(generation_result, Err(ScanError::Cancelled)));
    assert_eq!(budget.used(), 0);
    generation.store(7, std::sync::atomic::Ordering::Release);
    let retry = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(retry.local.pairs.len(), 3);
    assert!(retry.local.pairs.iter().all(|pair| pair.evidence.candidate));
    assert_eq!(budget.used(), 0);

    // The final two calls are the per-source post-check and report check;
    // last-2 is the final source hash's check immediately before stamp validation.
    let late_calls = std::cell::Cell::new(0);
    let late_changed = std::cell::Cell::new(false);
    let late_path = files[2].1.path.clone();
    let late_report = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        registration,
        limits,
        &budget,
        || {
            late_calls.set(late_calls.get() + 1);
            if late_calls.get() == last - 2 {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&late_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                late_changed.set(true);
            }
            false
        },
    )
    .unwrap();
    assert!(late_changed.get());
    assert_eq!(late_report.analysed, vec![1, 2]);
    assert!(late_report.local.source_issues.iter().any(|(id, _)| *id == 3));
    assert_eq!(late_report.local.pairs.len(), 1);
    assert_eq!(
        (late_report.local.pairs[0].left, late_report.local.pairs[0].right),
        (1, 2)
    );
    assert!(late_report.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
    std::fs::copy(&source, &late_path).unwrap();
    assert!(matches!(
        scan_projective_local_collection(
            files.clone(),
            policy(),
            filter,
            registration,
            limits,
            &budget,
            || {
                current.set(current.get() + 1);
                current.get() == last
            }
        ),
        Err(ScanError::Cancelled)
    ));
    assert_eq!(budget.used(), 0);
    assert!(matches!(
        scan_projective_local_collection(
            files.clone(),
            policy(),
            filter,
            registration,
            FileFeatureBudgets {
                max_pairs: 0,
                ..limits
            },
            &budget,
            || false
        ),
        Err(ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    let refused = scan_projective_local_collection(
        files.clone(),
        policy(),
        filter,
        ProjectiveRegistrationPolicy {
            max_sample_pairs: 0,
            ..registration
        },
        limits,
        &budget,
        || false,
    )
    .unwrap();
    assert!(refused.local.pairs.is_empty());
    assert_eq!(refused.local.issues.len(), 3);
    assert!(
        refused
            .local
            .issues
            .iter()
            .all(|(_, _, e)| matches!(e, LocalFileError::Pixels(WarpError::Budget)))
    );
    assert_eq!(budget.used(), 0);
    let changed = std::cell::Cell::new(false);
    let changed_path = files[1].1.path.clone();
    let report =
        scan_projective_local_collection(files, policy(), filter, registration, limits, &budget, || {
            if budget.used() > 0 && !changed.replace(true) {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&changed_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
            }
            false
        })
        .unwrap();
    assert!(changed.get());
    assert_eq!(report.analysed, vec![1, 3]);
    assert!(report.local.source_issues.iter().any(|(id, _)| *id == 2));
    assert_eq!(report.local.pairs.len(), 1);
    assert_eq!((report.local.pairs[0].left, report.local.pairs[0].right), (1, 3));
    assert!(report.local.pairs[0].evidence.candidate);
    assert_eq!(budget.used(), 0);
}

#[test]
fn registration_portfolio_collection_matches_all_pairs_on_six_source_corpus() {
    use rrrah_dedup::{
        local_collection::{ProjectivePortfolioCollectionPolicy, scan_projective_local_collection_portfolio},
        local_index::FileFeatureBudgets,
        local_scan::{ProjectivePortfolioFilePolicy, compare_local_files_projective_portfolio},
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, ProjectiveRegistrationPolicy},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let files: Vec<_> = [2414, 2418, 2883, 5025, 1425, 5495]
        .into_iter()
        .enumerate()
        .flat_map(|(i, id)| {
            [
                (
                    2 * i as u64,
                    DecodeRequest::new(root.join(format!("photos-heldout/{id}-base.png"))),
                ),
                (
                    2 * i as u64 + 1,
                    DecodeRequest::new(root.join(format!("photos-projective/{id}-perspective.png"))),
                ),
            ]
        })
        .collect();
    let mut p = policy();
    p.matching.max_distance = 64;
    p.geometry.tolerance = 2.;
    p.geometry.min_inliers = 10;
    p.pixels.tolerance = 0.03;
    p.minimum_coverage_fraction = 0.3;
    p.minimum_matched_fraction = 0.9;
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
    p.extract.max_features = 500;
    p.geometry.max_points = 500;
    p.geometry.max_hypotheses = 2048;
    p.matching.max_comparisons = 250_000;
    let registration = rrrah_dedup::warp::ProjectiveRegistrationTrustPolicy {
        registration,
        photometric: rrrah_dedup::warp::PhotometricPolicy {
            residual: p.pixels,
            minimum_samples: 16,
            minimum_variance: 1e-5,
            minimum_gain: 0.2,
            maximum_gain: 5.,
            maximum_offset: 0.1,
        },
        maximum_corner_shift: 1.,
    };
    let registration = rrrah_dedup::warp::ProjectiveRegistrationPortfolioPolicy {
        anchored: registration,
        unanchored: registration.registration,
        max_sample_pairs: 128_000_000,
    };
    let search = ProjectivePortfolioFilePolicy {
        local: p,
        filter,
        registration,
        spatial: None,
        sampling: Some(rrrah_dedup::geometry::ProjectiveSamplingPolicy {
            trials: 2048,
            seed: 0x1234abcd,
        }),
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut exhaustive = std::collections::BTreeSet::new();
    for (i, (left, a)) in files.iter().enumerate() {
        for (right, b) in &files[i + 1..] {
            let evidence = compare_local_files_projective_portfolio(a, b, search, &budget, || false).unwrap();
            // Even IDs are independently labelled source captures. Garden views 0/2
            // are related and intentionally excluded from the unrelated-scene label.
            if left % 2 == 0 && right % 2 == 0 && (*left, *right) != (0, 2) {
                assert!(
                    !evidence.candidate,
                    "sampled false candidate on unrelated captures {left}/{right}: {evidence:?}"
                );
            }
            if evidence.candidate {
                exhaustive.insert((*left, *right));
            }
            assert_eq!(budget.used(), 0);
        }
    }
    for i in 0..6 {
        assert!(
            exhaustive.contains(&(2 * i, 2 * i + 1)),
            "missing authored positive {i}"
        );
    }
    let report = scan_projective_local_collection_portfolio(
        files,
        ProjectivePortfolioCollectionPolicy {
            search,
            budgets: FileFeatureBudgets {
                max_files: 12,
                max_features: 6000,
                max_hits: 20_000_000,
                max_pair_counts: 66,
                max_pairs: 66,
            },
        },
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(report.analysed.len(), 12);
    assert!(report.local.issues.is_empty());
    assert!(report.file_issues.is_empty());
    assert!(report.local.source_issues.is_empty());
    let indexed: std::collections::BTreeSet<_> = report
        .local
        .pairs
        .iter()
        .filter(|pair| pair.evidence.candidate)
        .map(|pair| (pair.left, pair.right))
        .collect();
    assert_eq!(indexed, exhaustive);
    assert_eq!(budget.used(), 0);
}

#[test]
fn pyramid_collection_and_nested_roots_match_direct_pairs_and_refusals() {
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local_collection::{
            ProjectivePyramidCollectionPolicy, scan_projective_local_collection_pyramid,
            scan_projective_local_collection_roots_pyramid,
        },
        local_index::FileFeatureBudgets,
        local_scan::{
            ProjectivePyramidPhotometricFilePolicy, compare_local_files_projective_pyramid_photometric,
        },
        scan::ScanError,
        warp::{ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy},
    };
    use std::{cell::Cell, collections::BTreeSet};
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let tree = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for (id, name, relative) in [
        (1, "2414-base.png", "a/deep/a.png"),
        (2, "2414-base.png", "b/deep/b.png"),
        (3, "5495-base.png", "c/deep/c.png"),
    ] {
        let path = tree.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::copy(source.join(name), &path).unwrap();
        files.push((id, DecodeRequest::new(path)));
    }
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
    let search = ProjectivePyramidPhotometricFilePolicy {
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
    let config = ProjectivePyramidCollectionPolicy {
        search,
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 1152,
            max_hits: 1_000_000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut direct = BTreeSet::new();
    for i in 0..3 {
        for j in i + 1..3 {
            let e = compare_local_files_projective_pyramid_photometric(
                &files[i].1,
                &files[j].1,
                search,
                &budget,
                || false,
            )
            .unwrap();
            if e.candidate {
                direct.insert((files[i].0, files[j].0));
            }
            assert_eq!(budget.used(), 0);
        }
    }
    assert_eq!(direct, BTreeSet::from([(1, 2)]));
    let calls = Cell::new(0);
    let report = scan_projective_local_collection_pyramid(files.clone(), config, &budget, || {
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
    let accepted = |r: &rrrah_dedup::local_collection::LocalCollectionReport<
        rrrah_dedup::local_scan::ProjectivePhotometricFileEvidence,
    >| {
        r.local
            .pairs
            .iter()
            .filter(|p| p.evidence.candidate)
            .map(|p| (p.left, p.right))
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(accepted(&report), direct);
    assert_eq!(budget.used(), 0);
    for stop in [1, checkpoints / 2, checkpoints] {
        calls.set(0);
        assert!(
            matches!(
                scan_projective_local_collection_pyramid(files.clone(), config, &budget, || {
                    calls.set(calls.get() + 1);
                    calls.get() >= stop
                }),
                Err(ScanError::Cancelled)
            ),
            "stop={stop}"
        );
        assert_eq!(budget.used(), 0);
    }
    let limited = ProjectivePyramidCollectionPolicy {
        budgets: FileFeatureBudgets {
            max_pairs: 0,
            ..config.budgets
        },
        ..config
    };
    assert!(matches!(
        scan_projective_local_collection_pyramid(files.clone(), limited, &budget, || false),
        Err(ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    let invalid = ProjectivePyramidCollectionPolicy {
        search: ProjectivePyramidPhotometricFilePolicy {
            max_levels: 0,
            ..search
        },
        ..config
    };
    assert!(matches!(
        scan_projective_local_collection_pyramid(files.clone(), invalid, &budget, || false),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(budget.used(), 0);
    files.reverse();
    assert_eq!(
        accepted(&scan_projective_local_collection_pyramid(files, config, &budget, || false).unwrap()),
        direct
    );
    assert_eq!(budget.used(), 0);
    let roots = vec![tree.path().to_path_buf(), tree.path().join("a")];
    let recursive = scan_projective_local_collection_roots_pyramid(
        &roots,
        &rrrah_dedup::exact::Options::default(),
        config,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(recursive.files.len(), 3);
    assert_eq!(
        recursive
            .indexed
            .local
            .pairs
            .iter()
            .filter(|p| p.evidence.candidate)
            .count(),
        1
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn complementary_files_preserve_both_searches_and_atomic_refusals() {
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local::LocalError,
        local_scan::{
            LocalFileError, ProjectiveComplementaryFilePolicy, ProjectivePortfolioFilePolicy,
            ProjectivePyramidPhotometricFilePolicy, compare_local_files_projective_complementary,
        },
        warp::{
            ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy,
            ProjectiveRegistrationPolicy, ProjectiveRegistrationPortfolioPolicy,
            ProjectiveRegistrationTrustPolicy, WarpError,
        },
    };
    use std::{cell::Cell, io::Write};
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let dir = tempfile::tempdir().unwrap();
    let a_path = dir.path().join("a.png");
    let b_path = dir.path().join("b.png");
    std::fs::copy(source.join("2414-base.png"), &a_path).unwrap();
    std::fs::copy(&a_path, &b_path).unwrap();
    let a = DecodeRequest::new(&a_path);
    let b = DecodeRequest::new(&b_path);
    let unrelated = DecodeRequest::new(source.join("5495-base.png"));
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
    let calls = Cell::new(0);
    let evidence = compare_local_files_projective_complementary(&a, &b, policy, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let checkpoints = calls.get();
    assert_eq!(evidence.accepted_searches, [true, true]);
    assert!(evidence.candidate);
    assert!(evidence.pyramid.pixels.is_some() && evidence.pyramid.unfitted.is_some());
    assert!(evidence.registration.pixels.is_some());
    assert_eq!(budget.used(), 0);
    assert!(
        !compare_local_files_projective_complementary(&a, &unrelated, policy, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
    let missing = DecodeRequest::new(dir.path().join("missing.png"));
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    let mut insufficient = policy;
    insufficient.max_total_comparisons -= 1;
    assert!(matches!(
        compare_local_files_projective_complementary(&missing, &missing, insufficient, &fresh, || false),
        Err(LocalFileError::Features(LocalError::Budget))
    ));
    assert_eq!(fresh.peak(), 0);
    insufficient = policy;
    insufficient.max_total_hypotheses -= 1;
    assert!(matches!(
        compare_local_files_projective_complementary(&missing, &missing, insufficient, &fresh, || false),
        Err(LocalFileError::Geometry(
            rrrah_dedup::geometry::GeometryError::Budget
        ))
    ));
    assert_eq!(fresh.peak(), 0);
    insufficient = policy;
    insufficient.max_total_sample_pairs -= 1;
    assert!(matches!(
        compare_local_files_projective_complementary(&missing, &missing, insufficient, &fresh, || false),
        Err(LocalFileError::Pixels(WarpError::Budget))
    ));
    assert_eq!(fresh.peak(), 0);
    let mut incompatible = policy;
    incompatible.registration.local.decode.max_frames += 1;
    assert!(matches!(
        compare_local_files_projective_complementary(&missing, &missing, incompatible, &fresh, || false),
        Err(LocalFileError::InvalidPolicy)
    ));
    assert_eq!(fresh.peak(), 0);
    let mut overflow = policy;
    overflow.registration.local.matching.max_comparisons = u64::MAX;
    assert!(matches!(
        compare_local_files_projective_complementary(&missing, &missing, overflow, &fresh, || false),
        Err(LocalFileError::Features(LocalError::Budget))
    ));
    assert_eq!(fresh.peak(), 0);
    let mut no_work = policy;
    no_work.registration.filter.filter.max_sample_pairs = 0;
    assert!(matches!(
        compare_local_files_projective_complementary(&a, &b, no_work, &budget, || false),
        Err(LocalFileError::Pixels(WarpError::Budget))
    ));
    assert_eq!(budget.used(), 0);
    let zero = MemoryBudget::new(0);
    assert!(compare_local_files_projective_complementary(&a, &b, policy, &zero, || false).is_err());
    assert_eq!(zero.used(), 0);
    for stop in [1, checkpoints / 2, checkpoints] {
        calls.set(0);
        assert!(
            matches!(
                compare_local_files_projective_complementary(&a, &b, policy, &budget, || {
                    calls.set(calls.get() + 1);
                    calls.get() >= stop
                }),
                Err(LocalFileError::Cancelled)
            ),
            "stop={stop}"
        );
        assert_eq!(budget.used(), 0);
    }
    let changed = Cell::new(false);
    let result = compare_local_files_projective_complementary(&a, &b, policy, &budget, || {
        if budget.used() > 0 && !changed.replace(true) {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&b_path)
                .unwrap()
                .write_all(&[0])
                .unwrap();
        }
        false
    });
    assert!(changed.get() && result.is_err(), "{result:?}");
    assert_eq!(budget.used(), 0);
    std::fs::copy(&a_path, &b_path).unwrap();
    assert!(
        compare_local_files_projective_complementary(&a, &b, policy, &budget, || false)
            .unwrap()
            .candidate
    );
    assert_eq!(budget.used(), 0);
}

#[test]
fn complementary_collection_keeps_separate_matching_and_nested_source_lifecycle() {
    use rrrah_dedup::{
        geometry::ProjectiveSamplingPolicy,
        local_collection::{
            ProjectiveComplementaryCollectionPolicy, scan_projective_local_collection_complementary,
            scan_projective_local_collection_roots_complementary,
            scan_projective_local_collection_roots_complementary_with_requests,
        },
        local_index::FileFeatureBudgets,
        local_scan::{
            LocalFileError, ProjectiveComplementaryFilePolicy, ProjectivePortfolioFilePolicy,
            ProjectivePyramidPhotometricFilePolicy, compare_local_files_projective_complementary,
        },
        scan::ScanError,
        warp::{
            ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitMode, PhotometricPolicy,
            ProjectiveRegistrationPolicy, ProjectiveRegistrationPortfolioPolicy,
            ProjectiveRegistrationTrustPolicy, WarpError,
        },
    };
    use std::{cell::Cell, collections::BTreeSet, io::Write};
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let dir = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for (id, name, relative) in [
        (1, "2414-base.png", "a/deep/a.png"),
        (2, "2414-base.png", "b/deep/b.png"),
        (3, "5495-base.png", "c/deep/c.png"),
    ] {
        let path = dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::copy(source.join(name), &path).unwrap();
        files.push((id, DecodeRequest::new(path)));
    }
    let mut local = policy();
    local.extract.max_features = 128;
    local.matching.max_comparisons = 128 * 128;
    local.matching.max_distance = 64;
    local.geometry.max_points = 128;
    local.geometry.max_hypotheses = 64;
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
        rounds: 1,
        max_sample_pairs: 64_000_000,
    };
    let sampling = ProjectiveSamplingPolicy {
        trials: 64,
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
    let search = ProjectiveComplementaryFilePolicy {
        registration: r,
        pyramid: p,
        max_total_comparisons: 128 * 128 + 384 * 384,
        max_total_hypotheses: 128,
        max_total_sample_pairs: 144_000_000,
    };
    let config = ProjectiveComplementaryCollectionPolicy {
        search,
        budgets: FileFeatureBudgets {
            max_files: 3,
            max_features: 1536,
            max_hits: 1536 * 1536,
            max_pair_counts: 3,
            max_pairs: 3,
        },
    };
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut direct = BTreeSet::new();
    for i in 0..3 {
        for j in i + 1..3 {
            if compare_local_files_projective_complementary(&files[i].1, &files[j].1, search, &budget, || {
                false
            })
            .unwrap()
            .candidate
            {
                direct.insert((files[i].0, files[j].0));
            }
            assert_eq!(budget.used(), 0);
        }
    }
    assert_eq!(direct, BTreeSet::from([(1, 2)]));
    let calls = Cell::new(0);
    let report = scan_projective_local_collection_complementary(files.clone(), config, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let checkpoints = calls.get();
    let accepted = |r: &rrrah_dedup::local_collection::LocalCollectionReport<
        rrrah_dedup::local_scan::ProjectiveComplementaryFileEvidence,
    >| {
        r.local
            .pairs
            .iter()
            .filter(|p| p.evidence.candidate)
            .map(|p| (p.left, p.right))
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(accepted(&report), direct);
    assert_eq!(report.analysed, vec![1, 2, 3]);
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty()
    );
    assert_eq!(budget.used(), 0);
    assert!(report.indexed_features > 0);
    assert!(
        report
            .local
            .pairs
            .iter()
            .filter(|p| p.evidence.candidate)
            .all(|p| p.evidence.accepted_searches == [true, true])
    );
    for stop in [1, checkpoints / 2, checkpoints] {
        calls.set(0);
        assert!(
            matches!(
                scan_projective_local_collection_complementary(files.clone(), config, &budget, || {
                    calls.set(calls.get() + 1);
                    calls.get() >= stop
                }),
                Err(ScanError::Cancelled)
            ),
            "stop={stop}"
        );
        assert_eq!(budget.used(), 0);
    }
    let limited = ProjectiveComplementaryCollectionPolicy {
        budgets: FileFeatureBudgets {
            max_pairs: 0,
            ..config.budgets
        },
        ..config
    };
    assert!(matches!(
        scan_projective_local_collection_complementary(files.clone(), limited, &budget, || false),
        Err(ScanError::Budget)
    ));
    assert_eq!(budget.used(), 0);
    let mut no_work = config;
    no_work.search.registration.filter.filter.max_sample_pairs = 0;
    let refused =
        scan_projective_local_collection_complementary(files.clone(), no_work, &budget, || false).unwrap();
    assert!(accepted(&refused).is_empty());
    assert!(
        refused
            .local
            .issues
            .iter()
            .any(|(a, b, e)| (*a, *b) == (1, 2) && matches!(e, LocalFileError::Pixels(WarpError::Budget)))
    );
    assert_eq!(budget.used(), 0);
    let changed = Cell::new(false);
    let changed_path = files[2].1.path.clone();
    calls.set(0);
    let changed_report =
        scan_projective_local_collection_complementary(files.clone(), config, &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == checkpoints - 2 {
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
    assert_eq!(changed_report.analysed, vec![1, 2]);
    assert!(changed_report.local.source_issues.iter().any(|(id, _)| *id == 3));
    assert_eq!(accepted(&changed_report), direct);
    assert_eq!(budget.used(), 0);
    std::fs::copy(source.join("5495-base.png"), &changed_path).unwrap();
    files.reverse();
    assert_eq!(
        accepted(&scan_projective_local_collection_complementary(files, config, &budget, || false).unwrap()),
        direct
    );
    assert_eq!(budget.used(), 0);
    let roots = vec![dir.path().to_path_buf(), dir.path().join("a")];
    let traversal = rrrah_dedup::exact::Options::default();
    let recursive =
        scan_projective_local_collection_roots_complementary(&roots, &traversal, config, &budget, || false)
            .unwrap();
    assert_eq!(recursive.files.len(), 3);
    assert_eq!(
        recursive
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
        scan_projective_local_collection_roots_complementary_with_requests(
            &roots,
            &traversal,
            config,
            &budget,
            |_| DecodeRequest::new("substitute.png"),
            || false
        ),
        Err(ScanError::InvalidPolicy)
    ));
    assert_eq!(budget.used(), 0);
}
