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
#[test]
fn ordinary_mirror_and_foreign_select_correct_six_phase_recipes() {
    use rrrah_dedup::anchor_rank_file::{
        search_anchor_rank_reflected_geometry_files, search_anchor_rank_reflection_fallback_files,
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let source = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let policy = portfolio();
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    for (name, attempts, reflected, supported) in [
        ("base.png", 1, false, true),
        ("mirrored.png", 4, true, true),
        ("unrelated.png", 6, true, false),
    ] {
        let target = rrrah_decode::DecodeRequest::new(root.join(name));
        let e = search_anchor_rank_reflection_fallback_files(
            &source,
            &target,
            policy,
            &factors(),
            &budget,
            || false,
        )
        .unwrap();
        assert_eq!(
            (e.attempted_recipes, e.reflected_source),
            (attempts, reflected),
            "{e:?}"
        );
        assert_eq!(e.evidence.anchors.is_some_and(|a| a.supported), supported);
        assert_eq!(e.smoothing_radii, if attempts == 6 { [0, 4] } else { [2, 2] });
        if reflected && supported {
            let direct = search_anchor_rank_reflected_geometry_files(
                &source,
                &target,
                config(),
                &factors(),
                [2, 2],
                anchors(),
                policy.maximum_reflection_bins / 3,
                &budget,
                || false,
            )
            .unwrap();
            assert_eq!(e.evidence.anchors, direct.anchors);
            assert_eq!(
                e.evidence
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
            drop(direct);
        }
        drop(e);
        assert_eq!(budget.used(), 0);
    }
}
#[test]
fn all_eight_cumulative_caps_refuse_before_missing_io_and_cancel_wins() {
    use rrrah_dedup::anchor_rank_file::{search_anchor_rank_reflection_fallback_files, AnchorRankFileError};
    let directory = tempfile::tempdir().unwrap();
    let missing = rrrah_decode::DecodeRequest::new(directory.path().join("missing.png"));
    for case in 0..8 {
        let mut bad = portfolio();
        match case {
            0 => bad.search.maximum_total_point_checks -= 1,
            1 => bad.search.maximum_total_selection_checks -= 1,
            2 => bad.search.maximum_total_rank_sites -= 1,
            3 => bad.search.maximum_total_pixel_reads -= 1,
            4 => bad.search.search.max_total_gradient_samples -= 1,
            5 => bad.search.search.max_total_match_comparisons -= 1,
            6 => bad.search.search.max_total_union_comparisons -= 1,
            _ => bad.maximum_reflection_bins -= 1,
        }
        for cancelled in [false, true] {
            let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
            let result = search_anchor_rank_reflection_fallback_files(
                &missing,
                &missing,
                bad,
                &factors(),
                &budget,
                || cancelled,
            );
            if cancelled {
                assert!(matches!(result, Err(AnchorRankFileError::Cancelled)));
            } else {
                assert!(
                    matches!(
                        result,
                        Err(AnchorRankFileError::Rank(
                            rrrah_dedup::local_rank::LocalRankError::Budget
                        ))
                    ),
                    "case={case}: {result:?}"
                );
            }
            assert_eq!(budget.peak(), 0);
        }
    }
}

#[test]
fn six_phase_cancellation_and_both_source_mutations_release_memory() {
    use rrrah_dedup::anchor_rank_file::{search_anchor_rank_reflection_fallback_files, AnchorRankFileError};
    use std::cell::Cell;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let temporary = tempfile::tempdir().unwrap();
    let paths = [
        temporary.path().join("left.png"),
        temporary.path().join("right.png"),
    ];
    let bytes = [
        std::fs::read(root.join("base.png")).unwrap(),
        std::fs::read(root.join("mirrored.png")).unwrap(),
    ];
    for i in 0..2 {
        std::fs::write(&paths[i], &bytes[i]).unwrap();
    }
    let left = rrrah_decode::DecodeRequest::new(&paths[0]);
    let right = rrrah_decode::DecodeRequest::new(&paths[1]);
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let seen = Cell::new(0usize);
    let baseline =
        search_anchor_rank_reflection_fallback_files(&left, &right, portfolio(), &factors(), &budget, || {
            seen.set(seen.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(baseline.attempted_recipes, 4);
    let calls = seen.get();
    drop(baseline);
    assert_eq!(budget.used(), 0);
    for stop in [1, calls / 2, calls] {
        let seen = Cell::new(0);
        let result = search_anchor_rank_reflection_fallback_files(
            &left,
            &right,
            portfolio(),
            &factors(),
            &budget,
            || {
                seen.set(seen.get() + 1);
                seen.get() == stop
            },
        );
        assert!(
            matches!(result, Err(AnchorRankFileError::Cancelled)),
            "{result:?}"
        );
        assert_eq!(budget.used(), 0);
    }
    for owner in 0..2 {
        for stop in [calls / 2, calls - 10] {
            let seen = Cell::new(0);
            let changed = Cell::new(false);
            let result = search_anchor_rank_reflection_fallback_files(
                &left,
                &right,
                portfolio(),
                &factors(),
                &budget,
                || {
                    seen.set(seen.get() + 1);
                    if seen.get() == stop {
                        let mut altered = bytes[owner].clone();
                        altered.push(0);
                        std::fs::write(&paths[owner], altered).unwrap();
                        changed.set(true);
                    }
                    false
                },
            );
            assert!(changed.get());
            assert!(
                matches!(
                    result,
                    Err(AnchorRankFileError::Source(_)) | Err(AnchorRankFileError::Search(_))
                ),
                "owner={owner}: {result:?}"
            );
            assert_eq!(budget.used(), 0);
            std::fs::write(&paths[owner], &bytes[owner]).unwrap();
        }
    }
    let restored =
        search_anchor_rank_reflection_fallback_files(&left, &right, portfolio(), &factors(), &budget, || {
            false
        })
        .unwrap();
    assert!(restored.evidence.anchors.unwrap().supported);
    drop(restored);
    assert_eq!(budget.used(), 0);
}

fn final_native_callback_mutation_is_rejected(six: bool, inconclusive: bool) {
    use rrrah_dedup::anchor_rank_file::{
        search_anchor_rank_geometry_fallback_files, search_anchor_rank_reflection_fallback_files,
        AnchorRankFileError,
    };
    use std::cell::Cell;
    let dir = tempfile::tempdir().unwrap();
    let original = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"),
    )
    .unwrap();
    let paths = [dir.path().join("left.png"), dir.path().join("right.png")];
    for path in &paths {
        std::fs::write(path, &original).unwrap();
    }
    let left = rrrah_decode::DecodeRequest::new(&paths[0]);
    let right = rrrah_decode::DecodeRequest::new(&paths[1]);
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let mut p = portfolio();
    if inconclusive {
        p.search.anchors.local.minimum_witnesses = 1000;
    } else {
        p.search.search.search.low_contrast_corner_score = 1e9;
        p.search
            .search
            .search
            .search
            .gradient
            .search
            .local
            .extract
            .minimum_corner_score = 1e9;
    }
    let run = |cancel: &dyn Fn() -> bool| -> Result<(), AnchorRankFileError> {
        if six {
            drop(search_anchor_rank_reflection_fallback_files(
                &left,
                &right,
                p,
                &factors(),
                &budget,
                cancel,
            )?);
        } else {
            drop(search_anchor_rank_geometry_fallback_files(
                &left,
                &right,
                p.search,
                &factors(),
                &budget,
                cancel,
            )?);
        }
        Ok(())
    };
    let count = Cell::new(0);
    let baseline = run(&|| {
        count.set(count.get() + 1);
        false
    });
    if inconclusive {
        assert!(matches!(
            baseline,
            Err(AnchorRankFileError::Rank(
                rrrah_dedup::local_rank::LocalRankError::InsufficientGeometry
            ))
        ));
    } else {
        baseline.unwrap();
    }
    let last = count.get();
    assert_eq!(budget.used(), 0);
    for path in &paths {
        let n = Cell::new(0);
        let result = run(&|| {
            n.set(n.get() + 1);
            if n.get() == last {
                let mut changed = original.clone();
                changed.push(0);
                std::fs::write(path, changed).unwrap();
            }
            false
        });
        assert!(
            matches!(result, Err(AnchorRankFileError::Source(_))),
            "six={six}: final callback published stale result: {result:?}"
        );
        assert_eq!(budget.used(), 0);
        std::fs::write(path, &original).unwrap();
    }
}
#[test]
fn three_recipe_final_callback_cannot_publish_mutated_source() {
    final_native_callback_mutation_is_rejected(false, false);
}
#[test]
fn six_recipe_final_callback_cannot_publish_mutated_source() {
    final_native_callback_mutation_is_rejected(true, false);
}

#[test]
fn insufficient_anchor_geometry_continues_remaining_native_recipes() {
    use rrrah_dedup::anchor_rank_file::{
        search_anchor_rank_geometry_fallback_files, search_anchor_rank_geometry_files_with_smoothing,
        search_anchor_rank_reflection_fallback_files, AnchorRankFileError,
    };
    use rrrah_dedup::local_rank::LocalRankError;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");
    let source = rrrah_decode::DecodeRequest::new(root.join("base.png"));
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let mut p = portfolio();
    p.search.anchors.local.minimum_witnesses = 1000;
    let checkpoints = std::cell::Cell::new(0usize);
    let checkpoint = || {
        checkpoints.set(checkpoints.get() + 1);
        false
    };
    let first = search_anchor_rank_geometry_files_with_smoothing(
        &source,
        &source,
        p.search.search.search,
        &factors(),
        [p.search.search.search.smoothing_radius; 2],
        p.search.anchors,
        &budget,
        checkpoint,
    );
    assert!(matches!(
        first,
        Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry))
    ));
    let first_checks = checkpoints.replace(0);
    let three = search_anchor_rank_geometry_fallback_files(
        &source,
        &source,
        p.search,
        &factors(),
        &budget,
        checkpoint,
    );
    assert!(matches!(
        three,
        Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry))
    ));
    let three_checks = checkpoints.replace(0);
    assert!(
        three_checks > 2 * first_checks,
        "three recipes stopped after the first error"
    );
    let six =
        search_anchor_rank_reflection_fallback_files(&source, &source, p, &factors(), &budget, checkpoint);
    assert!(matches!(
        six,
        Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry))
    ));
    assert!(checkpoints.get() > three_checks, "reflected recipes were skipped");
    assert_eq!(budget.used(), 0);
}

#[test]
fn final_callback_source_mutation_precedes_native_inconclusive_error() {
    for six in [false, true] {
        final_native_callback_mutation_is_rejected(six, true);
    }
}

fn standalone_final_source_guard(case: usize) {
    use rrrah_dedup::anchor_rank_file::*;
    use rrrah_dedup::geometry::{Correspondence, ProjectiveTransform};
    let dir = tempfile::tempdir().unwrap();
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"),
    )
    .unwrap();
    let paths = [dir.path().join("left.png"), dir.path().join("right.png")];
    for path in &paths {
        std::fs::write(path, &bytes).unwrap();
    }
    let left = rrrah_decode::DecodeRequest::new(&paths[0]);
    let right = rrrah_decode::DecodeRequest::new(&paths[1]);
    let budget = rrrah_core::MemoryBudget::new(128 * 1024 * 1024);
    let mut p = portfolio();
    p.search.search.search.low_contrast_corner_score = 1e9;
    p.search
        .search
        .search
        .search
        .gradient
        .search
        .local
        .extract
        .minimum_corner_score = 1e9;
    let points: Vec<_> = (0..10)
        .map(|i| {
            let xy = [20. + 24. * (i % 5) as f64, 20. + 24. * (i / 5) as f64];
            Correspondence {
                source: xy,
                target: xy,
            }
        })
        .collect();
    let run = |cancel: &dyn Fn() -> bool| -> Result<(), AnchorRankFileError> {
        match case {
            0 => {
                compare_anchor_rank_files(
                    &left,
                    &right,
                    ProjectiveTransform {
                        matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                    },
                    &points,
                    p.search.search.search.search.gradient.search.local.decode,
                    p.search.anchors,
                    &budget,
                    cancel,
                )?;
            }
            1 => {
                drop(search_anchor_rank_files(
                    &left,
                    &right,
                    p.search.search.search,
                    &factors(),
                    p.search.anchors,
                    &budget,
                    cancel,
                )?);
            }
            2 => {
                drop(search_anchor_rank_fallback_files(
                    &left,
                    &right,
                    p.search.search,
                    &factors(),
                    p.search.anchors,
                    &budget,
                    cancel,
                )?);
            }
            3 => {
                drop(search_anchor_rank_geometry_files_with_smoothing(
                    &left,
                    &right,
                    p.search.search.search,
                    &factors(),
                    [2, 2],
                    p.search.anchors,
                    &budget,
                    cancel,
                )?);
            }
            4 => {
                drop(search_anchor_rank_reflected_geometry_files(
                    &left,
                    &right,
                    p.search.search.search,
                    &factors(),
                    [2, 2],
                    p.search.anchors,
                    p.maximum_reflection_bins / 3,
                    &budget,
                    cancel,
                )?);
            }
            _ => unreachable!(),
        }
        Ok(())
    };
    let count = std::cell::Cell::new(0usize);
    run(&|| {
        count.set(count.get() + 1);
        false
    })
    .unwrap();
    let last = count.get();
    assert_eq!(budget.used(), 0);
    for path in &paths {
        count.set(0);
        let result = run(&|| {
            count.set(count.get() + 1);
            if count.get() == last {
                let mut changed = bytes.clone();
                changed.push(0);
                std::fs::write(path, changed).unwrap();
            }
            false
        });
        assert!(
            matches!(result, Err(AnchorRankFileError::Source(_))),
            "case={case}: stale publication {result:?}"
        );
        assert_eq!(budget.used(), 0);
        std::fs::write(path, &bytes).unwrap();
    }
}
#[test]
fn standalone_rank_final_callback_guards_source() {
    standalone_final_source_guard(0);
}
#[test]
fn standalone_color_candidate_final_callback_guards_source() {
    standalone_final_source_guard(1);
}
#[test]
fn standalone_color_fallback_final_callback_guards_source() {
    standalone_final_source_guard(2);
}
#[test]
fn standalone_geometry_final_callback_guards_source() {
    standalone_final_source_guard(3);
}
#[test]
fn standalone_reflected_geometry_final_callback_guards_source() {
    standalone_final_source_guard(4);
}
