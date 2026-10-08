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
fn native_geometry_drives_guarded_affine_grid_without_manual_model() {
    use rrrah_dedup::{
        affine_color::AffineColorPolicy,
        affine_region::AffineRegionPolicy,
        affine_region_file::{search_affine_color_regions, verify_affine_grid_files},
        affine_region_grid::AffineGridPolicy,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let a = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let b = rrrah_decode::DecodeRequest::new(root.join("scale-117-angle-0.png"));
    let budget = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
    let grid = AffineGridPolicy {
        region: AffineRegionPolicy {
            radius: 1,
            maximum_sites: 200000,
            maximum_pixel_reads: 32000000,
            color: AffineColorPolicy {
                minimum_samples: 16,
                maximum_samples: 200000,
                minimum_variance: 1e-5,
                minimum_relative_pivot: 1e-6,
                maximum_coefficient: 5.,
                maximum_offset: 0.1,
            },
            tolerance: 0.03,
        },
        grid: (2, 2),
        maximum_regions: 4,
        maximum_source_radius: 16,
        maximum_total_pixel_reads: 256000000,
    };
    let result = search_affine_color_regions(&a, &b, config(), &factors(), grid, &budget, || false).unwrap();
    let model = result.search.geometry.as_ref().unwrap().transform;
    let regions = result.color_regions.as_ref().unwrap();
    assert_eq!(regions.len(), 4);
    let direct = verify_affine_grid_files(
        &a,
        &b,
        model,
        config().search.gradient.search.local.decode,
        grid,
        &budget,
        || false,
    )
    .unwrap();
    assert_eq!(format!("{regions:?}"), format!("{direct:?}"));
    drop(direct);
    drop(result);
    assert_eq!(budget.used(), 0);
}
