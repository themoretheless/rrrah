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
fn geometry_only_anchor_pipeline_equals_legacy_anchor_confirmation() {
    use rrrah_dedup::{
        anchor_rank::AnchorRankPolicy,
        anchor_rank_file::{search_anchor_rank_files, search_anchor_rank_geometry_files},
        local_rank::LocalRankPolicy,
        rank_region::RankRegionPolicy,
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

    for target in [
        a.clone(),
        rrrah_decode::DecodeRequest::new(root.join("scale-117-angle-0.png")),
    ] {
        let legacy =
            search_anchor_rank_files(&a, &target, config(), &factors(), p, &budget, || false).unwrap();
        let current =
            search_anchor_rank_geometry_files(&a, &target, config(), &factors(), p, &budget, || false)
                .unwrap();
        assert_eq!(legacy.anchors, current.anchors);
        assert!(current.anchors.unwrap().supported);
        assert_eq!(
            legacy
                .search
                .correspondences
                .iter()
                .map(|p| (p.source, p.target))
                .collect::<Vec<_>>(),
            current
                .search
                .correspondences
                .iter()
                .map(|p| (p.source, p.target))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            legacy.search.geometry.as_ref().unwrap().transform.matrix,
            current.search.geometry.as_ref().unwrap().transform.matrix
        );
        drop(legacy);
        drop(current);
        assert_eq!(budget.used(), 0);
    }
    assert!(search_anchor_rank_geometry_files(&a, &a, config(), &factors(), p, &budget, || true).is_err());
    assert_eq!(budget.used(), 0);

    use rrrah_dedup::anchor_rank_file::AnchorRankFileError;
    use std::{
        cell::Cell,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };
    let calls = Cell::new(0);
    let measured = search_anchor_rank_geometry_files(&a, &a, config(), &factors(), p, &budget, || {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    drop(measured);
    assert_eq!(budget.used(), 0);
    for stop in [1, calls.get() / 2, calls.get()] {
        let count = Cell::new(0);
        let result = search_anchor_rank_geometry_files(&a, &a, config(), &factors(), p, &budget, || {
            count.set(count.get() + 1);
            count.get() == stop
        });
        assert!(
            matches!(result, Err(AnchorRankFileError::Cancelled)),
            "stop={stop}: {result:?}"
        );
        assert_eq!(budget.used(), 0);
        for owner in 0..2 {
            let generation = Arc::new(AtomicU64::new(1));
            let mut left = a.clone();
            let mut right = a.clone();
            let token = rrrah_decode::GenerationToken::new(generation.clone(), 1);
            if owner == 0 {
                left.cancellation = Some(token);
            } else {
                right.cancellation = Some(token);
            }
            let count = Cell::new(0);
            let result =
                search_anchor_rank_geometry_files(&left, &right, config(), &factors(), p, &budget, || {
                    count.set(count.get() + 1);
                    if count.get() == stop {
                        generation.store(2, Ordering::Release);
                    }
                    false
                });
            assert!(
                matches!(result, Err(AnchorRankFileError::Cancelled)),
                "owner={owner}, stop={stop}: {result:?}"
            );
            assert_eq!(budget.used(), 0);
        }
    }

    // Mutation after pixel work must invalidate evidence already computed.
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mutable.png");
    let original = std::fs::read(&a.path).unwrap();
    std::fs::write(&path, &original).unwrap();
    let mutable = rrrah_decode::DecodeRequest::new(&path);
    let count = Cell::new(0);
    let measured = search_anchor_rank_geometry_files(&mutable, &a, config(), &factors(), p, &budget, || {
        count.set(count.get() + 1);
        false
    })
    .unwrap();
    drop(measured);
    let total = count.get();
    for stop in [total / 2, total - 10] {
        std::fs::write(&path, &original).unwrap();
        let count = Cell::new(0);
        let changed = Cell::new(false);
        let result =
            search_anchor_rank_geometry_files(&mutable, &a, config(), &factors(), p, &budget, || {
                count.set(count.get() + 1);
                if count.get() == stop {
                    let mut bytes = original.clone();
                    bytes.push(0);
                    std::fs::write(&path, bytes).unwrap();
                    changed.set(true);
                }
                false
            });
        assert!(changed.get());
        assert!(
            matches!(
                result,
                Err(AnchorRankFileError::Source(_)) | Err(AnchorRankFileError::Search(_))
            ),
            "stop={stop}: {result:?}"
        );
        assert_eq!(budget.used(), 0);
    }
    std::fs::write(&path, original).unwrap();
    let restored =
        search_anchor_rank_geometry_files(&mutable, &a, config(), &factors(), p, &budget, || false).unwrap();
    assert!(restored.anchors.unwrap().supported);
    drop(restored);
    assert_eq!(budget.used(), 0);
}
