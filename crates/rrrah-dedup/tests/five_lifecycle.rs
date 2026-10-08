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
fn five_search_source_replacement_discards_prior_positives() {
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
    use std::{cell::Cell, io::Write};
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let dir = tempfile::tempdir().unwrap();
    let a_path = dir.path().join("a.png");
    let b_path = dir.path().join("b.png");
    std::fs::copy(source.join("2414-base.png"), &a_path).unwrap();
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

    use rrrah_dedup::{gradient::GradientMatchPolicy, local_scan::*};
    let q = ProjectiveComplementaryFilterPortfolioPolicy {
        searches: policy,
        secondary: ColorFilterPolicy {
            filter: FilterPolicy {
                radius: 3,
                max_sample_pairs: 32_000_000,
            },
            color_space: FilterColorSpace::LinearSrgb,
        },
        max_total_sample_pairs: 176_000_000,
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
    let combined = ProjectiveComplementaryGradientPolicy {
        base: q,
        gradient,
        max_total_comparisons: policy.max_total_comparisons + 384 * 384,
        max_total_hypotheses: policy.max_total_hypotheses + p.sampling.trials,
        max_total_sample_pairs: q.max_total_sample_pairs + p.filter.filter.max_sample_pairs,
        max_total_gradient_samples: 2 * gradient.max_total_gradient_samples,
    };

    let five = ProjectiveComplementaryGradientPortfolioPolicy {
        primary: combined,
        interpolated: gradient,
        max_total_comparisons: combined.max_total_comparisons + gradient.matching.max_comparisons,
        max_total_hypotheses: combined.max_total_hypotheses + gradient.search.sampling.trials,
        max_total_sample_pairs: combined.max_total_sample_pairs
            + gradient.search.filter.filter.max_sample_pairs,
        max_total_gradient_samples: combined.max_total_gradient_samples
            + 2 * gradient.max_total_gradient_samples,
    };
    let calls = Cell::new(0);
    let four = compare_local_files_projective_complementary_gradient(&a, &b, combined, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(four.accepted_searches, [true; 4]);
    let four_calls = calls.get();
    calls.set(0);
    let baseline =
        compare_local_files_projective_complementary_gradient_portfolio(&a, &b, five, &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(baseline.accepted_searches, [true; 5]);
    let five_calls = calls.get();
    assert!(five_calls > four_calls + 100);
    let stop = four_calls + (five_calls - four_calls) / 2;
    let changed = Cell::new(false);
    calls.set(0);
    let result =
        compare_local_files_projective_complementary_gradient_portfolio(&a, &b, five, &budget, || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                assert!(budget.used() > 0);
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&b_path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap();
                changed.set(true);
            }
            false
        });
    assert!(
        changed.get()
            && matches!(
                result,
                Err(LocalFileError::Source(rrrah_dedup::exact::SnapshotError::Changed))
            ),
        "{result:?}"
    );
    assert_eq!(budget.used(), 0);
    std::fs::copy(&a_path, &b_path).unwrap();
    let retry =
        compare_local_files_projective_complementary_gradient_portfolio(&a, &b, five, &budget, || false)
            .unwrap();
    assert_eq!(retry.accepted_searches, [true; 5]);
    assert_eq!(budget.used(), 0);
}
