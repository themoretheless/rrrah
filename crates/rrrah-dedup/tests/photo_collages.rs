#![cfg(feature = "decode")]
use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::GeometryPolicy,
    local::{LocalPolicy, MatchPolicy},
    local_scan::{
        LocalFileError, LocalFilePolicy, compare_local_files_photometric,
        compare_local_files_with_color_filter,
    },
    warp::{
        ColorFilterPolicy, FilterColorSpace, FilterPolicy, PhotometricFitFailure, PhotometricPolicy,
        WarpError, WarpPolicy,
    },
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
fn dark_signal_can_pass_selected_encoded_metric_with_auxiliary_linear_failure_retained() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photo-collages");
    let a = DecodeRequest::new(root.join("shadow.png"));
    let budget = MemoryBudget::new(8 * 1024 * 1024);
    let mut p = policy();
    p.extract.minimum_corner_score = 0.;
    p.extract.max_features = 100;
    let raw = compare_local_files_photometric(&a, &a, p, photometric_policy(), &budget, || false);
    assert!(
        matches!(
            raw,
            Err(LocalFileError::Pixels(WarpError::Fit(
                PhotometricFitFailure::LowVariance { channel: 0 }
            )))
        ),
        "{raw:?}"
    );
    let e = compare_local_files_with_color_filter(
        &a,
        &a,
        p,
        photometric_policy(),
        ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 3,
                max_sample_pairs: 20_000_000,
            },
            color_space: FilterColorSpace::EncodedSrgb,
        },
        &budget,
        || false,
    )
    .unwrap();
    assert!(e.candidate);
    assert!(e.pixels.is_some());
    assert!(e.photometric.is_none());
    assert_eq!(
        e.photometric_failure,
        Some(PhotometricFitFailure::LowVariance { channel: 0 })
    );
    let f = e.filtered.unwrap();
    assert_eq!(
        f.evidence.forward.pixels.matched_pixels,
        f.evidence.forward.pixels.compared_pixels
    );
    assert_eq!(
        f.evidence.reverse.pixels.matched_pixels,
        f.evidence.reverse.pixels.compared_pixels
    );
    assert_eq!(budget.used(), 0);
}
#[test]
fn partial_copy_challenges_distinguish_residual_rejections_from_inconclusive_fits() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let filter = ColorFilterPolicy {
        filter: FilterPolicy {
            radius: 3,
            max_sample_pairs: 20_000_000,
        },
        color_space: FilterColorSpace::EncodedSrgb,
    };
    let mut photo = photometric_policy();
    photo.minimum_gain = 0.01;
    photo.maximum_gain = 100.;
    photo.maximum_offset = 1.;
    let mut residual_rejections = 0;
    let mut no_geometry = 0;
    let mut inconclusive = 0;
    for a in [830, 898, 1084, 1294] {
        for b in [830, 898, 1084, 1294] {
            if a == b {
                continue;
            }
            for percent in [40, 70] {
                let result = compare_local_files_with_color_filter(
                    &DecodeRequest::new(root.join(format!("photos/{a}-base.png"))),
                    &DecodeRequest::new(root.join(format!("photo-collages/{a}-{b}-{percent}.png"))),
                    policy(),
                    photo,
                    filter,
                    &budget,
                    || false,
                );
                match result {
                    Ok(e) => {
                        assert!(!e.candidate, "{a} {b} {percent}");
                        if let Some(g) = e.geometry {
                            assert!(g.inliers.len() >= 10);
                            assert!(e.filtered.is_some());
                            residual_rejections += 1;
                        } else {
                            no_geometry += 1;
                        }
                    }
                    Err(LocalFileError::Pixels(WarpError::Fit(_))) => inconclusive += 1,
                    Err(e) => panic!("unexpected source/resource failure: {e:?}"),
                }
                assert_eq!(budget.used(), 0);
            }
        }
    }
    assert!(residual_rejections >= 16);
    assert_eq!(residual_rejections + no_geometry + inconclusive, 24);
    // Inconclusive cases stay explicit; no test treats them as proven negatives.
}

#[test]
fn constrained_partial_copies_reject_residuals_without_relaxing_model_bounds() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let budget = MemoryBudget::new(64 * 1024 * 1024);
    let mut residuals = 0;
    let mut missing_geometry = 0;
    for a in [830, 898, 1084, 1294] {
        for b in [830, 898, 1084, 1294] {
            if a == b {
                continue;
            }
            for percent in [40, 70] {
                let e = rrrah_dedup::local_scan::compare_local_files_with_constrained_filter(
                    &DecodeRequest::new(root.join(format!("photos/{a}-base.png"))),
                    &DecodeRequest::new(root.join(format!("photo-collages/{a}-{b}-{percent}.png"))),
                    policy(),
                    photometric_policy(),
                    ColorFilterPolicy {
                        filter: FilterPolicy {
                            radius: 3,
                            max_sample_pairs: 20_000_000,
                        },
                        color_space: FilterColorSpace::EncodedSrgb,
                    },
                    &budget,
                    || false,
                )
                .unwrap();
                let indexed = indexed_partial_pair(&root, a, b, percent, &budget);
                assert!(!indexed.candidate, "indexed {a} {b} {percent}");
                assert_eq!(indexed.geometry.is_some(), e.geometry.is_some());
                assert_eq!(indexed.filtered.is_some(), e.filtered.is_some());
                assert!(!e.candidate, "{a} {b} {percent}");
                if let Some(f) = e.filtered {
                    assert_eq!(
                        f.fit_mode,
                        rrrah_dedup::warp::PhotometricFitMode::ConstrainedLeastSquares
                    );
                    for grid in [&f.evidence.forward, &f.evidence.reverse] {
                        assert!(grid.gain.iter().all(|v| (0.2..=5.0).contains(v)));
                        assert!(grid.offset.iter().all(|v| v.abs() <= 0.1));
                    }
                    residuals += 1;
                } else {
                    missing_geometry += 1;
                }
                assert_eq!(budget.used(), 0);
            }
        }
    }
    assert_eq!((residuals, missing_geometry), (20, 4));
}

fn indexed_partial_pair(
    root: &std::path::Path,
    a: u64,
    b: u64,
    percent: u64,
    budget: &MemoryBudget,
) -> rrrah_dedup::local_scan::LocalFileEvidence {
    use rrrah_dedup::{
        local_collection::{LocalCollectionPolicy, scan_local_collection},
        local_index::FileFeatureBudgets,
        local_scan::{LocalComparisonMode, LocalSearchPolicy},
        warp::PhotometricFitMode,
    };
    let report = scan_local_collection(
        [
            (0, DecodeRequest::new(root.join(format!("photos/{a}-base.png")))),
            (
                1,
                DecodeRequest::new(root.join(format!("photo-collages/{a}-{b}-{percent}.png"))),
            ),
        ],
        LocalCollectionPolicy {
            search: LocalSearchPolicy {
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
                    fit: PhotometricFitMode::ConstrainedLeastSquares,
                },
            },
            budgets: FileFeatureBudgets {
                max_files: 2,
                max_features: 1000,
                max_hits: 1_000_000,
                max_pair_counts: 1,
                max_pairs: 1,
            },
        },
        budget,
        || false,
    )
    .unwrap();
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty()
    );
    assert_eq!(report.analysed, [0, 1]);
    assert_eq!(report.local.pairs.len(), 1);
    report.local.pairs.into_iter().next().unwrap().evidence
}
