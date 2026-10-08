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
fn reflected_geometry_recovers_original_coordinate_mirror_and_refuses_before_io() {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let left=rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let right=rrrah_decode::DecodeRequest::new(root.join("mirrored.png"));
    let c=config();
    let limit=256*c.search.gradient.search.max_total_features as u64;
    let budget=rrrah_core::MemoryBudget::new(128*1024*1024);
    let e=rrrah_dedup::local_scan::find_projective_candidate_union_reflected_geometry_files_managed(
        &left,&right,c,&factors(),[2,2],limit,&budget,||false).unwrap();
    let geometry=e.geometry.as_ref().expect("mirrored geometry missing");
    assert!(e.correspondences.len()>=10);
    let H=geometry.transform.matrix;
    let determinant=H[0][0]*(H[1][1]*H[2][2]-H[1][2]*H[2][1])
        -H[0][1]*(H[1][0]*H[2][2]-H[1][2]*H[2][0])
        +H[0][2]*(H[1][0]*H[2][1]-H[1][1]*H[2][0]);
    assert!(determinant<0.,"reflection expected: {H:?}");
    for p in e.correspondences.iter() {
        assert!((0. ..160.).contains(&p.source[0]) && (0. ..160.).contains(&p.target[0]));
    }
    drop(e);
    assert_eq!(budget.used(),0);
    let missing=rrrah_decode::DecodeRequest::new(root.join("does-not-exist.png"));
    for cancelled in [false,true] {
        let fresh=rrrah_core::MemoryBudget::new(128*1024*1024);
        let result=rrrah_dedup::local_scan::find_projective_candidate_union_reflected_geometry_files_managed(
            &missing,&missing,c,&factors(),[2,2],limit-1,&fresh,||cancelled);
        if cancelled {assert!(matches!(result,Err(rrrah_dedup::local_scan::LocalFileError::Cancelled)));}
        else {assert!(matches!(result,Err(rrrah_dedup::local_scan::LocalFileError::Features(rrrah_dedup::local::LocalError::Budget))),"{result:?}");}
        assert_eq!(fresh.peak(),0);
    }
}
