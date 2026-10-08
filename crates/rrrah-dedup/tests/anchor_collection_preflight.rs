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
    ProjectiveCandidateUnionRegionsFilePolicy, ProjectiveDistinctScaleRegionsFilePolicy, SpatialFeaturePolicy,
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
fn malformed_anchor_policies_and_work_refuse_before_file_iteration() {
    use rrrah_dedup::{
        anchor_rank::AnchorRankPolicy, local_rank::LocalRankPolicy, rank_region::RankRegionPolicy,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let a = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let p = AnchorRankPolicy {
        local: LocalRankPolicy {
            rank: RankRegionPolicy {
                radius: 8,
                minimum_contrast: 0.005,
                minimum_pairs: 1000,
                maximum_sites: 12800000,
                maximum_pixel_reads: 64000000,
            },
            filter_radius: 8,
            minimum_witnesses: 10,
            maximum_points: 4000,
            maximum_point_checks: 8002000,
            minimum_point_separation: 2.,
            target_tolerance: 2.,
            source_tolerance: 2.,
            minimum_coverage: 0.3,
            minimum_agreement: 0.9,
        },
        window_radius: 2,
        maximum_selection_checks: 24002000,
    };

    let config = rrrah_dedup::local_collection::ProjectiveCandidateUnionRegionsCollectionPolicy {
        search: config(),
        budgets: rrrah_dedup::local_index::FileFeatureBudgets {
            max_files: 3,
            max_features: 12000,
            max_hits: 32000000,
            max_pair_counts: 3,
            max_pairs: 3,
        },
        max_retrieval_comparisons: 144000000,
    };

    for kind in 0..11 {
        let mut malformed = p;
        match kind {
            0 => malformed.local.minimum_coverage = f64::NAN,
            1 => malformed.local.minimum_agreement = 1.01,
            2 => malformed.local.rank.minimum_contrast = 0.,
            3 => malformed.local.minimum_point_separation = f64::INFINITY,
            4 => malformed.local.source_tolerance = -1.,
            5 => malformed.local.target_tolerance = 0.,
            6 => malformed.local.rank.radius = 0,
            7 => malformed.local.rank.minimum_pairs = 0,
            8 => malformed.local.minimum_witnesses = 3,
            9 => malformed.local.maximum_points = 9,
            _ => malformed.maximum_selection_checks = 0,
        }
        let unread = std::iter::from_fn(|| -> Option<(u64, rrrah_decode::DecodeRequest)> {
            panic!("input must not be consumed");
        });
        let result = rrrah_dedup::local_collection::scan_anchor_rank_collection(
            unread,
            config,
            &factors(),
            malformed,
            &budget,
            || false,
        );
        if kind == 10 {
            assert!(matches!(result, Err(rrrah_dedup::scan::ScanError::Budget)));
        } else {
            assert!(
                matches!(result, Err(rrrah_dedup::scan::ScanError::InvalidPolicy)),
                "kind={kind}: {result:?}"
            );
        }
        assert_eq!(budget.peak(), 0);
    }
    let mut malformed = p;
    malformed.local.minimum_coverage = f64::NAN;
    let unread = std::iter::from_fn(|| -> Option<(u64, rrrah_decode::DecodeRequest)> {
        panic!("cancelled input must not be consumed");
    });
    assert!(matches!(
        rrrah_dedup::local_collection::scan_anchor_rank_collection(
            unread,
            config,
            &factors(),
            malformed,
            &budget,
            || true
        ),
        Err(rrrah_dedup::scan::ScanError::Cancelled)
    ));
    assert_eq!(budget.peak(), 0);
}
