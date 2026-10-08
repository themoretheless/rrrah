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
fn geometry_only_indexed_collection_matches_all_direct_pairs() {
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

    let scaled = rrrah_decode::DecodeRequest::new(root.join("scale-117-angle-0.png"));
    let files = vec![(1, a.clone()), (2, scaled), (3, a)];
    let report = rrrah_dedup::local_collection::scan_anchor_rank_geometry_collection(
        files.clone(),
        config,
        &factors(),
        p,
        &budget,
        || false,
    )
    .unwrap();
    assert!(
        report.file_issues.is_empty()
            && report.local.issues.is_empty()
            && report.local.source_issues.is_empty(),
        "{report:?}"
    );
    assert_eq!(report.local.pairs.len(), 3);
    for i in 0..files.len() {
        for j in i + 1..files.len() {
            let direct = rrrah_dedup::anchor_rank_file::search_anchor_rank_geometry_files(
                &files[i].1,
                &files[j].1,
                config.search,
                &factors(),
                p,
                &budget,
                || false,
            )
            .unwrap();
            let indexed = report
                .local
                .pairs
                .iter()
                .find(|v| (v.left, v.right) == (files[i].0, files[j].0))
                .unwrap();
            assert_eq!(indexed.evidence.anchors, direct.anchors);
            assert_eq!(
                indexed
                    .evidence
                    .search
                    .correspondences
                    .iter()
                    .map(|p| (p.source, p.target))
                    .collect::<Vec<_>>(),
                direct
                    .search
                    .correspondences
                    .iter()
                    .map(|p| (p.source, p.target))
                    .collect::<Vec<_>>()
            );
            let indexed_geometry = indexed.evidence.search.geometry.as_ref().unwrap();
            let direct_geometry = direct.search.geometry.as_ref().unwrap();
            assert_eq!(
                indexed_geometry.transform.matrix,
                direct_geometry.transform.matrix
            );
            assert_eq!(&*indexed_geometry.inliers, &*direct_geometry.inliers);
            assert_eq!(indexed_geometry.squared_error, direct_geometry.squared_error);
            assert_eq!(indexed_geometry.hypotheses, direct_geometry.hypotheses);
            assert!(direct.anchors.unwrap().supported);
            drop(direct);
        }
    }
    drop(report);
    assert_eq!(budget.used(), 0);
}
