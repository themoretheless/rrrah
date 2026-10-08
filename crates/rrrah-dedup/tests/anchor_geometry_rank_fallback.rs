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
fn rank_driven_fallback_admission_selection_cancel_and_release() {
    use rrrah_dedup::{
        anchor_rank::AnchorRankPolicy,
        anchor_rank_file::{
            AnchorGeometryFallbackPolicy, AnchorRankFileError, search_anchor_rank_geometry_fallback_files,
            search_anchor_rank_geometry_files,
        },
        local_rank::{LocalRankError, LocalRankPolicy},
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
    let c = config();
    let policy = AnchorGeometryFallbackPolicy {
        search: rrrah_dedup::local_scan::ProjectiveCandidateUnionFallbackFilePolicy {
            search: c,
            asymmetric_radius: 4,
            max_total_gradient_samples: 3 * c.max_total_gradient_samples,
            max_total_match_comparisons: 3 * c.max_total_match_comparisons,
            max_total_union_comparisons: 3 * c.max_union_comparisons,
        },
        anchors: p,
        maximum_total_point_checks: 3 * p.local.maximum_point_checks,
        maximum_total_selection_checks: 3 * p.maximum_selection_checks,
        maximum_total_rank_sites: 3 * p.local.rank.maximum_sites,
        maximum_total_pixel_reads: 3 * p.local.rank.maximum_pixel_reads,
    };
    let direct = search_anchor_rank_geometry_files(&a, &a, c, &factors(), p, &budget, || false).unwrap();
    let result =
        search_anchor_rank_geometry_fallback_files(&a, &a, policy, &factors(), &budget, || false).unwrap();
    assert_eq!(result.attempted_recipes, 1);
    assert_eq!(result.smoothing_radii, [2, 2]);
    assert_eq!(result.evidence.anchors, direct.anchors);
    assert!(result.evidence.anchors.unwrap().supported);
    assert_eq!(
        result
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
    drop(result);
    drop(direct);
    assert_eq!(budget.used(), 0);
    let missing = rrrah_decode::DecodeRequest::new("/nonexistent/rrrah-rank-fallback.bmp");
    for case in 0..4 {
        let mut bad = policy;
        match case {
            0 => bad.maximum_total_point_checks -= 1,
            1 => bad.maximum_total_selection_checks -= 1,
            2 => bad.maximum_total_rank_sites -= 1,
            _ => bad.maximum_total_pixel_reads -= 1,
        }
        let fresh = rrrah_core::MemoryBudget::new(64 * 1024 * 1024);
        assert!(matches!(
            search_anchor_rank_geometry_fallback_files(&missing, &missing, bad, &factors(), &fresh, || false),
            Err(AnchorRankFileError::Rank(LocalRankError::Budget))
        ));
        assert!(matches!(
            search_anchor_rank_geometry_fallback_files(&missing, &missing, bad, &factors(), &fresh, || true),
            Err(AnchorRankFileError::Cancelled)
        ));
        assert_eq!(fresh.peak(), 0);
    }
    // A valid flat image has no geometric proposal: all three attempts are misses.
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("flat.bmp");
    let mut bytes = Vec::new();
    let pixels = 64 * 64 * 3u32;
    bytes.extend(b"BM");
    bytes.extend((54 + pixels).to_le_bytes());
    bytes.extend([0; 4]);
    bytes.extend(54u32.to_le_bytes());
    bytes.extend(40u32.to_le_bytes());
    bytes.extend(64i32.to_le_bytes());
    bytes.extend(64i32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(24u16.to_le_bytes());
    bytes.extend([0; 24]);
    bytes.resize((54 + pixels) as usize, 0);
    std::fs::write(&path, &bytes).unwrap();
    let flat = rrrah_decode::DecodeRequest::new(&path);
    let calls = std::cell::Cell::new(0);
    let result =
        search_anchor_rank_geometry_fallback_files(&flat, &flat, policy, &factors(), &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(result.attempted_recipes, 3);
    assert_eq!(result.smoothing_radii, [0, 4]);
    assert!(result.evidence.anchors.is_none());
    assert!(result.evidence.search.geometry.is_none());
    drop(result);
    assert_eq!(budget.used(), 0);
    for stop in [1, calls.get() / 2, calls.get()] {
        let count = std::cell::Cell::new(0);
        let result =
            search_anchor_rank_geometry_fallback_files(&flat, &flat, policy, &factors(), &budget, || {
                count.set(count.get() + 1);
                count.get() == stop
            });
        assert!(
            matches!(result, Err(AnchorRankFileError::Cancelled)),
            "stop={stop}: {result:?}"
        );
        assert_eq!(budget.used(), 0);
    }
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    for owner in 0..2 {
        for stop in [1, calls.get() / 2, calls.get()] {
            let generation = Arc::new(AtomicU64::new(1));
            let token = rrrah_decode::GenerationToken::new(generation.clone(), 1);
            let mut left = flat.clone();
            let mut right = flat.clone();
            if owner == 0 {
                left.cancellation = Some(token);
            } else {
                right.cancellation = Some(token);
            }
            let count = std::cell::Cell::new(0);
            let result = search_anchor_rank_geometry_fallback_files(
                &left,
                &right,
                policy,
                &factors(),
                &budget,
                || {
                    count.set(count.get() + 1);
                    if count.get() == stop {
                        generation.store(2, Ordering::Release);
                    }
                    false
                },
            );
            assert!(
                matches!(result, Err(AnchorRankFileError::Cancelled)),
                "owner={owner}, stop={stop}: {result:?}"
            );
            assert_eq!(budget.used(), 0);
        }
    }
    for stop in [calls.get() / 2, calls.get() - 10] {
        std::fs::write(&path, &bytes).unwrap();
        let count = std::cell::Cell::new(0);
        let changed = std::cell::Cell::new(false);
        let result =
            search_anchor_rank_geometry_fallback_files(&flat, &flat, policy, &factors(), &budget, || {
                count.set(count.get() + 1);
                if count.get() == stop {
                    let mut changed_bytes = bytes.clone();
                    changed_bytes.push(0);
                    std::fs::write(&path, changed_bytes).unwrap();
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
    std::fs::write(&path, &bytes).unwrap();
    let restored =
        search_anchor_rank_geometry_fallback_files(&flat, &flat, policy, &factors(), &budget, || false)
            .unwrap();
    assert_eq!(restored.attempted_recipes, 3);
    assert!(restored.evidence.anchors.is_none());
    drop(restored);
    assert_eq!(budget.used(), 0);
}
