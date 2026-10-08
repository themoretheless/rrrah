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
fn supplied_model_regions_require_pixels_and_cancel_atomically() {
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

    use rrrah_dedup::{
        decode::decode_selected_frame_bounded,
        local_scan::{
            ProjectivePyramidRegionsFilePolicy, compare_local_files_projective_pyramid_region_grid,
        },
        region_transform::verify_projective_region_grid_views,
    };
    let q = ProjectivePyramidRegionsFilePolicy {
        search: p,
        max_regions: 4,
        max_total_sample_pairs: 60_000_000,
    };
    let direct =
        compare_local_files_projective_pyramid_region_grid(&a, &a, q, (2, 2), &budget, || false).unwrap();
    let model = direct.whole.geometry.as_ref().unwrap().transform;
    let image = decode_selected_frame_bounded(&a, p.local.decode, &budget, || false).unwrap();
    let view = image.view(|| false).unwrap();
    let baseline = budget.used();
    let calls = Cell::new(0);
    let regions = verify_projective_region_grid_views(&view, &view, model, q, (2, 2), &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    assert_eq!(format!("{:?}", regions), format!("{:?}", direct.regions));
    assert!(regions.iter().all(|r| r.accepted_region));
    drop(regions);
    assert_eq!(budget.used(), baseline);
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(matches!(
            verify_projective_region_grid_views(&view, &view, model, q, (2, 2), &budget, || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(LocalFileError::Cancelled)
        ));
        assert_eq!(budget.used(), baseline);
    }
    let mut short = q;
    short.max_total_sample_pairs -= 1;
    assert!(matches!(
        verify_projective_region_grid_views(&view, &view, model, short, (2, 2), &budget, || false),
        Err(LocalFileError::Pixels(rrrah_dedup::warp::WarpError::Budget))
    ));
    assert_eq!(budget.used(), baseline);
    use rrrah_dedup::region_transform::verify_projective_bidirectional_region_grid_views;
    let mut both = q;
    both.max_regions = 8;
    both.max_total_sample_pairs = 108_000_000;
    calls.set(0);
    let symmetric =
        verify_projective_bidirectional_region_grid_views(&view, &view, model, both, (2, 2), &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let both_calls = calls.get();
    assert_eq!(symmetric.len(), 8);
    assert_eq!(
        format!("{:?}", &symmetric[..4]),
        format!("{:?}", &direct.regions[..])
    );
    assert!(symmetric.iter().all(|r| r.accepted_region));
    drop(symmetric);
    assert_eq!(budget.used(), baseline);
    for stop in [1, both_calls / 2, both_calls] {
        calls.set(0);
        assert!(matches!(
            verify_projective_bidirectional_region_grid_views(
                &view,
                &view,
                model,
                both,
                (2, 2),
                &budget,
                || {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }
            ),
            Err(LocalFileError::Cancelled)
        ));
        assert_eq!(budget.used(), baseline);
    }
    for insufficient in [
        ProjectivePyramidRegionsFilePolicy {
            max_regions: 7,
            ..both
        },
        ProjectivePyramidRegionsFilePolicy {
            max_total_sample_pairs: 107_999_999,
            ..both
        },
    ] {
        let fresh = MemoryBudget::new(64 * 1024 * 1024);
        assert!(matches!(
            verify_projective_bidirectional_region_grid_views(
                &view,
                &view,
                model,
                insufficient,
                (2, 2),
                &fresh,
                || false
            ),
            Err(LocalFileError::Pixels(rrrah_dedup::warp::WarpError::Budget))
        ));
        assert_eq!(fresh.peak(), 0);
    }
    let unrelated = decode_selected_frame_bounded(&b, p.local.decode, &budget, || false).unwrap();
    let unrelated_view = unrelated.view(|| false).unwrap();
    let both_negative = verify_projective_bidirectional_region_grid_views(
        &view,
        &unrelated_view,
        model,
        both,
        (2, 2),
        &budget,
        || false,
    )
    .unwrap();
    assert!(!both_negative.iter().any(|r| r.accepted_region));
    drop(both_negative);
    let negative =
        verify_projective_region_grid_views(&view, &unrelated_view, model, q, (2, 2), &budget, || false)
            .unwrap();
    assert!(!negative.iter().any(|r| r.accepted_region));
    drop(negative);
    drop(unrelated_view);
    drop(unrelated);
    drop(view);
    drop(image);
    drop(direct);
    assert_eq!(budget.used(), 0);
    use rrrah_dedup::local_scan::compare_local_files_projective_region_grid_transform;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("copy.png");
    std::fs::copy(root.join("2414-base.png"), &path).unwrap();
    let request = DecodeRequest::new(&path);
    calls.set(0);
    let file =
        compare_local_files_projective_region_grid_transform(&a, &request, model, q, (2, 2), &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let total = calls.get();
    assert_eq!(file.region_support_count(), 4);
    assert_eq!(file.transform, model);
    drop(file);
    assert_eq!(budget.used(), 0);
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(matches!(
            compare_local_files_projective_region_grid_transform(
                &a,
                &request,
                model,
                q,
                (2, 2),
                &budget,
                || {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }
            ),
            Err(LocalFileError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let changed = Cell::new(false);
    calls.set(0);
    let refused =
        compare_local_files_projective_region_grid_transform(&a, &request, model, q, (2, 2), &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == total / 2 {
                use std::io::Write;
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                changed.set(true);
            }
            false
        });
    assert!(changed.get());
    assert!(matches!(
        refused,
        Err(LocalFileError::Source(rrrah_dedup::exact::SnapshotError::Changed))
    ));
    assert_eq!(budget.used(), 0);
    std::fs::copy(root.join("2414-base.png"), &path).unwrap();
    let retry =
        compare_local_files_projective_region_grid_transform(&a, &request, model, q, (2, 2), &budget, || {
            false
        })
        .unwrap();
    assert_eq!(retry.region_support_count(), 4);
    drop(retry);
    assert_eq!(budget.used(), 0);
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    let missing = DecodeRequest::new("missing-supplied-model.png");
    assert!(matches!(
        compare_local_files_projective_region_grid_transform(
            &missing,
            &missing,
            model,
            short,
            (2, 2),
            &fresh,
            || false
        ),
        Err(LocalFileError::Pixels(rrrah_dedup::warp::WarpError::Budget))
    ));
    assert_eq!(fresh.peak(), 0);

    use rrrah_dedup::local_scan::compare_local_files_projective_bidirectional_region_grid_transform;
    calls.set(0);
    let symmetric_file = compare_local_files_projective_bidirectional_region_grid_transform(
        &a,
        &request,
        model,
        both,
        (2, 2),
        &budget,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let file_calls = calls.get();
    assert_eq!(symmetric_file.region_support_count(), 8);
    drop(symmetric_file);
    assert_eq!(budget.used(), 0);
    for stop in [1, file_calls / 2, file_calls] {
        calls.set(0);
        assert!(matches!(
            compare_local_files_projective_bidirectional_region_grid_transform(
                &a,
                &request,
                model,
                both,
                (2, 2),
                &budget,
                || {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }
            ),
            Err(LocalFileError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
    }
    let mut transfer_short = both;
    transfer_short.max_total_sample_pairs -= 1;
    let fresh = MemoryBudget::new(64 * 1024 * 1024);
    assert!(matches!(
        compare_local_files_projective_bidirectional_region_grid_transform(
            &missing,
            &missing,
            model,
            transfer_short,
            (2, 2),
            &fresh,
            || false
        ),
        Err(LocalFileError::Pixels(rrrah_dedup::warp::WarpError::Budget))
    ));
    assert_eq!(fresh.peak(), 0);
    calls.set(0);
    changed.set(false);
    let refused = compare_local_files_projective_bidirectional_region_grid_transform(
        &a,
        &request,
        model,
        both,
        (2, 2),
        &budget,
        || {
            calls.set(calls.get() + 1);
            if calls.get() == file_calls / 2 {
                use std::io::Write;
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                changed.set(true);
            }
            false
        },
    );
    assert!(changed.get());
    assert!(matches!(
        refused,
        Err(LocalFileError::Source(rrrah_dedup::exact::SnapshotError::Changed))
    ));
    assert_eq!(budget.used(), 0);
    std::fs::copy(root.join("2414-base.png"), &path).unwrap();
    let retry = compare_local_files_projective_bidirectional_region_grid_transform(
        &a,
        &request,
        model,
        both,
        (2, 2),
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(retry.region_support_count(), 8);
    drop(retry);
    assert_eq!(budget.used(), 0);
}
