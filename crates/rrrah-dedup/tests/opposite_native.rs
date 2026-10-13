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

fn anchors() -> rrrah_dedup::anchor_rank::AnchorRankPolicy {
    rrrah_dedup::anchor_rank::AnchorRankPolicy {
        local: rrrah_dedup::local_rank::LocalRankPolicy {
            rank: rrrah_dedup::rank_region::RankRegionPolicy {
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
    }
}

fn portfolio() -> rrrah_dedup::anchor_rank_file::AnchorReflectionFallbackPolicy {
    let c = config();
    let p = anchors();
    rrrah_dedup::anchor_rank_file::AnchorReflectionFallbackPolicy {
        search: rrrah_dedup::anchor_rank_file::AnchorGeometryFallbackPolicy {
            search: rrrah_dedup::local_scan::ProjectiveCandidateUnionFallbackFilePolicy {
                search: c,
                asymmetric_radius: 4,
                max_total_gradient_samples: 6 * c.max_total_gradient_samples,
                max_total_match_comparisons: 6 * c.max_total_match_comparisons,
                max_total_union_comparisons: 6 * c.max_union_comparisons,
            },
            anchors: p,
            maximum_total_point_checks: 6 * p.local.maximum_point_checks,
            maximum_total_selection_checks: 6 * p.maximum_selection_checks,
            maximum_total_rank_sites: 6 * p.local.rank.maximum_sites,
            maximum_total_pixel_reads: 6 * p.local.rank.maximum_pixel_reads,
        },
        maximum_reflection_bins: 768 * c.search.gradient.search.max_total_features as u64,
    }
}
fn opposite_policy() -> rrrah_dedup::anchor_rank_file::OppositeAnchorSearchPolicy {
    let search = portfolio();
    rrrah_dedup::anchor_rank_file::OppositeAnchorSearchPolicy {
        maximum_swapped_points: search.search.search.search.max_union_points,
        search,
    }
}
#[test]
fn opposite_real_fixture_support_reflection_and_release() {
    use rrrah_dedup::anchor_rank_file::search_anchor_rank_opposite_files;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let left = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    for (name, supported) in [
        ("base.png", true),
        ("mirrored.png", true),
        ("unrelated.png", false),
    ] {
        let right = rrrah_decode::DecodeRequest::new(root.join(name));
        let e =
            search_anchor_rank_opposite_files(&left, &right, opposite_policy(), &factors(), &budget, || {
                false
            })
            .unwrap();
        assert_eq!(e.anchors.is_some_and(|a| a.supported), supported, "{name}");
        assert!((1..=6).contains(&e.attempted_recipes));
        if supported {
            assert!(e.transform.is_some());
            assert!(!e.correspondences.is_empty());
        }
        drop(e);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn opposite_preflight_swap_work_overflow_memory_and_cancel_before_io() {
    use rrrah_dedup::anchor_rank_file::{AnchorRankFileError, search_anchor_rank_opposite_files};
    let missing = rrrah_decode::DecodeRequest::new("does-not-exist-opposite-native");
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    for case in 0..4 {
        let mut p = opposite_policy();
        match case {
            0 => p.maximum_swapped_points = 0,
            1 => p.maximum_swapped_points = usize::MAX,
            2 => p.search.maximum_reflection_bins -= 1,
            _ => p.search.search.maximum_total_point_checks = 0,
        }
        assert!(matches!(
            search_anchor_rank_opposite_files(&missing, &missing, p, &factors(), &budget, || false),
            Err(AnchorRankFileError::Rank(_))
        ));
        assert_eq!(budget.used(), 0);
    }
    let small = rrrah_core::MemoryBudget::new(1);
    assert!(matches!(
        search_anchor_rank_opposite_files(&missing, &missing, opposite_policy(), &factors(), &small, || {
            false
        }),
        Err(AnchorRankFileError::Rank(_))
    ));
    assert_eq!(small.used(), 0);
    assert!(matches!(
        search_anchor_rank_opposite_files(&missing, &missing, opposite_policy(), &factors(), &budget, || {
            true
        }),
        Err(AnchorRankFileError::Cancelled)
    ));
}
#[test]
fn opposite_unknown_is_preserved_and_releases() {
    use rrrah_dedup::anchor_rank_file::{AnchorRankFileError, search_anchor_rank_opposite_files};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png");
    let file = rrrah_decode::DecodeRequest::new(root);
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let mut p = opposite_policy();
    p.search.search.anchors.local.minimum_witnesses = 1000;
    assert!(matches!(
        search_anchor_rank_opposite_files(&file, &file, p, &factors(), &budget, || false),
        Err(AnchorRankFileError::Rank(
            rrrah_dedup::local_rank::LocalRankError::InsufficientGeometry
        ))
    ));
    assert_eq!(budget.used(), 0);
}
#[test]
fn opposite_final_callback_mutation_cannot_publish() {
    use rrrah_dedup::anchor_rank_file::{AnchorRankFileError, search_anchor_rank_opposite_files};
    use std::cell::Cell;
    let dir = tempfile::tempdir().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png");
    let original = std::fs::read(root).unwrap();
    let paths = [dir.path().join("left.png"), dir.path().join("right.png")];
    for path in &paths {
        std::fs::write(path, &original).unwrap();
    }
    let left = rrrah_decode::DecodeRequest::new(&paths[0]);
    let right = rrrah_decode::DecodeRequest::new(&paths[1]);
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let calls = Cell::new(0);
    drop(
        search_anchor_rank_opposite_files(&left, &right, opposite_policy(), &factors(), &budget, || {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap(),
    );
    let total = calls.get();
    assert!(total > 10);
    for owner in 0..2 {
        let seen = Cell::new(0);
        let changed = Cell::new(false);
        let result =
            search_anchor_rank_opposite_files(&left, &right, opposite_policy(), &factors(), &budget, || {
                seen.set(seen.get() + 1);
                if seen.get() == total {
                    let mut data = original.clone();
                    let n = data.len();
                    data[n - 1] ^= 1;
                    std::fs::write(&paths[owner], data).unwrap();
                    changed.set(true);
                }
                false
            });
        assert!(changed.get());
        assert!(
            matches!(result, Err(AnchorRankFileError::Source(_))),
            "{result:?}"
        );
        assert_eq!(budget.used(), 0);
        std::fs::write(&paths[owner], &original).unwrap();
    }
    for stop in [1, total / 2, total] {
        let seen = Cell::new(0);
        let result =
            search_anchor_rank_opposite_files(&left, &right, opposite_policy(), &factors(), &budget, || {
                seen.set(seen.get() + 1);
                seen.get() == stop
            });
        assert!(matches!(result, Err(AnchorRankFileError::Cancelled)));
        assert_eq!(budget.used(), 0);
    }
    drop(
        search_anchor_rank_opposite_files(&left, &right, opposite_policy(), &factors(), &budget, || false)
            .unwrap(),
    );
    assert_eq!(budget.used(), 0);
}

fn joint_policy() -> rrrah_dedup::anchor_rank_file::JointOppositeAnchorSearchPolicy {
    let opposite = opposite_policy();
    let phase = opposite.search.search.search.search;
    let g = phase.search.gradient.search;
    rrrah_dedup::anchor_rank_file::JointOppositeAnchorSearchPolicy {
        opposite,
        maximum_total_projection_checks: 6 * 2 * g.local.geometry.max_points as u64 * (g.sampling.trials + 1),
    }
}
#[test]
fn joint_opposite_preflight_and_unknown_do_not_publish_negative() {
    use rrrah_dedup::anchor_rank_file::{AnchorRankFileError, search_anchor_rank_joint_opposite_files};
    let missing = rrrah_decode::DecodeRequest::new("missing-joint-opposite");
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let mut p = joint_policy();
    p.maximum_total_projection_checks -= 1;
    assert!(matches!(
        search_anchor_rank_joint_opposite_files(&missing, &missing, p, &factors(), &budget, || false),
        Err(AnchorRankFileError::Search(
            rrrah_dedup::local_scan::LocalFileError::Geometry(rrrah_dedup::geometry::GeometryError::Budget)
        ))
    ));
    assert!(matches!(
        search_anchor_rank_joint_opposite_files(&missing, &missing, p, &factors(), &budget, || true),
        Err(AnchorRankFileError::Cancelled)
    ));
    let file = rrrah_decode::DecodeRequest::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"),
    );
    let mut p = joint_policy();
    p.opposite
        .search
        .search
        .search
        .search
        .search
        .gradient
        .search
        .local
        .geometry
        .min_inliers = 2000;
    assert!(matches!(
        search_anchor_rank_joint_opposite_files(&file, &file, p, &factors(), &budget, || false),
        Err(AnchorRankFileError::Rank(
            rrrah_dedup::local_rank::LocalRankError::InsufficientGeometry
        ))
    ));
    assert_eq!(budget.used(), 0);
}
#[test]
fn joint_opposite_original_pixels_support_and_release() {
    use rrrah_dedup::anchor_rank_file::search_anchor_rank_joint_opposite_files;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let left = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    for name in ["base.png", "mirrored.png"] {
        let right = rrrah_decode::DecodeRequest::new(root.join(name));
        let e = search_anchor_rank_joint_opposite_files(
            &left,
            &right,
            joint_policy(),
            &factors(),
            &budget,
            || false,
        )
        .unwrap();
        assert!(e.anchors.is_some_and(|a| a.supported), "{name}");
        drop(e);
        assert_eq!(budget.used(), 0);
    }
}
