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

mod model_fixture {

    use rrrah_core::{MemoryBudget, SharedBuffer};
    use rrrah_decode::DecodeRequest;
    use rrrah_dedup::{
        anchor_rank::AnchorRankPolicy,
        animated::AnimationBudget,
        geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
        local_rank::LocalRankPolicy,
        model_compare::{ModelComparisonPolicy, ProposalProviderError},
        model_input::ModelInputPolicy,
        proposals::{ProposalPolicy, ScoredProposal},
        rank_region::RankRegionPolicy,
    };
    use std::path::Path;
    pub(super) fn policy() -> ModelComparisonPolicy {
        ModelComparisonPolicy {
            decode: AnimationBudget {
                max_frames: 1,
                max_pixels: 4096,
                max_file_bytes: 65536,
            },
            input: ModelInputPolicy {
                max_side: 64,
                max_source_pixels: 4096,
                max_output_pixels: 4096,
                max_pixel_reads: 4096,
            },
            proposals: ProposalPolicy {
                max_proposals: 16,
                max_ranking_checks: 10000,
                max_separation_checks: 10000,
                minimum_separation: 2.,
            },
            geometry: GeometryPolicy {
                tolerance: 0.01,
                min_inliers: 10,
                max_points: 16,
                max_hypotheses: 64,
            },
            sampling: ProjectiveSamplingPolicy {
                trials: 64,
                seed: 173,
            },
            anchors: AnchorRankPolicy {
                local: LocalRankPolicy {
                    rank: RankRegionPolicy {
                        radius: 2,
                        minimum_contrast: 0.001,
                        minimum_pairs: 100,
                        maximum_sites: 100000,
                        maximum_pixel_reads: 500000,
                    },
                    filter_radius: 0,
                    minimum_witnesses: 10,
                    maximum_points: 16,
                    maximum_point_checks: 10000,
                    minimum_point_separation: 2.,
                    target_tolerance: 0.01,
                    source_tolerance: 0.01,
                    minimum_coverage: 0.3,
                    minimum_agreement: 0.9,
                },
                window_radius: 1,
                maximum_selection_checks: 10000,
            },
            max_model_bytes: 1024,
        }
    }
    pub(super) fn fixture(dir: &Path) -> ([DecodeRequest; 2], std::path::PathBuf) {
        let mut b = Vec::new();
        b.extend(b"BM");
        b.extend((54u32 + 64 * 64 * 3).to_le_bytes());
        b.extend([0; 4]);
        b.extend(54u32.to_le_bytes());
        b.extend(40u32.to_le_bytes());
        b.extend(64i32.to_le_bytes());
        b.extend(64i32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(24u16.to_le_bytes());
        b.extend([0; 24]);
        for y in 0..64u32 {
            for x in 0..64u32 {
                b.extend([(x * y / 16) as u8, (y * 3) as u8, (x * 3) as u8]);
            }
        }
        let requests = std::array::from_fn(|i| {
            let p = dir.join(format!("{i}.bmp"));
            std::fs::write(&p, &b).unwrap();
            DecodeRequest::new(p)
        });
        let weights = dir.join("weights.bin");
        std::fs::write(&weights, b"fixture provider only").unwrap();
        (requests, weights)
    }
    pub(super) fn proposals(
        budget: &MemoryBudget,
    ) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
        let mut b = budget.try_buffer(
            10,
            ScoredProposal {
                source: [0.; 2],
                target: [0.; 2],
                confidence: 1.,
            },
        )?;
        for (i, p) in b.iter_mut().enumerate() {
            let position = [8. + (i % 5) as f64 * 8., 8. + (i / 5) as f64 * 16.];
            p.source = position;
            p.target = position;
        }
        Ok(b.freeze())
    }
}

use rrrah_core::MemoryBudget;
use rrrah_decode::DecodeRequest;
use rrrah_dedup::search_portfolio::*;
use std::{cell::Cell, error::Error, path::Path};
type TestResult = Result<(), Box<dyn Error + Send + Sync>>;
fn combined(order: SearchOrder) -> SearchPortfolioPolicy {
    let native = portfolio();
    let model = model_fixture::policy();
    let n = native.search.anchors;
    let m = model.anchors;
    SearchPortfolioPolicy {
        native,
        model,
        order,
        model_tiles: None,
        work: PortfolioWorkLimits {
            point_checks: 6 * n.local.maximum_point_checks + m.local.maximum_point_checks,
            selection_checks: 6 * n.maximum_selection_checks + m.maximum_selection_checks,
            rank_sites: 6 * n.local.rank.maximum_sites + m.local.rank.maximum_sites,
            pixel_reads: 6 * n.local.rank.maximum_pixel_reads + m.local.rank.maximum_pixel_reads,
            geometry_hypotheses: 6 * 4096 + model.sampling.trials,
            model_input_reads: 2 * model.input.max_pixel_reads,
        },
    }
}
fn native_miss(p: &mut SearchPortfolioPolicy) {
    p.native.search.search.search.low_contrast_corner_score = 1e9;
    p.native
        .search
        .search
        .search
        .search
        .gradient
        .search
        .local
        .extract
        .minimum_corner_score = 1e9;
}

fn tiled(mut p: SearchPortfolioPolicy) -> SearchPortfolioPolicy {
    p.model_tiles = Some(rrrah_dedup::model_tiles::ModelTileWorkLimits::for_model(p.model).unwrap());
    p.work = p.required_work().unwrap();
    p
}

#[test]
fn automatic_tiles_recover_after_full_misses_in_both_orders_and_exhaust_before_native_fallback() -> TestResult
{
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let budget = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        let mut p = tiled(combined(order));
        native_miss(&mut p);
        let calls = Cell::new(0);
        let evidence = search_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            &[1.],
            &budget,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                let count = if calls.get() == 11 { 10 } else { 0 };
                let mut points = b.try_buffer(
                    count,
                    rrrah_dedup::proposals::ScoredProposal {
                        source: [0.; 2],
                        target: [0.; 2],
                        confidence: 1.,
                    },
                )?;
                for (i, point) in points.iter_mut().enumerate() {
                    point.source = [4. + (i % 5) as f64 * 4., 12. + (i / 5) as f64 * 8.];
                    point.target = [point.source[0] + 32., point.source[1]];
                }
                Ok(points.freeze())
            },
            || false,
        )?;
        assert!(evidence.supported());
        assert_eq!(calls.get(), 11);
        assert_eq!(
            evidence.attempted_stages,
            if order == SearchOrder::ModelFirst { 1 } else { 2 }
        );
        let SearchStageEvidence::ModelTiles(tiles) = &evidence.terminal else {
            panic!("expected tiled model evidence")
        };
        assert_eq!(tiles.attempts().len(), 11);
        assert_eq!(tiles.terminal().left_rectangle, [32, 0, 32, 32]);
        let retained = tiles.clone();
        drop(evidence);
        assert!(budget.used() > 0);
        drop(retained);
        assert_eq!(budget.used(), 0);
    }
    let mut p = tiled(combined(SearchOrder::ModelFirst));
    native_miss(&mut p);
    let calls = Cell::new(0);
    let evidence = search_portfolio_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        &[1.],
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            Ok(b.try_buffer(
                0,
                rrrah_dedup::proposals::ScoredProposal {
                    source: [0.; 2],
                    target: [0.; 2],
                    confidence: 0.,
                },
            )?
            .freeze())
        },
        || false,
    )?;
    assert_eq!(calls.get(), 25);
    assert!(!evidence.supported());
    assert!(matches!(
        evidence.first_miss,
        Some(SearchStageMiss::ModelTiles { attempts: 25, .. })
    ));
    assert!(matches!(evidence.terminal, SearchStageEvidence::Native(_)));
    drop(evidence);
    assert_eq!(budget.used(), 0);
    Ok(())
}

#[test]
fn tiled_and_aggregate_work_refuse_before_io_even_in_native_first_order() -> TestResult {
    let r = rrrah_decode::DecodeRequest::new("missing");
    let budget = rrrah_core::MemoryBudget::new(0);
    let p = tiled(combined(SearchOrder::NativeFirst));
    let mut invalid = p;
    invalid.model_tiles.as_mut().unwrap().proposal_ranking_checks -= 1;
    assert!(matches!(
        search_portfolio_files_with(
            &r,
            &r,
            Path::new("missing"),
            invalid,
            &[1.],
            &budget,
            |_, _, _, _, _| panic!("provider must not run"),
            || false
        ),
        Err(SearchPortfolioError::ModelTiles(_))
    ));
    for field in 0..6 {
        let mut invalid = p;
        *match field {
            0 => &mut invalid.work.point_checks,
            1 => &mut invalid.work.selection_checks,
            2 => &mut invalid.work.rank_sites,
            3 => &mut invalid.work.pixel_reads,
            4 => &mut invalid.work.geometry_hypotheses,
            _ => &mut invalid.work.model_input_reads,
        } -= 1;
        assert!(matches!(
            search_portfolio_files_with(
                &r,
                &r,
                Path::new("missing"),
                invalid,
                &[1.],
                &budget,
                |_, _, _, _, _| panic!("provider must not run"),
                || false
            ),
            Err(SearchPortfolioError::Work)
        ));
    }
    assert_eq!((budget.used(), budget.peak()), (0, 0));
    Ok(())
}

#[test]
fn portfolio_final_user_checkpoint_cannot_publish_mutated_sources_or_weights() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let budget = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
    let p = combined(SearchOrder::ModelFirst);
    let calls = Cell::new(0);
    drop(search_portfolio_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        &[1.],
        &budget,
        |_, _, _, b, _| model_fixture::proposals(b),
        || {
            calls.set(calls.get() + 1);
            false
        },
    )?);
    let last = calls.get();
    for path in [&r[0].path, &r[1].path, &w] {
        let original = std::fs::read(path)?;
        let current = Cell::new(0);
        let e = search_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            &[1.],
            &budget,
            |_, _, _, b, _| model_fixture::proposals(b),
            || {
                current.set(current.get() + 1);
                if current.get() == last {
                    let mut changed = original.clone();
                    changed.push(0);
                    std::fs::write(path, changed).unwrap();
                }
                false
            },
        );
        assert!(matches!(e, Err(SearchPortfolioError::Source(_))), "{e:?}");
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original)?;
    }
    Ok(())
}
#[test]
fn both_orders_stop_at_support_and_misses_advance_without_leaked_payload() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (requests, weights) = model_fixture::fixture(dir.path());
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    for order in [SearchOrder::ModelFirst, SearchOrder::NativeFirst] {
        let mut p = combined(order);
        native_miss(&mut p);
        let calls = Cell::new(0);
        let e = search_portfolio_files_with(
            &requests[0],
            &requests[1],
            &weights,
            p,
            &factors(),
            &budget,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                model_fixture::proposals(b)
            },
            || false,
        )?;
        assert!(e.supported());
        assert_eq!(calls.get(), 1);
        assert_eq!(
            e.attempted_stages,
            if order == SearchOrder::ModelFirst { 1 } else { 2 }
        );
        assert!(matches!(e.terminal, SearchStageEvidence::Model(_)));
        if order == SearchOrder::NativeFirst {
            assert!(matches!(
                e.first_miss,
                Some(SearchStageMiss::Native {
                    recipes: 6,
                    candidates: 0
                })
            ));
        }
        drop(e);
        assert_eq!(budget.used(), 0);
    }
    Ok(())
}
#[test]
fn native_early_support_never_calls_model_and_model_miss_falls_through() -> TestResult {
    let dir = tempfile::tempdir()?;
    let weights = dir.path().join("weights");
    std::fs::write(&weights, b"fixture provider")?;
    let source = DecodeRequest::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"),
    );
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        let mut p = combined(order);
        p.model.decode.max_pixels = 200000;
        p.model.input.max_source_pixels = 200000;
        p.model.input.max_pixel_reads = 800000;
        p.work.model_input_reads = 1600000;
        let calls = Cell::new(0);
        let e = search_portfolio_files_with(
            &source,
            &source,
            &weights,
            p,
            &factors(),
            &budget,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                Ok(b.try_buffer(
                    0,
                    rrrah_dedup::proposals::ScoredProposal {
                        source: [0.; 2],
                        target: [0.; 2],
                        confidence: 0.,
                    },
                )?
                .freeze())
            },
            || false,
        )?;
        assert!(e.supported());
        assert_eq!(calls.get(), if order == SearchOrder::NativeFirst { 0 } else { 1 });
        assert_eq!(
            e.attempted_stages,
            if order == SearchOrder::NativeFirst { 1 } else { 2 }
        );
        assert!(matches!(e.terminal, SearchStageEvidence::Native(_)));
        drop(e);
        assert_eq!(budget.used(), 0);
    }
    Ok(())
}
#[test]
fn cumulative_and_both_policy_preflight_precede_io_cancel_wins() -> TestResult {
    let missing = DecodeRequest::new("/nonexistent/portfolio.png");
    let weights = std::path::Path::new("/nonexistent/portfolio.weights");
    let budget = MemoryBudget::new(1024);
    for case in 0..10 {
        let mut p = combined(SearchOrder::NativeFirst);
        match case {
            0 => p.work.point_checks -= 1,
            1 => p.work.selection_checks -= 1,
            2 => p.work.rank_sites -= 1,
            3 => p.work.pixel_reads -= 1,
            4 => p.work.geometry_hypotheses -= 1,
            5 => p.work.model_input_reads -= 1,
            6 => p.model.sampling.seed = 0,
            7 => p.native.maximum_reflection_bins -= 1,
            8 => {
                p.model.geometry.max_hypotheses = u64::MAX;
                p.model.sampling.trials = u64::MAX;
                p.work.geometry_hypotheses = u64::MAX;
            }
            _ => {
                p.model.input.max_pixel_reads = u64::MAX;
                p.work.model_input_reads = u64::MAX;
            }
        }
        let run = |stop| {
            search_portfolio_files_with(
                &missing,
                &missing,
                weights,
                p,
                &factors(),
                &budget,
                |_, _, _, _, _| panic!("preflight must not call model"),
                || stop,
            )
        };
        let error = run(false).err().ok_or("expected policy refusal")?;
        assert!(matches!(
            error,
            SearchPortfolioError::Work | SearchPortfolioError::Native(_) | SearchPortfolioError::Model(_)
        ));
        assert!(matches!(run(true), Err(SearchPortfolioError::Cancelled)));
        assert_eq!(budget.peak(), 0);
    }
    Ok(())
}
#[test]
fn cross_stage_source_or_weight_changes_and_provider_errors_never_become_misses() -> TestResult {
    for change in 0..3 {
        let dir = tempfile::tempdir()?;
        let (requests, weights) = model_fixture::fixture(dir.path());
        let budget = MemoryBudget::new(128 * 1024 * 1024);
        let mut p = combined(SearchOrder::ModelFirst);
        native_miss(&mut p);
        let provider_done = Cell::new(false);
        let changed = Cell::new(false);
        let e = search_portfolio_files_with(
            &requests[0],
            &requests[1],
            &weights,
            p,
            &factors(),
            &budget,
            |_, _, _, b, _| {
                provider_done.set(true);
                Ok(b.try_buffer(
                    0,
                    rrrah_dedup::proposals::ScoredProposal {
                        source: [0.; 2],
                        target: [0.; 2],
                        confidence: 0.,
                    },
                )?
                .freeze())
            },
            || {
                if provider_done.get() && budget.used() == 0 && !changed.replace(true) {
                    let path = if change < 2 {
                        &requests[change].path
                    } else {
                        &weights
                    };
                    std::fs::write(path, b"changed between model return and native fallback")
                        .expect("fixture mutation");
                }
                false
            },
        );
        assert!(changed.get());
        assert!(matches!(e, Err(SearchPortfolioError::Source(_))), "{e:?}");
        assert_eq!(budget.used(), 0);
    }
    let dir = tempfile::tempdir()?;
    let (requests, weights) = model_fixture::fixture(dir.path());
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let e = search_portfolio_files_with(
        &requests[0],
        &requests[1],
        &weights,
        combined(SearchOrder::ModelFirst),
        &factors(),
        &budget,
        |_, _, _, _, _| Err("provider refused".into()),
        || false,
    );
    assert!(matches!(
        e,
        Err(SearchPortfolioError::Model(
            rrrah_dedup::model_compare::ModelComparisonError::Provider(_)
        ))
    ));
    assert_eq!(budget.used(), 0);
    Ok(())
}

fn empty_provider(
    _: &std::path::Path,
    _: &rrrah_dedup::model_input::GrayModelInput,
    _: &rrrah_dedup::model_input::GrayModelInput,
    budget: &MemoryBudget,
    _: &dyn Fn() -> bool,
) -> Result<
    rrrah_core::SharedBuffer<rrrah_dedup::proposals::ScoredProposal>,
    rrrah_dedup::model_compare::ProposalProviderError,
> {
    Ok(budget
        .try_buffer(
            0,
            rrrah_dedup::proposals::ScoredProposal {
                source: [0.; 2],
                target: [0.; 2],
                confidence: 0.,
            },
        )?
        .freeze())
}

#[test]
fn global_and_both_generations_cancel_entry_and_terminal_publication() -> TestResult {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let dir = tempfile::tempdir()?;
    let (requests, weights) = model_fixture::fixture(dir.path());
    let mut policy = combined(SearchOrder::ModelFirst);
    native_miss(&mut policy);
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let count = Cell::new(0);
    let baseline = search_portfolio_files_with(
        &requests[0],
        &requests[1],
        &weights,
        policy,
        &factors(),
        &budget,
        empty_provider,
        || {
            count.set(count.get() + 1);
            false
        },
    )?;
    assert!(!baseline.supported());
    assert_eq!(baseline.attempted_stages, 2);
    drop(baseline);
    let total = count.get();
    assert!(total > 2);
    for stop in [1, total] {
        for mode in 0..3 {
            let mut requests = requests.clone();
            let generation = Arc::new(AtomicU64::new(1));
            if mode > 0 {
                requests[mode - 1].cancellation =
                    Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
            }
            let count = Cell::new(0);
            let result = search_portfolio_files_with(
                &requests[0],
                &requests[1],
                &weights,
                policy,
                &factors(),
                &budget,
                empty_provider,
                || {
                    count.set(count.get() + 1);
                    if count.get() == stop {
                        if mode == 0 {
                            return true;
                        }
                        generation.store(2, Ordering::Release);
                    }
                    false
                },
            );
            assert!(
                matches!(result, Err(SearchPortfolioError::Cancelled)),
                "{result:?}"
            );
            assert_eq!(count.get(), stop);
            assert_eq!(budget.used(), 0);
        }
    }
    Ok(())
}

#[test]
fn insufficient_model_anchor_geometry_advances_to_independently_verified_native_support() -> TestResult {
    let dir = tempfile::tempdir()?;
    let weights = dir.path().join("weights");
    std::fs::write(&weights, b"fixture provider")?;
    let source =
        DecodeRequest::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    for use_tiles in [false, true] {
        let mut p = combined(SearchOrder::ModelFirst);
        p.model.decode.max_pixels = 200000;
        p.model.input.max_source_pixels = 200000;
        p.model.input.max_pixel_reads = 800000;
        p.model.anchors.local.minimum_witnesses = 11;
        if use_tiles {
            p = tiled(p)
        } else {
            p.work = p.required_work()?;
        }
        let calls = Cell::new(0);
        let e = search_portfolio_files_with(
            &source,
            &source,
            &weights,
            p,
            &factors(),
            &budget,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    model_fixture::proposals(b)
                } else {
                    empty_model_proposals(b)
                }
            },
            || false,
        )?;
        assert!(e.supported());
        assert_eq!(e.attempted_stages, 2);
        assert!(matches!(e.terminal, SearchStageEvidence::Native(_)));
        assert_eq!(calls.get(), if use_tiles { 25 } else { 1 });
        match e.first_miss {
            Some(SearchStageMiss::InsufficientAnchorGeometry(InsufficientAnchorStage::ModelTiles {
                attempts: 25,
                insufficient_attempts: 1,
                ..
            })) if use_tiles => {}
            Some(SearchStageMiss::InsufficientAnchorGeometry(InsufficientAnchorStage::Model { .. }))
                if !use_tiles => {}
            other => panic!("inconclusive reason lost: {other:?}"),
        }
        drop(e);
        assert_eq!(budget.used(), 0);
    }
    Ok(())
}
#[test]
fn complete_native_miss_cannot_turn_model_insufficiency_into_an_ordinary_negative_in_either_order()
-> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    for order in [SearchOrder::ModelFirst, SearchOrder::NativeFirst] {
        for use_tiles in [false, true] {
            let mut p = combined(order);
            native_miss(&mut p);
            p.model.anchors.local.minimum_witnesses = 11;
            if use_tiles {
                p = tiled(p)
            } else {
                p.work = p.required_work()?;
            }
            let calls = Cell::new(0);
            let result = search_portfolio_files_with(
                &r[0],
                &r[1],
                &w,
                p,
                &factors(),
                &budget,
                |_, _, _, b, _| {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        model_fixture::proposals(b)
                    } else {
                        empty_model_proposals(b)
                    }
                },
                || false,
            );
            assert!(
                matches!(result, Err(SearchPortfolioError::InsufficientAnchorGeometry(_))),
                "{result:?}"
            );
            assert_eq!(calls.get(), if use_tiles { 25 } else { 1 });
            assert_eq!(budget.used(), 0);
        }
    }
    Ok(())
}
#[test]
fn final_source_mutation_and_cancellation_precede_portfolio_inconclusive_publication() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = combined(SearchOrder::ModelFirst);
    native_miss(&mut p);
    p.model.anchors.local.minimum_witnesses = 11;
    p.work = p.required_work()?;
    let run = |checkpoint: &dyn Fn() -> bool| {
        search_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            &factors(),
            &budget,
            |_, _, _, b, _| model_fixture::proposals(b),
            checkpoint,
        )
    };
    let count = Cell::new(0);
    let baseline = run(&|| {
        count.set(count.get() + 1);
        false
    });
    assert!(matches!(
        baseline,
        Err(SearchPortfolioError::InsufficientAnchorGeometry(_))
    ));
    drop(baseline);
    let last = count.get();
    for path in [&r[0].path, &r[1].path, &w] {
        let original = std::fs::read(path)?;
        let n = Cell::new(0);
        let result = run(&|| {
            n.set(n.get() + 1);
            if n.get() == last {
                let mut bytes = original.clone();
                bytes.push(0);
                std::fs::write(path, bytes).unwrap();
            }
            false
        });
        assert!(
            matches!(result, Err(SearchPortfolioError::Source(_))),
            "{result:?}"
        );
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original)?;
    }
    let n = Cell::new(0);
    let result = run(&|| {
        n.set(n.get() + 1);
        n.get() == last
    });
    assert!(matches!(result, Err(SearchPortfolioError::Cancelled)));
    assert_eq!(budget.used(), 0);
    Ok(())
}
#[test]
fn provider_failure_after_an_inconclusive_tile_remains_terminal() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = combined(SearchOrder::ModelFirst);
    native_miss(&mut p);
    p.model.anchors.local.minimum_witnesses = 11;
    p = tiled(p);
    let calls = Cell::new(0);
    let result = search_portfolio_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        &factors(),
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                model_fixture::proposals(b)
            } else {
                Err("deliberate provider failure".into())
            }
        },
        || false,
    );
    assert!(matches!(
        result,
        Err(SearchPortfolioError::ModelTiles(
            rrrah_dedup::model_tiles::ModelTileError::Comparison(
                rrrah_dedup::model_compare::ModelComparisonError::Provider(_)
            )
        ))
    ));
    assert_eq!(calls.get(), 2);
    assert_eq!(budget.used(), 0);
    Ok(())
}

fn empty_model_proposals(
    budget: &MemoryBudget,
) -> Result<
    rrrah_core::SharedBuffer<rrrah_dedup::proposals::ScoredProposal>,
    rrrah_dedup::model_compare::ProposalProviderError,
> {
    Ok(budget
        .try_buffer(
            0,
            rrrah_dedup::proposals::ScoredProposal {
                source: [0.; 2],
                target: [0.; 2],
                confidence: 0.,
            },
        )?
        .freeze())
}

#[test]
fn exhausted_native_anchor_geometry_advances_to_independently_verified_model_support() -> TestResult {
    let dir = tempfile::tempdir()?;
    let weights = dir.path().join("weights");
    std::fs::write(&weights, b"fixture provider")?;
    let source =
        DecodeRequest::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    for use_tiles in [false, true] {
        let mut p = combined(SearchOrder::NativeFirst);
        p.native.search.anchors.local.minimum_witnesses = 1000;
        p.model.decode.max_pixels = 200000;
        p.model.input.max_source_pixels = 200000;
        p.model.input.max_pixel_reads = 800000;
        if use_tiles {
            p = tiled(p);
        } else {
            p.work = p.required_work()?;
        }
        let calls = Cell::new(0);
        let evidence = search_portfolio_files_with(
            &source,
            &source,
            &weights,
            p,
            &factors(),
            &budget,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                model_fixture::proposals(b)
            },
            || false,
        )?;
        assert!(evidence.supported());
        assert_eq!(evidence.attempted_stages, 2);
        assert_eq!(calls.get(), 1);
        assert!(matches!(
            evidence.first_miss,
            Some(SearchStageMiss::InsufficientAnchorGeometry(
                InsufficientAnchorStage::Native
            ))
        ));
        assert!(matches!(
            (&evidence.terminal, use_tiles),
            (SearchStageEvidence::Model(_), false) | (SearchStageEvidence::ModelTiles(_), true)
        ));
        drop(evidence);
        assert_eq!(budget.used(), 0);
    }
    Ok(())
}

fn recovery_policy() -> rrrah_dedup::search_portfolio::SearchRecoveryPolicy {
    use rrrah_dedup::{anchor_rank_file::OppositeAnchorSearchPolicy, search_portfolio::SearchRecoveryPolicy};
    let primary = combined(SearchOrder::NativeFirst);
    let mut p = SearchRecoveryPolicy {
        primary,
        opposite: OppositeAnchorSearchPolicy {
            search: primary.native,
            maximum_swapped_points: primary.native.search.search.search.max_union_points,
        },
        work: primary.work,
    };
    p.work = p.required_work().expect("fixture maxima");
    p
}

#[test]
fn recovery_preflight_admits_all_work_and_storage_before_io() -> TestResult {
    use rrrah_dedup::search_portfolio::search_recovery_files_with;
    let missing = DecodeRequest::new("/nonexistent/recovery.png");
    let weights = std::path::Path::new("/nonexistent/recovery.weights");
    for field in 0..8 {
        let mut p = recovery_policy();
        match field {
            0 => p.work.point_checks -= 1,
            1 => p.work.selection_checks -= 1,
            2 => p.work.rank_sites -= 1,
            3 => p.work.pixel_reads -= 1,
            4 => p.work.geometry_hypotheses -= 1,
            5 => p.work.model_input_reads -= 1,
            6 => p.opposite.maximum_swapped_points = 0,
            _ => p.opposite.maximum_swapped_points = usize::MAX,
        }
        let budget = MemoryBudget::new(128 * 1024 * 1024);
        let result = search_recovery_files_with(
            &missing,
            &missing,
            weights,
            p,
            &factors(),
            &budget,
            |_, _, _, _, _| panic!("preflight provider"),
            || false,
        );
        if field < 6 {
            assert!(matches!(result, Err(SearchPortfolioError::Work)));
        } else {
            assert!(matches!(result, Err(SearchPortfolioError::Native(_))));
        }
        assert_eq!(budget.used(), 0);
    }
    let budget = MemoryBudget::new(0);
    assert!(matches!(
        search_recovery_files_with(
            &missing,
            &missing,
            weights,
            recovery_policy(),
            &factors(),
            &budget,
            |_, _, _, _, _| panic!("provider"),
            || false
        ),
        Err(SearchPortfolioError::Native(_))
    ));
    assert_eq!(budget.used(), 0);
    Ok(())
}

#[test]
fn recovery_primary_support_is_lazy_and_releases_reserved_storage() -> TestResult {
    use rrrah_dedup::search_portfolio::{SearchRecoveryOutcome, search_recovery_files_with};
    let dir = tempfile::tempdir()?;
    let weights = dir.path().join("weights");
    std::fs::write(&weights, b"fixture provider")?;
    let source = DecodeRequest::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"),
    );
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let e = search_recovery_files_with(
        &source,
        &source,
        &weights,
        recovery_policy(),
        &factors(),
        &budget,
        |_, _, _, _, _| panic!("native support must avoid model"),
        || false,
    )?;
    assert_eq!(e.supported(), Some(true));
    assert!(matches!(e, SearchRecoveryOutcome::Primary(_)));
    drop(e);
    assert_eq!(budget.used(), 0);
    Ok(())
}

#[test]
fn recovery_negative_cannot_erase_primary_unknown_and_publication_is_guarded() -> TestResult {
    use rrrah_dedup::search_portfolio::{SearchRecoveryOutcome, search_recovery_files_with};
    let dir = tempfile::tempdir()?;
    let (requests, weights) = model_fixture::fixture(dir.path());
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = recovery_policy();
    native_miss(&mut p.primary);
    p.primary.model.anchors.local.minimum_witnesses = 11;
    p.primary.work = p.primary.required_work()?;
    p.opposite.search = p.primary.native;
    p.work = p.required_work()?;
    let checkpoints = Cell::new(0);
    let outcome = search_recovery_files_with(
        &requests[0],
        &requests[1],
        &weights,
        p,
        &factors(),
        &budget,
        |_, _, _, b, _| model_fixture::proposals(b),
        || {
            checkpoints.set(checkpoints.get() + 1);
            false
        },
    )?;
    assert_eq!(outcome.supported(), None);
    assert!(matches!(
        outcome,
        SearchRecoveryOutcome::Inconclusive {
            primary_insufficient: Some(_),
            opposite_insufficient: false
        }
    ));
    drop(outcome);
    assert_eq!(budget.used(), 0);
    let last = checkpoints.get();
    checkpoints.set(0);
    let result = search_recovery_files_with(
        &requests[0],
        &requests[1],
        &weights,
        p,
        &factors(),
        &budget,
        |_, _, _, b, _| model_fixture::proposals(b),
        || {
            checkpoints.set(checkpoints.get() + 1);
            if checkpoints.get() == last {
                std::fs::write(&weights, b"changed weights").unwrap();
            }
            false
        },
    );
    assert!(
        matches!(result, Err(SearchPortfolioError::Source(_))),
        "{result:?}"
    );
    assert_eq!(budget.used(), 0);
    Ok(())
}

#[test]
fn recovery_runs_opposite_after_primary_miss_and_cancel_retry_releases() -> TestResult {
    use rrrah_dedup::search_portfolio::{SearchRecoveryOutcome, search_recovery_files_with};
    let dir = tempfile::tempdir()?;
    let weights = dir.path().join("weights");
    std::fs::write(&weights, b"fixture provider")?;
    let source = DecodeRequest::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"),
    );
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = recovery_policy();
    native_miss(&mut p.primary);
    p.primary.model.decode.max_pixels = 200000;
    p.primary.model.input.max_source_pixels = 200000;
    p.primary.model.input.max_pixel_reads = 800000;
    p.primary.work = p.primary.required_work()?;
    p.work = p.required_work()?;
    let calls = Cell::new(0);
    let checkpoints = Cell::new(0);
    let run = |stop_at| {
        search_recovery_files_with(
            &source,
            &source,
            &weights,
            p,
            &factors(),
            &budget,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                empty_model_proposals(b)
            },
            || {
                checkpoints.set(checkpoints.get() + 1);
                checkpoints.get() == stop_at
            },
        )
    };
    let outcome = run(usize::MAX)?;
    assert_eq!(outcome.supported(), Some(true));
    assert!(matches!(outcome, SearchRecoveryOutcome::Opposite { .. }));
    assert_eq!(calls.get(), 1);
    drop(outcome);
    assert_eq!(budget.used(), 0);
    let last = checkpoints.get();
    for checkpoint in [1, last / 2, last] {
        checkpoints.set(0);
        assert!(matches!(run(checkpoint), Err(SearchPortfolioError::Cancelled)));
        assert_eq!(budget.used(), 0);
    }
    checkpoints.set(0);
    drop(run(usize::MAX)?);
    assert_eq!(budget.used(), 0);
    Ok(())
}

#[test]
fn joint_recovery_preflight_and_real_fallback_cancel_retry() -> TestResult {
    use rrrah_dedup::search_portfolio::{SearchRecoveryOutcome, search_joint_recovery_files_with};
    let mut p = recovery_policy();
    native_miss(&mut p.primary);
    p.primary.model.decode.max_pixels = 200000;
    p.primary.model.input.max_source_pixels = 200000;
    p.primary.model.input.max_pixel_reads = 800000;
    p.primary.work = p.primary.required_work()?;
    p.work = p.required_work()?;
    let g = p.opposite.search.search.search.search.search.gradient.search;
    let projections = 12 * g.local.geometry.max_points as u64 * (g.sampling.trials + 1);
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let missing = DecodeRequest::new("missing-joint-recovery");
    assert!(matches!(
        search_joint_recovery_files_with(
            &missing,
            &missing,
            Path::new("missing-weights"),
            p,
            &factors(),
            &budget,
            projections - 1,
            |_, _, _, _, _| panic!("preflight"),
            || false
        ),
        Err(SearchPortfolioError::Native(_))
    ));
    let dir = tempfile::tempdir()?;
    let weights = dir.path().join("weights");
    std::fs::write(&weights, b"fixture")?;
    let source =
        DecodeRequest::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));
    let count = Cell::new(0);
    let run = |stop| {
        search_joint_recovery_files_with(
            &source,
            &source,
            &weights,
            p,
            &factors(),
            &budget,
            projections,
            |_, _, _, b, _| empty_model_proposals(b),
            || {
                count.set(count.get() + 1);
                count.get() == stop
            },
        )
    };
    let evidence = run(usize::MAX)?;
    assert!(matches!(evidence, SearchRecoveryOutcome::Opposite { .. }));
    assert_eq!(evidence.supported(), Some(true));
    drop(evidence);
    assert_eq!(budget.used(), 0);
    let last = count.get();
    count.set(0);
    assert!(run(last).is_err());
    assert_eq!(budget.used(), 0);
    count.set(0);
    drop(run(usize::MAX)?);
    assert_eq!(budget.used(), 0);
    Ok(())
}

#[test]
fn joint_model_automatic_tiles_recover_after_full_misses_in_both_orders_and_exhaust_before_native_fallback()
-> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let budget = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        let mut p = tiled(combined(order));
        native_miss(&mut p);
        let calls = Cell::new(0);
        let evidence = rrrah_dedup::search_portfolio::search_joint_model_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            2 * (p.model.geometry.max_points as u64) * (p.model.sampling.trials + 1),
            25 * 2 * (p.model.geometry.max_points as u64) * (p.model.sampling.trials + 1),
            &[1.],
            &budget,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                let count = if calls.get() == 11 { 10 } else { 0 };
                let mut points = b.try_buffer(
                    count,
                    rrrah_dedup::proposals::ScoredProposal {
                        source: [0.; 2],
                        target: [0.; 2],
                        confidence: 1.,
                    },
                )?;
                for (i, point) in points.iter_mut().enumerate() {
                    point.source = [4. + (i % 5) as f64 * 4., 12. + (i / 5) as f64 * 8.];
                    point.target = [point.source[0] + 32., point.source[1]];
                }
                Ok(points.freeze())
            },
            || false,
        )?;
        assert!(evidence.supported());
        assert_eq!(calls.get(), 11);
        assert_eq!(
            evidence.attempted_stages,
            if order == SearchOrder::ModelFirst { 1 } else { 2 }
        );
        let SearchStageEvidence::ModelTiles(tiles) = &evidence.terminal else {
            panic!("expected tiled model evidence")
        };
        assert_eq!(tiles.attempts().len(), 11);
        assert_eq!(tiles.terminal().left_rectangle, [32, 0, 32, 32]);
        let retained = tiles.clone();
        drop(evidence);
        assert!(budget.used() > 0);
        drop(retained);
        assert_eq!(budget.used(), 0);
    }
    let mut p = tiled(combined(SearchOrder::ModelFirst));
    native_miss(&mut p);
    let calls = Cell::new(0);
    let evidence = rrrah_dedup::search_portfolio::search_joint_model_portfolio_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        2 * (p.model.geometry.max_points as u64) * (p.model.sampling.trials + 1),
        25 * 2 * (p.model.geometry.max_points as u64) * (p.model.sampling.trials + 1),
        &[1.],
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            Ok(b.try_buffer(
                0,
                rrrah_dedup::proposals::ScoredProposal {
                    source: [0.; 2],
                    target: [0.; 2],
                    confidence: 0.,
                },
            )?
            .freeze())
        },
        || false,
    )?;
    assert_eq!(calls.get(), 25);
    assert!(!evidence.supported());
    assert!(matches!(
        evidence.first_miss,
        Some(SearchStageMiss::ModelTiles { attempts: 25, .. })
    ));
    assert!(matches!(evidence.terminal, SearchStageEvidence::Native(_)));
    drop(evidence);
    assert_eq!(budget.used(), 0);
    Ok(())
}

#[test]
fn joint_model_portfolio_final_user_checkpoint_cannot_publish_mutated_sources_or_weights() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let budget = rrrah_core::MemoryBudget::new(8 * 1024 * 1024);
    let p = combined(SearchOrder::ModelFirst);
    let calls = Cell::new(0);
    drop(
        rrrah_dedup::search_portfolio::search_joint_model_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            2 * (p.model.geometry.max_points as u64) * (p.model.sampling.trials + 1),
            25 * 2 * (p.model.geometry.max_points as u64) * (p.model.sampling.trials + 1),
            &[1.],
            &budget,
            |_, _, _, b, _| model_fixture::proposals(b),
            || {
                calls.set(calls.get() + 1);
                false
            },
        )?,
    );
    let last = calls.get();
    for path in [&r[0].path, &r[1].path, &w] {
        let original = std::fs::read(path)?;
        let current = Cell::new(0);
        let e = rrrah_dedup::search_portfolio::search_joint_model_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            2 * (p.model.geometry.max_points as u64) * (p.model.sampling.trials + 1),
            25 * 2 * (p.model.geometry.max_points as u64) * (p.model.sampling.trials + 1),
            &[1.],
            &budget,
            |_, _, _, b, _| model_fixture::proposals(b),
            || {
                current.set(current.get() + 1);
                if current.get() == last {
                    let mut changed = original.clone();
                    changed.push(0);
                    std::fs::write(path, changed).unwrap();
                }
                false
            },
        );
        assert!(matches!(e, Err(SearchPortfolioError::Source(_))), "{e:?}");
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original)?;
    }
    Ok(())
}
#[test]
fn joint_model_cross_stage_source_or_weight_changes_and_provider_errors_never_become_misses() -> TestResult {
    for change in 0..3 {
        let dir = tempfile::tempdir()?;
        let (requests, weights) = model_fixture::fixture(dir.path());
        let budget = MemoryBudget::new(128 * 1024 * 1024);
        let mut p = combined(SearchOrder::ModelFirst);
        native_miss(&mut p);
        let provider_done = Cell::new(false);
        let changed = Cell::new(false);
        let e = rrrah_dedup::search_portfolio::search_joint_model_portfolio_files_with(
            &requests[0],
            &requests[1],
            &weights,
            p,
            2 * (p.model.geometry.max_points as u64) * (p.model.sampling.trials + 1),
            25 * 2 * (p.model.geometry.max_points as u64) * (p.model.sampling.trials + 1),
            &factors(),
            &budget,
            |_, _, _, b, _| {
                provider_done.set(true);
                Ok(b.try_buffer(
                    0,
                    rrrah_dedup::proposals::ScoredProposal {
                        source: [0.; 2],
                        target: [0.; 2],
                        confidence: 0.,
                    },
                )?
                .freeze())
            },
            || {
                if provider_done.get() && budget.used() == 0 && !changed.replace(true) {
                    let path = if change < 2 {
                        &requests[change].path
                    } else {
                        &weights
                    };
                    std::fs::write(path, b"changed between model return and native fallback")
                        .expect("fixture mutation");
                }
                false
            },
        );
        assert!(changed.get());
        assert!(matches!(e, Err(SearchPortfolioError::Source(_))), "{e:?}");
        assert_eq!(budget.used(), 0);
    }
    let dir = tempfile::tempdir()?;
    let (requests, weights) = model_fixture::fixture(dir.path());
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let e = rrrah_dedup::search_portfolio::search_joint_model_portfolio_files_with(
        &requests[0],
        &requests[1],
        &weights,
        combined(SearchOrder::ModelFirst),
        u64::MAX / 25,
        u64::MAX,
        &factors(),
        &budget,
        |_, _, _, _, _| Err("provider refused".into()),
        || false,
    );
    assert!(matches!(
        e,
        Err(SearchPortfolioError::Model(
            rrrah_dedup::model_compare::ModelComparisonError::Provider(_)
        ))
    ));
    assert_eq!(budget.used(), 0);
    Ok(())
}

#[test]
fn joint_model_projection_preflight_precedes_even_native_first_io() -> TestResult {
    let request = rrrah_decode::DecodeRequest::new("missing");
    let budget = rrrah_core::MemoryBudget::new(0);
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        let p = tiled(combined(order));
        let per = 2 * (p.model.geometry.max_points as u64) * (p.model.sampling.trials + 1);
        for (cap, total) in [(per - 1, u64::MAX), (per, per * 25 - 1), (u64::MAX, u64::MAX)] {
            for stop in [false, true] {
                let result = rrrah_dedup::search_portfolio::search_joint_model_portfolio_files_with(
                    &request,
                    &request,
                    Path::new("missing-weights"),
                    p,
                    cap,
                    total,
                    &[1.],
                    &budget,
                    |_, _, _, _, _| panic!("no inference before preflight"),
                    || stop,
                );
                if stop {
                    assert!(matches!(result, Err(SearchPortfolioError::Cancelled)));
                } else {
                    assert!(matches!(
                        result,
                        Err(SearchPortfolioError::Model(
                            rrrah_dedup::model_compare::ModelComparisonError::Geometry(
                                rrrah_dedup::geometry::GeometryError::Budget
                            )
                        ))
                    ));
                }
                assert_eq!(budget.used(), 0);
            }
        }
    }
    Ok(())
}

fn bank_setup(
    mut p: SearchPortfolioPolicy,
) -> (
    SearchPortfolioPolicy,
    rrrah_dedup::joint_witnesses::JointBankPolicy,
    rrrah_dedup::model_tiles::ModelTileWorkLimits,
) {
    use rrrah_dedup::{
        geometry::JointProjectivePolicy,
        joint_witnesses::{JointBankPolicy, JointWitnessPolicy},
        model_tiles::ModelTileWorkLimits,
    };
    p.model.anchors.local.maximum_points = 400;
    p.model.anchors.local.maximum_point_checks = 100000;
    p.model.anchors.maximum_selection_checks = 250000;
    let m = p.model;
    let n = 25 * m.proposals.max_proposals as u64;
    let fit = 2 * m.geometry.max_points as u64 * (m.sampling.trials + 1);
    let sep = n * (n - 1) / 2;
    let mut target = m.geometry;
    target.min_inliers = 4;
    let banks = JointBankPolicy {
        hypothesis: JointProjectivePolicy {
            target,
            source_tolerance: m.anchors.local.source_tolerance,
            maximum_projection_checks: fit,
        },
        sampling: m.sampling,
        witnesses: JointWitnessPolicy {
            source_tolerance: m.anchors.local.source_tolerance,
            target_tolerance: m.geometry.tolerance,
            minimum_separation: 2.,
            maximum_points: n as usize,
            maximum_projection_checks: 2 * n,
            maximum_separation_checks: sep,
        },
        minimum_witnesses: 10,
        maximum_banks: 25,
        maximum_total_projection_checks: 25 * (fit + 2 * n),
        maximum_total_separation_checks: 25 * sep,
    };
    let mut work = ModelTileWorkLimits::for_model(m).unwrap();
    work.model_input_reads = 52 * m.input.max_pixel_reads;
    work.point_checks *= 2;
    work.selection_checks *= 2;
    work.rank_sites *= 2;
    work.pixel_reads *= 2;
    p.work = required_model_bank_portfolio_work(p, work).unwrap();
    (p, banks, work)
}
#[test]
fn bank_portfolio_confirms_in_both_orders_after_native_miss() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let b = MemoryBudget::new(128 * 1024 * 1024);
    for order in [SearchOrder::ModelFirst, SearchOrder::NativeFirst] {
        let mut p = combined(order);
        native_miss(&mut p);
        let (p, banks, work) = bank_setup(p);
        let calls = Cell::new(0);
        let e = search_model_bank_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            banks,
            work,
            &[12, 1],
            &factors(),
            &b,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                model_fixture::proposals(b)
            },
            || false,
        )?;
        assert!(e.supported());
        assert_eq!(calls.get(), 25);
        assert_eq!(
            e.attempted_stages,
            if order == SearchOrder::ModelFirst { 1 } else { 2 }
        );
        assert!(matches!(e.terminal, SearchStageEvidence::Model(_)));
        drop(e);
        assert_eq!(b.used(), 0);
    }
    Ok(())
}
#[test]
fn bank_portfolio_empty_learned_and_native_miss_remain_typed_unknown() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let b = MemoryBudget::new(128 * 1024 * 1024);
    for order in [SearchOrder::ModelFirst, SearchOrder::NativeFirst] {
        let mut p = combined(order);
        native_miss(&mut p);
        let (p, banks, work) = bank_setup(p);
        let e = search_model_bank_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            banks,
            work,
            &[2, 6],
            &factors(),
            &b,
            empty_provider,
            || false,
        );
        assert!(matches!(
            e,
            Err(SearchPortfolioError::InsufficientAnchorGeometry(_))
        ));
        assert_eq!(b.used(), 0);
    }
    Ok(())
}
#[test]
fn bank_portfolio_all_work_precedes_native_first_io_and_cancel_wins() {
    let (p, banks, work) = bank_setup(combined(SearchOrder::NativeFirst));
    let r = DecodeRequest::new("missing");
    let b = MemoryBudget::new(1024);
    let mut low = p;
    low.work.pixel_reads -= 1;
    let e = search_model_bank_portfolio_files_with(
        &r,
        &r,
        Path::new("missing"),
        low,
        banks,
        work,
        &[2, 6],
        &factors(),
        &b,
        |_, _, _, _, _| panic!("preflight"),
        || false,
    );
    assert!(matches!(e, Err(SearchPortfolioError::Work)));
    assert_eq!(b.used(), 0);
    let e = search_model_bank_portfolio_files_with(
        &r,
        &r,
        Path::new("missing"),
        p,
        banks,
        work,
        &[],
        &factors(),
        &b,
        |_, _, _, _, _| panic!("cancel"),
        || true,
    );
    assert!(matches!(e, Err(SearchPortfolioError::Cancelled)));
    assert_eq!(b.used(), 0);
}
#[test]
fn bank_portfolio_last_provider_weight_change_discards_all_evidence() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let (p, banks, work) = bank_setup(combined(SearchOrder::ModelFirst));
    let b = MemoryBudget::new(128 * 1024 * 1024);
    let calls = Cell::new(0);
    let e = search_model_bank_portfolio_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        banks,
        work,
        &[2, 6],
        &factors(),
        &b,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            let points = model_fixture::proposals(b)?;
            if calls.get() == 25 {
                std::fs::write(&w, b"changed")?;
            }
            Ok(points)
        },
        || false,
    );
    assert!(e.is_err());
    assert_eq!(calls.get(), 25);
    assert_eq!(b.used(), 0);
    Ok(())
}
#[test]
fn bank_unknown_advances_to_native_support_and_native_first_skips_model() -> TestResult {
    let dir = tempfile::tempdir()?;
    let w = dir.path().join("weights");
    std::fs::write(&w, b"fixture")?;
    let source =
        DecodeRequest::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));
    let b = MemoryBudget::new(128 * 1024 * 1024);
    for order in [SearchOrder::ModelFirst, SearchOrder::NativeFirst] {
        let mut p = combined(order);
        p.model.decode.max_pixels = 200000;
        p.model.input.max_source_pixels = 200000;
        p.model.input.max_pixel_reads = 800000;
        let (p, banks, work) = bank_setup(p);
        let calls = Cell::new(0);
        let e = search_model_bank_portfolio_files_with(
            &source,
            &source,
            &w,
            p,
            banks,
            work,
            &[2, 6],
            &factors(),
            &b,
            |a, l, r, b, c| {
                calls.set(calls.get() + 1);
                empty_provider(a, l, r, b, c)
            },
            || false,
        )?;
        assert!(e.supported());
        assert!(matches!(e.terminal, SearchStageEvidence::Native(_)));
        assert_eq!(calls.get(), if order == SearchOrder::ModelFirst { 25 } else { 0 });
        drop(e);
        assert_eq!(b.used(), 0);
    }
    Ok(())
}

fn bank_recovery_setup(
    mut p: SearchRecoveryPolicy,
) -> (
    SearchRecoveryPolicy,
    rrrah_dedup::joint_witnesses::JointBankPolicy,
    rrrah_dedup::model_tiles::ModelTileWorkLimits,
    u64,
) {
    let (primary, banks, work) = bank_setup(p.primary);
    p.primary = primary;
    p.work = required_model_bank_recovery_work(p, work).unwrap();
    let g = p.opposite.search.search.search.search.search.gradient.search;
    let projections = 12 * g.local.geometry.max_points as u64 * (g.sampling.trials + 1);
    (p, banks, work, projections)
}
#[test]
fn bank_joint_recovery_complete_admission_and_cancel_precede_io() {
    let (p, banks, work, projections) = bank_recovery_setup(recovery_policy());
    let r = DecodeRequest::new("missing");
    let b = MemoryBudget::new(128 * 1024 * 1024);
    for case in 0..5 {
        let mut p = p;
        let mut cap = projections;
        let mut radii = &[2, 6][..];
        match case {
            0 => p.work.pixel_reads -= 1,
            1 => p.primary.work.model_input_reads -= 1,
            2 => cap -= 1,
            3 => radii = &[],
            _ => p.opposite.maximum_swapped_points = 0,
        }
        for stop in [false, true] {
            let e = search_model_bank_joint_recovery_files_with(
                &r,
                &r,
                Path::new("missing"),
                p,
                banks,
                work,
                radii,
                cap,
                &factors(),
                &b,
                |_, _, _, _, _| panic!("preflight before provider"),
                || stop,
            );
            assert!(e.is_err());
            if stop {
                assert!(matches!(e, Err(SearchPortfolioError::Cancelled)));
            }
            assert_eq!(b.used(), 0);
        }
    }
}
#[test]
fn bank_joint_recovery_bank_support_stops_before_opposite_and_empty_stays_unknown() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let b = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = recovery_policy();
    p.primary.order = SearchOrder::ModelFirst;
    native_miss(&mut p.primary);
    let (p, banks, work, projections) = bank_recovery_setup(p);
    let calls = Cell::new(0);
    let e = search_model_bank_joint_recovery_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        banks,
        work,
        &[12, 1],
        projections,
        &factors(),
        &b,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            model_fixture::proposals(b)
        },
        || false,
    )?;
    assert!(matches!(e, SearchRecoveryOutcome::Primary(_)));
    assert_eq!(e.supported(), Some(true));
    assert_eq!(calls.get(), 25);
    drop(e);
    assert_eq!(b.used(), 0);
    let mut p = p;
    p.opposite.search = p.primary.native;
    let e = search_model_bank_joint_recovery_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        banks,
        work,
        &[2, 6],
        projections,
        &factors(),
        &b,
        empty_provider,
        || false,
    )?;
    assert!(matches!(e, SearchRecoveryOutcome::Inconclusive { .. }));
    assert_eq!(e.supported(), None);
    drop(e);
    assert_eq!(b.used(), 0);
    Ok(())
}
#[test]
fn bank_joint_recovery_unknown_advances_to_joint_opposite_with_final_guards() -> TestResult {
    let dir = tempfile::tempdir()?;
    let w = dir.path().join("weights");
    std::fs::write(&w, b"fixture")?;
    let source =
        DecodeRequest::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));
    let b = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = recovery_policy();
    native_miss(&mut p.primary);
    p.primary.model.decode.max_pixels = 200000;
    p.primary.model.input.max_source_pixels = 200000;
    p.primary.model.input.max_pixel_reads = 800000;
    let (p, banks, work, projections) = bank_recovery_setup(p);
    for order in [SearchOrder::ModelFirst, SearchOrder::NativeFirst] {
        let mut p = p;
        p.primary.order = order;
        let checkpoints = Cell::new(0);
        let providers = Cell::new(0);
        let e = search_model_bank_joint_recovery_files_with(
            &source,
            &source,
            &w,
            p,
            banks,
            work,
            &[2, 6],
            projections,
            &factors(),
            &b,
            |a, l, r, b, c| {
                providers.set(providers.get() + 1);
                empty_provider(a, l, r, b, c)
            },
            || {
                checkpoints.set(checkpoints.get() + 1);
                false
            },
        )?;
        assert_eq!(providers.get(), 25);
        assert_eq!(e.supported(), Some(true));
        assert!(matches!(
            e,
            SearchRecoveryOutcome::Opposite {
                primary_insufficient: Some(_),
                ..
            }
        ));
        drop(e);
        assert_eq!(b.used(), 0);
        let last = checkpoints.get();
        let count = Cell::new(0);
        let bytes = std::fs::read(&w)?;
        let e = search_model_bank_joint_recovery_files_with(
            &source,
            &source,
            &w,
            p,
            banks,
            work,
            &[2, 6],
            projections,
            &factors(),
            &b,
            empty_provider,
            || {
                count.set(count.get() + 1);
                if count.get() == last {
                    std::fs::write(&w, b"changed at publication").unwrap();
                }
                false
            },
        );
        assert!(matches!(e, Err(SearchPortfolioError::Source(_))), "{e:?}");
        assert_eq!(b.used(), 0);
        std::fs::write(&w, bytes)?;
    }
    Ok(())
}
#[test]
fn bank_joint_recovery_late_provider_failure_cannot_enter_opposite() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let mut p = recovery_policy();
    p.primary.order = SearchOrder::ModelFirst;
    let (p, banks, work, projections) = bank_recovery_setup(p);
    let b = MemoryBudget::new(128 * 1024 * 1024);
    let calls = Cell::new(0);
    let e = search_model_bank_joint_recovery_files_with(
        &r[0],
        &r[1],
        &w,
        p,
        banks,
        work,
        &[2, 6],
        projections,
        &factors(),
        &b,
        |a, l, r, b, c| {
            calls.set(calls.get() + 1);
            if calls.get() == 25 {
                return Err(std::io::Error::other("provider fatal on last bank").into());
            }
            empty_provider(a, l, r, b, c)
        },
        || false,
    );
    assert!(e.is_err());
    assert_eq!(calls.get(), 25);
    assert_eq!(b.used(), 0);
    Ok(())
}

#[test]
fn native_family_first_support_does_not_call_provider_and_has_explicit_trace() -> TestResult {
    let dir = tempfile::tempdir()?;
    let w = dir.path().join("weights");
    std::fs::write(&w, b"fixture")?;
    let r =
        DecodeRequest::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));
    let b = MemoryBudget::new(128 * 1024 * 1024);
    for opposite in [false, true] {
        let mut p = recovery_policy();
        if opposite {
            native_miss(&mut p.primary);
        }
        p.primary.model.decode.max_pixels = 200000;
        p.primary.model.input.max_source_pixels = 200000;
        p.primary.model.input.max_pixel_reads = 800000;
        let (p, banks, work, cap) = bank_recovery_setup(p);
        let e = search_native_bank_recovery_files_with(
            &r,
            &r,
            &w,
            p,
            banks,
            work,
            &[2, 6],
            cap,
            &factors(),
            &b,
            |_, _, _, _, _| panic!("native positive must skip provider"),
            || false,
        )?;
        assert_eq!(e.supported(), Some(true));
        assert!(!e.model_attempted());
        assert_eq!(e.opposite_insufficient, opposite.then_some(false));
        assert_eq!(e.native_miss.is_some(), opposite);
        if opposite {
            assert!(matches!(e.terminal, NativeBankRecoveryTerminal::Opposite(_)));
        } else {
            assert!(matches!(e.terminal, NativeBankRecoveryTerminal::Native(_)));
        }
        drop(e);
        assert_eq!(b.used(), 0);
    }
    Ok(())
}
#[test]
fn native_family_first_model_support_or_unknown_follows_both_native_misses() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let b = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = recovery_policy();
    native_miss(&mut p.primary);
    p.opposite.search = p.primary.native;
    let (p, banks, work, cap) = bank_recovery_setup(p);
    for positive in [true, false] {
        let calls = Cell::new(0);
        let e = search_native_bank_recovery_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            banks,
            work,
            &[12, 1],
            cap,
            &factors(),
            &b,
            |a, l, r, b, c| {
                calls.set(calls.get() + 1);
                if positive {
                    model_fixture::proposals(b)
                } else {
                    empty_provider(a, l, r, b, c)
                }
            },
            || false,
        )?;
        assert_eq!(calls.get(), 25);
        assert!(e.model_attempted());
        assert!(e.native_miss.is_some());
        assert!(e.opposite_insufficient.is_some());
        assert_eq!(e.supported(), if positive { Some(true) } else { None });
        drop(e);
        assert_eq!(b.used(), 0);
    }
    Ok(())
}
#[test]
fn native_family_first_all_stage_work_precedes_io_and_cancel_wins() {
    let (p, banks, work, cap) = bank_recovery_setup(recovery_policy());
    let r = DecodeRequest::new("missing");
    let b = MemoryBudget::new(128 * 1024 * 1024);
    for case in 0..3 {
        let mut p = p;
        let mut radii = &[2, 6][..];
        let mut cap = cap;
        match case {
            0 => p.work.pixel_reads -= 1,
            1 => radii = &[],
            _ => cap -= 1,
        };
        for stop in [false, true] {
            let e = search_native_bank_recovery_files_with(
                &r,
                &r,
                Path::new("missing"),
                p,
                banks,
                work,
                radii,
                cap,
                &factors(),
                &b,
                |_, _, _, _, _| panic!("no provider before admission"),
                || stop,
            );
            assert!(e.is_err());
            assert!(!matches!(e, Err(SearchPortfolioError::Source(_))));
            if stop {
                assert!(matches!(e, Err(SearchPortfolioError::Cancelled)));
            }
            assert_eq!(b.used(), 0);
        }
    }
}
#[test]
fn native_family_first_publication_checks_weights_after_early_opposite_support() -> TestResult {
    let dir = tempfile::tempdir()?;
    let w = dir.path().join("weights");
    std::fs::write(&w, b"fixture")?;
    let r =
        DecodeRequest::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));
    let b = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = recovery_policy();
    native_miss(&mut p.primary);
    p.primary.model.decode.max_pixels = 200000;
    p.primary.model.input.max_source_pixels = 200000;
    p.primary.model.input.max_pixel_reads = 800000;
    let (p, banks, work, cap) = bank_recovery_setup(p);
    let count = Cell::new(0);
    drop(search_native_bank_recovery_files_with(
        &r,
        &r,
        &w,
        p,
        banks,
        work,
        &[2, 6],
        cap,
        &factors(),
        &b,
        |_, _, _, _, _| panic!("no model"),
        || {
            count.set(count.get() + 1);
            false
        },
    )?);
    let last = count.get();
    count.set(0);
    let e = search_native_bank_recovery_files_with(
        &r,
        &r,
        &w,
        p,
        banks,
        work,
        &[2, 6],
        cap,
        &factors(),
        &b,
        |_, _, _, _, _| panic!("no model"),
        || {
            count.set(count.get() + 1);
            if count.get() == last {
                std::fs::write(&w, b"changed at final checkpoint").unwrap();
            }
            false
        },
    );
    assert!(matches!(e, Err(SearchPortfolioError::Source(_))));
    assert_eq!(b.used(), 0);
    Ok(())
}

use rrrah_dedup::search_portfolio::{
    NativeBankDirectionPolicy, required_native_bank_consensus_work, search_native_bank_consensus_files_with,
};
#[test]
fn consensus_admits_reverse_and_total_work_before_io() {
    let (p, banks, model_work, cap) = bank_recovery_setup(recovery_policy());
    let factors = factors();
    let forward = NativeBankDirectionPolicy {
        search: p,
        banks,
        model_work,
        radii: &[2, 6],
        maximum_projection_checks: cap,
        factors: &factors,
    };
    let total = required_native_bank_consensus_work(forward, forward).unwrap();
    let r = DecodeRequest::new("missing");
    let b = MemoryBudget::new(128 * 1024 * 1024);
    for case in 0..5 {
        let mut reverse = forward;
        let mut work = total;
        let mut projections = 2 * cap;
        match case {
            0 => reverse.radii = &[],
            1 => work.pixel_reads -= 1,
            2 => projections -= 1,
            3 => reverse.search.work.point_checks -= 1,
            _ => reverse.maximum_projection_checks = u64::MAX,
        }
        for stop in [false, true] {
            let e = search_native_bank_consensus_files_with(
                &r,
                &r,
                Path::new("missing"),
                forward,
                reverse,
                work,
                projections,
                &b,
                |_, _, _, _, _| panic!("preflight"),
                || stop,
            );
            assert!(e.is_err());
            assert!(!matches!(e, Err(SearchPortfolioError::Source(_))));
            if stop {
                assert!(matches!(e, Err(SearchPortfolioError::Cancelled)));
            }
            assert_eq!(b.used(), 0);
        }
    }
}
#[test]
fn consensus_native_support_and_final_mutation_cancel_are_guarded() -> TestResult {
    let dir = tempfile::tempdir()?;
    let w = dir.path().join("weights");
    std::fs::write(&w, b"fixture")?;
    let r =
        DecodeRequest::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));
    let b = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = recovery_policy();
    p.primary.model.decode.max_pixels = 200000;
    p.primary.model.input.max_source_pixels = 200000;
    p.primary.model.input.max_pixel_reads = 800000;
    let (p, banks, model_work, cap) = bank_recovery_setup(p);
    let factors = factors();
    let direction = NativeBankDirectionPolicy {
        search: p,
        banks,
        model_work,
        radii: &[2, 6],
        maximum_projection_checks: cap,
        factors: &factors,
    };
    let work = required_native_bank_consensus_work(direction, direction)?;
    let run = |cancel: &dyn Fn() -> bool| {
        search_native_bank_consensus_files_with(
            &r,
            &r,
            &w,
            direction,
            direction,
            work,
            2 * cap,
            &b,
            |_, _, _, _, _| panic!("native supports"),
            cancel,
        )
    };
    let count = Cell::new(0);
    let e = run(&|| {
        count.set(count.get() + 1);
        false
    })?;
    assert_eq!(e.supported(), Some(true));
    assert!(!e.forward.model_attempted && !e.reverse.model_attempted);
    assert_eq!(b.used(), 0);
    let last = count.get();
    count.set(0);
    let e = run(&|| {
        count.set(count.get() + 1);
        if count.get() == last {
            std::fs::write(&w, b"changed").unwrap();
        }
        false
    });
    assert!(matches!(e, Err(SearchPortfolioError::Source(_))));
    assert_eq!(b.used(), 0);
    std::fs::write(&w, b"fixture")?;
    count.set(0);
    let e = run(&|| {
        count.set(count.get() + 1);
        count.get() == last
    });
    assert!(matches!(e, Err(SearchPortfolioError::Cancelled)));
    assert_eq!(b.used(), 0);
    Ok(())
}
#[test]
fn consensus_unknown_runs_reverse_and_provider_failure_stays_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let b = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = recovery_policy();
    native_miss(&mut p.primary);
    p.opposite.search = p.primary.native;
    let (p, banks, model_work, cap) = bank_recovery_setup(p);
    let factors = factors();
    let direction = NativeBankDirectionPolicy {
        search: p,
        banks,
        model_work,
        radii: &[2, 6],
        maximum_projection_checks: cap,
        factors: &factors,
    };
    let work = required_native_bank_consensus_work(direction, direction)?;
    let original_weights = std::fs::read(&w)?;
    for mode in 0..5 {
        let calls = Cell::new(0);
        let stop = Cell::new(false);
        let result = search_native_bank_consensus_files_with(
            &r[0],
            &r[1],
            &w,
            direction,
            direction,
            work,
            2 * cap,
            &b,
            |a, l, r, b, c| {
                calls.set(calls.get() + 1);
                if calls.get() == 26 {
                    if mode == 1 {
                        return Err(std::io::Error::other("reverse provider failed").into());
                    }
                    if mode == 3 {
                        std::fs::write(&w, b"changed during reverse").unwrap();
                    }
                    if mode == 4 {
                        stop.set(true);
                    }
                }
                if mode >= 2 && calls.get() <= 25 {
                    model_fixture::proposals(b)
                } else {
                    empty_provider(a, l, r, b, c)
                }
            },
            || stop.get(),
        );
        match mode {
            1 => {
                assert!(result.is_err());
                assert_eq!(calls.get(), 26);
            }
            3 => {
                assert!(
                    matches!(
                        result,
                        Err(SearchPortfolioError::ModelTiles(
                            rrrah_dedup::model_tiles::ModelTileError::Comparison(
                                rrrah_dedup::model_compare::ModelComparisonError::ModelSource(_)
                            )
                        ))
                    ),
                    "{result:?}"
                );
                std::fs::write(&w, &original_weights)?;
            }
            4 => assert!(matches!(result, Err(SearchPortfolioError::Cancelled))),
            _ => {
                let e = result?;
                assert_eq!(e.supported(), None);
                assert_eq!(e.forward.support, if mode == 2 { Some(true) } else { None });
                assert_eq!(e.reverse.support, None);
                assert_eq!(calls.get(), 50);
            }
        }
        assert_eq!(b.used(), 0);
    }
    Ok(())
}
