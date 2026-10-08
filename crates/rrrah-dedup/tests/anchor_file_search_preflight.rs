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
fn all_anchor_file_searches_reject_invalid_policy_before_io() {
    use rrrah_dedup::{
        anchor_rank::AnchorRankPolicy,
        anchor_rank_file::{
            AnchorRankFileError, search_anchor_rank_fallback_files, search_anchor_rank_files,
            search_anchor_rank_geometry_files,
        },
        local_rank::{LocalRankError, LocalRankPolicy},
        rank_region::RankRegionPolicy,
    };
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
    let missing = rrrah_decode::DecodeRequest::new("/nonexistent/rrrah-anchor-search-preflight.png");
    let c = config();
    let fallback = rrrah_dedup::local_scan::ProjectiveCandidateUnionFallbackFilePolicy {
        search: c,
        asymmetric_radius: 4,
        max_total_gradient_samples: 3 * c.max_total_gradient_samples,
        max_total_match_comparisons: 3 * c.max_total_match_comparisons,
        max_total_union_comparisons: 3 * c.max_union_comparisons,
    };
    for case in 0..10 {
        let mut bad = p;
        match case {
            0 => bad.local.minimum_coverage = f64::NAN,
            1 => bad.local.minimum_agreement = 1.01,
            2 => bad.local.rank.minimum_contrast = 0.,
            3 => bad.local.minimum_point_separation = f64::INFINITY,
            4 => bad.local.source_tolerance = -1.,
            5 => bad.local.target_tolerance = 0.,
            6 => bad.local.rank.radius = 0,
            7 => bad.local.rank.minimum_pairs = 0,
            8 => bad.local.minimum_witnesses = 3,
            _ => bad.local.maximum_points = 9,
        }
        for cancelled in [false, true] {
            let results = [
                search_anchor_rank_files(&missing, &missing, c, &factors(), bad, &budget, || cancelled)
                    .map(|_| ()),
                search_anchor_rank_geometry_files(&missing, &missing, c, &factors(), bad, &budget, || {
                    cancelled
                })
                .map(|_| ()),
                search_anchor_rank_fallback_files(
                    &missing,
                    &missing,
                    fallback,
                    &factors(),
                    bad,
                    &budget,
                    || cancelled,
                )
                .map(|_| ()),
            ];
            for result in results {
                if cancelled {
                    assert!(matches!(result, Err(AnchorRankFileError::Cancelled)));
                } else {
                    assert!(
                        matches!(result, Err(AnchorRankFileError::Rank(LocalRankError::Invalid))),
                        "case={case}: {result:?}"
                    );
                }
            }
        }
    }
    assert_eq!(budget.peak(), 0);
}
