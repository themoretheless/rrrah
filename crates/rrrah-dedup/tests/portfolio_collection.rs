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
use std::{cell::Cell, error::Error};
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

use rrrah_dedup::portfolio_collection::*;
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

fn work(w: PortfolioWorkLimits, n: u64) -> PortfolioWorkLimits {
    PortfolioWorkLimits {
        point_checks: w.point_checks * n,
        selection_checks: w.selection_checks * n,
        rank_sites: w.rank_sites * n,
        pixel_reads: w.pixel_reads * n,
        geometry_hypotheses: w.geometry_hypotheses * n,
        model_input_reads: w.model_input_reads * n,
    }
}
fn collection() -> PortfolioCollectionPolicy {
    let search = combined(SearchOrder::ModelFirst);
    PortfolioCollectionPolicy {
        search,
        max_files: 3,
        max_pairs: 3,
        work: work(search.work, 3),
    }
}
fn three(
    dir: &std::path::Path,
) -> Result<([(u64, DecodeRequest); 3], std::path::PathBuf), Box<dyn Error + Send + Sync>> {
    let (r, w) = model_fixture::fixture(dir);
    let third = dir.join("third.bmp");
    std::fs::copy(&r[0].path, &third)?;
    Ok((
        [
            (u64::MAX, r[0].clone()),
            (0, r[1].clone()),
            (7, DecodeRequest::new(third)),
        ],
        w,
    ))
}
fn fake(
    _: &std::path::Path,
    _: &rrrah_dedup::model_input::GrayModelInput,
    _: &rrrah_dedup::model_input::GrayModelInput,
    b: &MemoryBudget,
    _: &dyn Fn() -> bool,
) -> Result<
    rrrah_core::SharedBuffer<rrrah_dedup::proposals::ScoredProposal>,
    rrrah_dedup::model_compare::ProposalProviderError,
> {
    model_fixture::proposals(b)
}

fn tiled_collection() -> PortfolioCollectionPolicy {
    let mut p = collection();
    p.search.model_tiles =
        Some(rrrah_dedup::model_tiles::ModelTileWorkLimits::for_model(p.search.model).unwrap());
    p.search.work = p.search.required_work().unwrap();
    p.work = work(p.search.work, 3);
    p
}
#[test]
fn collection_checks_every_pair_with_automatic_tiles_and_retained_attempt_credit() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (files, w) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let calls = Cell::new(0);
    let report = scan_portfolio_collection_with(
        files,
        &w,
        tiled_collection(),
        &factors(),
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            let count = if calls.get() % 11 == 0 { 10 } else { 0 };
            let mut points = b.try_buffer(
                count,
                rrrah_dedup::proposals::ScoredProposal {
                    source: [0.; 2],
                    target: [0.; 2],
                    confidence: 1.,
                },
            )?;
            for (i, p) in points.iter_mut().enumerate() {
                p.source = [4. + (i % 5) as f64 * 4., 12. + (i / 5) as f64 * 8.];
                p.target = [p.source[0] + 32., p.source[1]];
            }
            Ok(points.freeze())
        },
        || false,
    )?;
    assert_eq!(calls.get(), 33);
    assert_eq!(report.pairs().len(), 3);
    let mut retained = None;
    for pair in report.pairs() {
        let SearchStageEvidence::ModelTiles(e) = &pair.evidence.terminal else {
            panic!("expected tiled model result")
        };
        assert!(e.supported());
        assert_eq!(e.attempts().len(), 11);
        retained.get_or_insert_with(|| e.clone());
    }
    let before = budget.used();
    drop(report);
    assert!(budget.used() > 0 && budget.used() < before);
    drop(retained);
    assert_eq!(budget.used(), 0);
    Ok(())
}
#[test]
fn late_tiled_pair_failure_or_idle_owner_cancellation_discards_earlier_collection_results() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (files, w) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let calls = Cell::new(0);
    let report = scan_portfolio_collection_with(
        files.clone(),
        &w,
        tiled_collection(),
        &factors(),
        &budget,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                return Err(std::io::Error::other("later pair provider failure").into());
            }
            model_fixture::proposals(b)
        },
        || false,
    );
    assert!(matches!(report, Err(PortfolioCollectionError::Pair { .. })));
    assert_eq!(budget.used(), 0);
    for owner in 0..3 {
        let mut inputs = files.clone();
        let generation = Arc::new(AtomicU64::new(1));
        inputs[owner].1.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
        let calls = Cell::new(0);
        let report = scan_portfolio_collection_with(
            inputs,
            &w,
            tiled_collection(),
            &factors(),
            &budget,
            |_, _, _, b, cancel| {
                calls.set(calls.get() + 1);
                if calls.get() == 7 {
                    generation.store(2, Ordering::Release);
                    assert!(cancel());
                    generation.store(1, Ordering::Release);
                }
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
        );
        assert!(matches!(report, Err(PortfolioCollectionError::Cancelled)));
        assert_eq!(calls.get(), 7);
        assert_eq!(budget.used(), 0);
    }
    Ok(())
}
#[test]
fn every_pair_is_deterministic_matches_direct_evidence_and_credit_follows_report_owners() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (files, weights) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let calls = Cell::new(0);
    let report = scan_portfolio_collection_with(
        files.clone(),
        &weights,
        collection(),
        &factors(),
        &budget,
        |a, b, c, d, e| {
            calls.set(calls.get() + 1);
            fake(a, b, c, d, e)
        },
        || false,
    )?;
    assert_eq!(report.file_ids(), &[0, 7, u64::MAX]);
    assert_eq!(
        report
            .pairs()
            .iter()
            .map(|p| (p.left, p.right))
            .collect::<Vec<_>>(),
        vec![(0, 7), (0, u64::MAX), (7, u64::MAX)]
    );
    assert_eq!(calls.get(), 3);
    for pair in report.pairs() {
        assert!(pair.evidence.supported());
        let left = &files
            .iter()
            .find(|(id, _)| *id == pair.left)
            .ok_or("missing left")?
            .1;
        let right = &files
            .iter()
            .find(|(id, _)| *id == pair.right)
            .ok_or("missing right")?
            .1;
        let direct = search_portfolio_files_with(
            left,
            right,
            &weights,
            collection().search,
            &factors(),
            &budget,
            fake,
            || false,
        )?;
        let (SearchStageEvidence::Model(a), SearchStageEvidence::Model(b)) =
            (&pair.evidence.terminal, direct.terminal)
        else {
            return Err("expected model evidence".into());
        };
        assert_eq!(a.model_digest, b.model_digest);
        assert_eq!(a.raw_proposals, b.raw_proposals);
        assert_eq!(a.selected_points, b.selected_points);
        assert_eq!(a.transform.map(|t| t.matrix), b.transform.map(|t| t.matrix));
        assert_eq!(a.geometry_inliers, b.geometry_inliers);
        assert_eq!(a.hypotheses, b.hypotheses);
        assert_eq!(a.pixels, b.pixels);
    }
    assert!(budget.used() > 0);
    let report = Arc::new(report);
    let owner = report.clone();
    drop(report);
    assert!(budget.used() > 0);
    drop(owner);
    assert_eq!(budget.used(), 0);
    Ok(())
}
#[test]
fn a_native_empty_proposal_bank_does_not_hide_model_only_collection_support() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    let mut p = collection();
    p.search.order = SearchOrder::NativeFirst;
    native_miss(&mut p.search);
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let report = scan_portfolio_collection_with(
        [(0, r[0].clone()), (u64::MAX, r[1].clone())],
        &w,
        p,
        &factors(),
        &budget,
        fake,
        || false,
    )?;
    assert_eq!(report.pairs().len(), 1);
    let e = &report.pairs()[0].evidence;
    assert!(e.supported());
    assert_eq!(e.attempted_stages, 2);
    assert!(matches!(
        e.first_miss,
        Some(SearchStageMiss::Native {
            recipes: 6,
            candidates: 0
        })
    ));
    drop(report);
    assert_eq!(budget.used(), 0);
    Ok(())
}
struct Untouched;
impl IntoIterator for Untouched {
    type Item = (u64, DecodeRequest);
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        panic!("preflight consumed the iterator")
    }
}
#[test]
fn cumulative_overflow_policy_and_memory_preflight_precede_iterator_and_io() -> TestResult {
    let missing = std::path::Path::new("/nonexistent/collection.weights");
    for case in 0..9 {
        let mut p = collection();
        let budget = MemoryBudget::new(if case == 8 { 0 } else { 128 * 1024 * 1024 });
        match case {
            0 => p.work.point_checks -= 1,
            1 => p.work.selection_checks -= 1,
            2 => p.work.rank_sites -= 1,
            3 => p.work.pixel_reads -= 1,
            4 => p.work.geometry_hypotheses -= 1,
            5 => p.work.model_input_reads -= 1,
            6 => p.max_pairs = usize::MAX,
            7 => p.search.model.sampling.seed = 0,
            _ => {}
        }
        let result =
            scan_portfolio_collection_with(Untouched, missing, p, &factors(), &budget, fake, || false);
        assert!(
            matches!(
                result,
                Err(PortfolioCollectionError::Work
                    | PortfolioCollectionError::Policy(_)
                    | PortfolioCollectionError::Memory(_))
            ),
            "{result:?}"
        );
        assert_eq!(budget.used(), 0);
        assert_eq!(budget.peak(), 0);
        assert!(matches!(
            scan_portfolio_collection_with(Untouched, missing, p, &factors(), &budget, fake, || true),
            Err(PortfolioCollectionError::Cancelled)
        ));
    }
    Ok(())
}
#[test]
fn duplicate_file_and_complete_pair_limits_refuse_before_file_io() -> TestResult {
    let weights = std::path::Path::new("/nonexistent/collection.weights");
    for case in 0..3 {
        let mut p = collection();
        let ids = if case == 0 {
            vec![7, 7, 0]
        } else if case == 1 {
            vec![0, 1, 2, 3]
        } else {
            vec![0, 1, 2]
        };
        if case == 2 {
            p.max_pairs = 2;
            p.work = work(p.search.work, 2);
        }
        let budget = MemoryBudget::new(128 * 1024 * 1024);
        let result = scan_portfolio_collection_with(
            ids.into_iter()
                .map(|id| (id, DecodeRequest::new("/nonexistent/input.bmp"))),
            weights,
            p,
            &factors(),
            &budget,
            fake,
            || false,
        );
        assert!(matches!(
            (case, result),
            (0, Err(PortfolioCollectionError::DuplicateId(7)))
                | (1, Err(PortfolioCollectionError::Files))
                | (2, Err(PortfolioCollectionError::Pairs))
        ));
        assert_eq!(budget.used(), 0);
    }
    Ok(())
}
#[test]
fn idle_owner_cancellation_and_late_provider_failure_discard_earlier_decisions() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (files, weights) = three(dir.path())?;
    for owner in 0..3 {
        let mut files = files.clone();
        let generation = Arc::new(AtomicU64::new(1));
        files[owner].1.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
        let budget = MemoryBudget::new(128 * 1024 * 1024);
        let result = scan_portfolio_collection_with(
            files,
            &weights,
            collection(),
            &factors(),
            &budget,
            |a, b, c, d, e| {
                generation.store(2, Ordering::Release);
                fake(a, b, c, d, e)
            },
            || false,
        );
        assert!(
            matches!(result, Err(PortfolioCollectionError::Cancelled)),
            "{result:?}"
        );
        assert_eq!(budget.used(), 0);
    }
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let calls = Cell::new(0);
    let result = scan_portfolio_collection_with(
        files,
        &weights,
        collection(),
        &factors(),
        &budget,
        |a, b, c, d, e| {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                Err("late provider refusal".into())
            } else {
                fake(a, b, c, d, e)
            }
        },
        || false,
    );
    assert!(matches!(result, Err(PortfolioCollectionError::Pair { .. })));
    assert_eq!(calls.get(), 2);
    assert_eq!(budget.used(), 0);
    Ok(())
}
#[test]
fn final_cancellation_or_final_callback_source_mutation_never_publishes_a_stale_batch() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (files, weights) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let calls = Cell::new(0);
    let report = scan_portfolio_collection_with(
        files.clone(),
        &weights,
        collection(),
        &factors(),
        &budget,
        fake,
        || {
            calls.set(calls.get() + 1);
            false
        },
    )?;
    assert_eq!(report.pairs().len(), 3);
    drop(report);
    let last = calls.get();
    let calls = Cell::new(0);
    let result = scan_portfolio_collection_with(
        files.clone(),
        &weights,
        collection(),
        &factors(),
        &budget,
        fake,
        || {
            calls.set(calls.get() + 1);
            calls.get() == last
        },
    );
    assert!(matches!(result, Err(PortfolioCollectionError::Cancelled)));
    assert_eq!(calls.get(), last);
    assert_eq!(budget.used(), 0);
    for owner in 0..3 {
        let mut owned = files.clone();
        let generation = Arc::new(AtomicU64::new(1));
        owned[owner].1.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
        let calls = Cell::new(0);
        let result =
            scan_portfolio_collection_with(owned, &weights, collection(), &factors(), &budget, fake, || {
                calls.set(calls.get() + 1);
                if calls.get() == last {
                    generation.store(2, Ordering::Release);
                }
                false
            });
        assert!(
            matches!(result, Err(PortfolioCollectionError::Cancelled)),
            "{result:?}"
        );
        assert_eq!(calls.get(), last);
        assert_eq!(budget.used(), 0);
    }
    for index in 0..4 {
        let path = if index < 3 { &files[index].1.path } else { &weights };
        let original = std::fs::read(path)?;
        let mut changed = original.clone();
        changed.push(0);
        let calls = Cell::new(0);
        let write_error = RefCell::new(None);
        let result = scan_portfolio_collection_with(
            files.clone(),
            &weights,
            collection(),
            &factors(),
            &budget,
            fake,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == last {
                    if let Err(error) = std::fs::write(path, &changed) {
                        *write_error.borrow_mut() = Some(error);
                    }
                }
                false
            },
        );
        if let Some(error) = write_error.into_inner() {
            return Err(error.into());
        }
        assert_eq!(calls.get(), last);
        assert!(
            matches!(
                result,
                Err(PortfolioCollectionError::Source { .. } | PortfolioCollectionError::ModelSource(_))
            ),
            "{result:?}"
        );
        assert_eq!(budget.used(), 0);
        std::fs::write(path, original)?;
    }
    Ok(())
}
#[test]
fn empty_and_single_file_batches_are_complete_without_invented_pairs() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (r, w) = model_fixture::fixture(dir.path());
    for n in 0..2 {
        let mut p = collection();
        p.max_files = n;
        p.max_pairs = 0;
        p.work = work(p.search.work, 0);
        let budget = MemoryBudget::new(1024 * 1024);
        let report = scan_portfolio_collection_with(
            (0..n).map(|i| (i as u64, r[i].clone())),
            &w,
            p,
            &factors(),
            &budget,
            |_, _, _, _, _| panic!("no pairs expected"),
            || false,
        )?;
        assert_eq!(report.file_ids().len(), n);
        assert!(report.pairs().is_empty());
        drop(report);
        assert_eq!(budget.used(), 0);
    }
    Ok(())
}

#[test]
fn native_evidence_and_report_slots_have_independent_last_owner_credit() -> TestResult {
    let dir = tempfile::tempdir()?;
    let weights = dir.path().join("weights");
    std::fs::write(&weights, b"native early support")?;
    let source = DecodeRequest::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"),
    );
    let mut policy = collection();
    policy.search.order = SearchOrder::NativeFirst;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let report = scan_portfolio_collection_with(
        [(7, source.clone()), (0, source)],
        &weights,
        policy,
        &factors(),
        &budget,
        |_, _, _, _, _| panic!("native support should stop before model"),
        || false,
    )?;
    assert_eq!(report.pairs().len(), 1);
    assert!(report.pairs()[0].evidence.supported());
    let SearchStageEvidence::Native(e) = &report.pairs()[0].evidence.terminal else {
        return Err("expected native proof".into());
    };
    let coordinates = e.evidence.search.correspondences.clone();
    assert!(coordinates.len() >= 10);
    assert!(budget.used() > coordinates.capacity_bytes());
    drop(report);
    assert_eq!(budget.used(), coordinates.capacity_bytes());
    drop(coordinates);
    assert_eq!(budget.used(), 0);
    Ok(())
}

#[test]
fn insufficient_geometry_is_retained_for_every_pair_in_both_search_orders() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (files, weights) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    for tiled in [false, true] {
        for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
            let mut p = collection();
            p.search.order = order;
            p.search.model.anchors.local.minimum_witnesses = 11;
            native_miss(&mut p.search);
            if tiled {
                p.search.model_tiles = Some(rrrah_dedup::model_tiles::ModelTileWorkLimits::for_model(
                    p.search.model,
                )?);
            }
            p.search.work = p.search.required_work()?;
            p.work = work(p.search.work, 3);
            let calls = Cell::new(0);
            let report = scan_portfolio_outcome_collection_with(
                files.clone(),
                &weights,
                p,
                &factors(),
                &budget,
                |_, _, _, b, _| {
                    calls.set(calls.get() + 1);
                    if !tiled || (calls.get() - 1) % 25 == 0 {
                        model_fixture::proposals(b)
                    } else {
                        Ok(b.try_buffer(
                            0,
                            rrrah_dedup::proposals::ScoredProposal {
                                source: [0.; 2],
                                target: [0.; 2],
                                confidence: 0.,
                            },
                        )?
                        .freeze())
                    }
                },
                || false,
            )?;
            assert_eq!(report.file_ids(), &[0, 7, u64::MAX]);
            assert_eq!(report.pairs().len(), 3);
            assert_eq!(calls.get(), if tiled { 75 } else { 3 });
            for pair in report.pairs() {
                assert_eq!(pair.evidence.support(), None);
                match (&pair.evidence, tiled) {
                    (
                        PortfolioPairOutcome::Inconclusive(InsufficientAnchorStage::ModelTiles {
                            attempts: 25,
                            insufficient_attempts: 1,
                            ..
                        }),
                        true,
                    ) => {}
                    (PortfolioPairOutcome::Inconclusive(InsufficientAnchorStage::Model { .. }), false) => {}
                    other => panic!("unknown reason lost: {other:?}"),
                }
            }
            drop(report);
            assert_eq!(budget.used(), 0);
        }
    }
    Ok(())
}

#[test]
fn fatal_failure_after_unknown_still_discards_entire_outcome_collection() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (files, weights) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = collection();
    p.search.model.anchors.local.minimum_witnesses = 11;
    native_miss(&mut p.search);
    p.search.work = p.search.required_work()?;
    p.work = work(p.search.work, 3);
    for cancel_owner in [false, true] {
        let generation = Arc::new(AtomicU64::new(1));
        let mut owned = files.clone();
        owned[2].1.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
        let calls = Cell::new(0);
        let result = scan_portfolio_outcome_collection_with(
            owned,
            &weights,
            p,
            &factors(),
            &budget,
            |_, _, _, b, cancel| {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    if cancel_owner {
                        generation.store(2, Ordering::Release);
                        assert!(cancel());
                        generation.store(1, Ordering::Release);
                    } else {
                        return Err(std::io::Error::other("fatal provider after unknown").into());
                    }
                }
                model_fixture::proposals(b)
            },
            || false,
        );
        if cancel_owner {
            assert!(matches!(result, Err(PortfolioCollectionError::Cancelled)));
        } else {
            assert!(matches!(result, Err(PortfolioCollectionError::Pair { .. })));
        }
        assert_eq!(calls.get(), 2);
        assert_eq!(budget.used(), 0);
    }
    Ok(())
}

#[test]
fn scheduled_outcome_collection_keeps_all_pairs_and_canonical_report() -> TestResult {
    use rrrah_dedup::pair_schedule::CompletePairSchedule;
    let dir = tempfile::tempdir()?;
    let (files, weights) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    for hints in [vec![], vec![(7, u64::MAX)]] {
        let schedule = CompletePairSchedule::new(&[u64::MAX, 0, 7], &hints, 3, 3, 1, &budget, || false)?;
        let baseline = budget.used();
        let calls = Cell::new(0);
        let report = scan_portfolio_scheduled_outcome_collection_with(
            files.clone(),
            &schedule,
            &weights,
            collection(),
            &factors(),
            &budget,
            |a, b, c, d, e| {
                calls.set(calls.get() + 1);
                fake(a, b, c, d, e)
            },
            || false,
        )?;
        assert_eq!(
            report
                .pairs()
                .iter()
                .map(|p| (p.left, p.right))
                .collect::<Vec<_>>(),
            vec![(0, 7), (0, u64::MAX), (7, u64::MAX)]
        );
        assert!(report.pairs().iter().all(|p| p.evidence.support() == Some(true)));
        assert_eq!(calls.get(), 3);
        drop(report);
        assert_eq!(budget.used(), baseline);
        drop(schedule);
        assert_eq!(budget.used(), 0);
    }
    Ok(())
}
#[test]
fn scheduled_mismatch_refuses_before_missing_source_or_model_io() -> TestResult {
    use rrrah_dedup::pair_schedule::CompletePairSchedule;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let schedule = CompletePairSchedule::new(&[0, 7, 99], &[], 3, 3, 0, &budget, || false)?;
    let baseline = budget.used();
    let files = [
        (0, DecodeRequest::new("missing-left")),
        (7, DecodeRequest::new("missing-right")),
        (100, DecodeRequest::new("missing-third")),
    ];
    let result = scan_portfolio_scheduled_outcome_collection_with(
        files,
        &schedule,
        std::path::Path::new("missing-weights"),
        collection(),
        &factors(),
        &budget,
        |_, _, _, _, _| panic!("mismatched schedule must not call provider"),
        || false,
    );
    assert!(matches!(result, Err(PortfolioCollectionError::Pairs)));
    assert_eq!(budget.used(), baseline);
    drop(schedule);
    assert_eq!(budget.used(), 0);
    Ok(())
}
#[test]
fn scheduled_late_provider_failure_discards_prior_confirmed_pairs() -> TestResult {
    use rrrah_dedup::pair_schedule::CompletePairSchedule;
    let dir = tempfile::tempdir()?;
    let (files, weights) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let schedule =
        CompletePairSchedule::new(&[0, 7, u64::MAX], &[(7, u64::MAX)], 3, 3, 1, &budget, || false)?;
    let baseline = budget.used();
    let calls = Cell::new(0);
    let result = scan_portfolio_scheduled_outcome_collection_with(
        files,
        &schedule,
        &weights,
        collection(),
        &factors(),
        &budget,
        |a, b, c, d, e| {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                return Err(std::io::Error::other("late fallback provider failure").into());
            }
            fake(a, b, c, d, e)
        },
        || false,
    );
    assert!(matches!(
        result,
        Err(PortfolioCollectionError::Pair {
            left: 0,
            right: u64::MAX,
            ..
        })
    ));
    assert_eq!(calls.get(), 3);
    assert_eq!(budget.used(), baseline);
    drop(schedule);
    assert_eq!(budget.used(), 0);
    Ok(())
}
#[test]
fn scheduled_fallback_retains_inconclusive_geometry_as_unknown() -> TestResult {
    use rrrah_dedup::pair_schedule::CompletePairSchedule;
    let dir = tempfile::tempdir()?;
    let (files, weights) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let schedule =
        CompletePairSchedule::new(&[0, 7, u64::MAX], &[(7, u64::MAX)], 3, 3, 1, &budget, || false)?;
    let baseline = budget.used();
    let mut p = collection();
    p.search.model.anchors.local.minimum_witnesses = 11;
    native_miss(&mut p.search);
    p.search.work = p.search.required_work()?;
    p.work = work(p.search.work, 3);
    let report = scan_portfolio_scheduled_outcome_collection_with(
        files,
        &schedule,
        &weights,
        p,
        &factors(),
        &budget,
        fake,
        || false,
    )?;
    assert_eq!(report.pairs().len(), 3);
    assert!(report.pairs().iter().all(|p| p.evidence.support().is_none()));
    drop(report);
    assert_eq!(budget.used(), baseline);
    drop(schedule);
    assert_eq!(budget.used(), 0);
    Ok(())
}

fn recovery_collection() -> RecoveryCollectionPolicy {
    use rrrah_dedup::{anchor_rank_file::OppositeAnchorSearchPolicy, search_portfolio::SearchRecoveryPolicy};
    let primary = combined(SearchOrder::NativeFirst);
    let mut search = SearchRecoveryPolicy {
        primary,
        opposite: OppositeAnchorSearchPolicy {
            search: primary.native,
            maximum_swapped_points: primary.native.search.search.search.max_union_points,
        },
        work: primary.work,
    };
    search.work = search.required_work().unwrap();
    let mut p = RecoveryCollectionPolicy { search, max_files: 3, max_pairs: 3, work: search.work };
    p.work = p.required_work().unwrap();
    p
}

#[test]
fn recovery_collection_preflight_precedes_iterator_and_io() -> TestResult {
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    for field in 0..10 {
        let mut p = recovery_collection();
        match field {
            0 => p.work.point_checks -= 1,
            1 => p.work.selection_checks -= 1,
            2 => p.work.rank_sites -= 1,
            3 => p.work.pixel_reads -= 1,
            4 => p.work.geometry_hypotheses -= 1,
            5 => p.work.model_input_reads -= 1,
            6 => p.max_pairs = usize::MAX,
            7 => p.search.opposite.maximum_swapped_points = 0,
            8 => p.search.work.point_checks -= 1,
            _ => p.search.opposite.maximum_swapped_points = usize::MAX,
        }
        let files = std::iter::from_fn(|| -> Option<(u64, DecodeRequest)> { panic!("iterator before preflight") });
        assert!(scan_recovery_collection_with(files, None, std::path::Path::new("/missing/weights"), p,
            &factors(), &budget, |_, _, _, _, _| panic!("provider"), || false).is_err());
        assert_eq!(budget.used(), 0);
    }
    let budget = MemoryBudget::new(0);
    let files = std::iter::from_fn(|| -> Option<(u64, DecodeRequest)> { panic!("metadata before iterator") });
    assert!(matches!(scan_recovery_collection_with(files, None, std::path::Path::new("/missing/weights"),
        recovery_collection(), &factors(), &budget, fake, || false), Err(PortfolioCollectionError::Memory(_))));
    Ok(())
}

#[test]
fn recovery_collection_unknowns_late_provider_failure_and_final_weight_guard() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (files, weights) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = recovery_collection();
    native_miss(&mut p.search.primary);
    p.search.primary.model.anchors.local.minimum_witnesses = 11;
    p.search.primary.work = p.search.primary.required_work()?;
    p.search.opposite.search = p.search.primary.native;
    p.search.work = p.search.required_work()?;
    p.work = p.required_work()?;
    let checkpoints = Cell::new(0);
    let report = scan_recovery_collection_with(files.clone(), None, &weights, p,
        &factors(), &budget, fake, || { checkpoints.set(checkpoints.get() + 1); false })?;
    assert_eq!(report.file_ids(), &[0, 7, u64::MAX]);
    assert_eq!(report.pairs().len(), 3);
    assert!(report.pairs().iter().all(|pair| pair.evidence.supported().is_none()));
    drop(report);
    assert_eq!(budget.used(), 0);
    let last = checkpoints.get();
    let calls = Cell::new(0);
    let result = scan_recovery_collection_with(files.clone(), None, &weights, p,
        &factors(), &budget, |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 2 { return Err(std::io::Error::other("late provider").into()); }
            model_fixture::proposals(b)
        }, || false);
    assert!(matches!(result, Err(PortfolioCollectionError::Pair { .. })));
    assert_eq!(calls.get(), 2);
    assert_eq!(budget.used(), 0);
    checkpoints.set(0);
    let result = scan_recovery_collection_with(files, None, &weights, p,
        &factors(), &budget, fake, || {
            checkpoints.set(checkpoints.get() + 1);
            if checkpoints.get() == last { std::fs::write(&weights, b"changed weights").unwrap(); }
            false
        });
    assert!(matches!(result, Err(PortfolioCollectionError::ModelSource(_))), "{result:?}");
    assert_eq!(budget.used(), 0);
    Ok(())
}

#[test]
fn recovery_collection_schedule_keeps_fallback_pairs_and_canonical_publication() -> TestResult {
    use rrrah_dedup::{pair_schedule::CompletePairSchedule, search_portfolio::SearchRecoveryOutcome};
    let dir = tempfile::tempdir()?;
    let weights = dir.path().join("weights");
    std::fs::write(&weights, b"fixture provider")?;
    let source = DecodeRequest::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rotation/base.png"));
    let files = [(7, source.clone()), (0, source.clone()), (u64::MAX, source)];
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let schedule = CompletePairSchedule::new(&[7, 0, u64::MAX], &[(7, u64::MAX)],
        3, 3, 1, &budget, || false)?;
    let schedule_bytes = budget.used();
    let mut p = recovery_collection();
    native_miss(&mut p.search.primary);
    p.search.primary.model.decode.max_pixels = 200000;
    p.search.primary.model.input.max_source_pixels = 200000;
    p.search.primary.model.input.max_pixel_reads = 800000;
    p.search.primary.work = p.search.primary.required_work()?;
    p.search.work = p.search.required_work()?;
    p.work = p.required_work()?;
    let calls = Cell::new(0);
    let report = scan_recovery_collection_with(files.clone(), Some(&schedule), &weights, p,
        &factors(), &budget, |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            Ok(b.try_buffer(0, rrrah_dedup::proposals::ScoredProposal {
                source: [0.; 2], target: [0.; 2], confidence: 0.,
            })?.freeze())
        }, || false)?;
    assert_eq!(calls.get(), 3);
    assert_eq!(report.pairs().iter().map(|pair| (pair.left, pair.right)).collect::<Vec<_>>(),
        vec![(0, 7), (0, u64::MAX), (7, u64::MAX)]);
    assert!(report.pairs().iter().all(|pair| pair.evidence.supported() == Some(true)
        && matches!(pair.evidence, SearchRecoveryOutcome::Opposite { .. })));
    drop(report);
    assert_eq!(budget.used(), schedule_bytes);
    let mismatch = CompletePairSchedule::new(&[0, 7], &[], 2, 1, 0, &budget, || false)?;
    assert!(scan_recovery_collection_with(files, Some(&mismatch), &weights, p,
        &factors(), &budget, |_, _, _, _, _| panic!("schedule mismatch provider"), || false).is_err());
    drop(mismatch);
    drop(schedule);
    assert_eq!(budget.used(), 0);
    Ok(())
}

#[test]
fn joint_collection_projection_preflight_precedes_iterator_and_exhaustive_recovery() -> TestResult {
    use std::path::Path;
    use rrrah_dedup::portfolio_collection::scan_joint_recovery_collection_with;
    let mut p=recovery_collection();
    let g=p.search.opposite.search.search.search.search.search.gradient.search;
    let pair=12*g.local.geometry.max_points as u64*(g.sampling.trials+1);
    let budget=MemoryBudget::new(128*1024*1024);
    for (per,total) in [(pair-1,3*pair),(pair,3*pair-1)] {
        let files=std::iter::from_fn(||->Option<(u64,DecodeRequest)>{panic!("preflight iterator")});
        assert!(scan_joint_recovery_collection_with(files,None,Path::new("missing-weights"),p,&factors(),&budget,per,total,fake,||false).is_err());assert_eq!(budget.used(),0);
    }
    native_miss(&mut p.search.primary);p.search.primary.model.anchors.local.minimum_witnesses=11;
    p.search.primary.model.decode.max_pixels=200000;
    p.search.primary.model.input.max_source_pixels=200000;
    p.search.primary.model.input.max_pixel_reads=800000;
    p.search.primary.work=p.search.primary.required_work()?;p.search.work=p.search.required_work()?;p.work=p.required_work()?;
    let dir=tempfile::tempdir()?;let weights=dir.path().join("weights");std::fs::write(&weights,b"fixture")?;
    let source=DecodeRequest::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));
    let files=[(9,source.clone()),(2,source.clone()),(6,source)];
    let report=scan_joint_recovery_collection_with(files,None,&weights,p,&factors(),&budget,pair,3*pair,fake,||false)?;
    assert_eq!(report.file_ids(),&[2,6,9]);assert_eq!(report.pairs().len(),3);assert!(report.pairs().iter().all(|r|r.evidence.supported()==Some(true)));
    drop(report);assert_eq!(budget.used(),0);Ok(())
}

#[test]
fn joint_model_insufficient_geometry_is_retained_for_every_pair_in_both_search_orders() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (files, weights) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    for tiled in [false, true] {
        for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
            let mut p = collection();
            p.search.order = order;
            p.search.model.anchors.local.minimum_witnesses = 11;
            native_miss(&mut p.search);
            if tiled {
                p.search.model_tiles = Some(rrrah_dedup::model_tiles::ModelTileWorkLimits::for_model(
                    p.search.model,
                )?);
            }
            p.search.work = p.search.required_work()?;
            p.work = work(p.search.work, 3);
            let calls = Cell::new(0);
            let report = rrrah_dedup::portfolio_collection::scan_joint_model_portfolio_outcome_collection_with(
                files.clone(),
                &weights,
                p,
                 2*(p.search.model.geometry.max_points as u64)*(p.search.model.sampling.trials+1),
                 25*2*(p.search.model.geometry.max_points as u64)*(p.search.model.sampling.trials+1),
                 75*2*(p.search.model.geometry.max_points as u64)*(p.search.model.sampling.trials+1),
                &factors(),
                &budget,
                |_, _, _, b, _| {
                    calls.set(calls.get() + 1);
                    if !tiled || (calls.get() - 1) % 25 == 0 {
                        model_fixture::proposals(b)
                    } else {
                        Ok(b.try_buffer(
                            0,
                            rrrah_dedup::proposals::ScoredProposal {
                                source: [0.; 2],
                                target: [0.; 2],
                                confidence: 0.,
                            },
                        )?
                        .freeze())
                    }
                },
                || false,
            )?;
            assert_eq!(report.file_ids(), &[0, 7, u64::MAX]);
            assert_eq!(report.pairs().len(), 3);
            assert_eq!(calls.get(), if tiled { 75 } else { 3 });
            for pair in report.pairs() {
                assert_eq!(pair.evidence.support(), None);
                match (&pair.evidence, tiled) {
                    (
                        PortfolioPairOutcome::Inconclusive(InsufficientAnchorStage::ModelTiles {
                            attempts: 25,
                            insufficient_attempts: 1,
                            ..
                        }),
                        true,
                    ) => {}
                    (PortfolioPairOutcome::Inconclusive(InsufficientAnchorStage::Model { .. }), false) => {}
                    other => panic!("unknown reason lost: {other:?}"),
                }
            }
            drop(report);
            assert_eq!(budget.used(), 0);
        }
    }
    Ok(())
}

#[test]
fn joint_model_fatal_failure_after_unknown_still_discards_entire_outcome_collection() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (files, weights) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let mut p = collection();
    p.search.model.anchors.local.minimum_witnesses = 11;
    native_miss(&mut p.search);
    p.search.work = p.search.required_work()?;
    p.work = work(p.search.work, 3);
    for cancel_owner in [false, true] {
        let generation = Arc::new(AtomicU64::new(1));
        let mut owned = files.clone();
        owned[2].1.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
        let calls = Cell::new(0);
        let result = rrrah_dedup::portfolio_collection::scan_joint_model_portfolio_outcome_collection_with(
            owned,
            &weights,
            p,
             2*(p.search.model.geometry.max_points as u64)*(p.search.model.sampling.trials+1),
             25*2*(p.search.model.geometry.max_points as u64)*(p.search.model.sampling.trials+1),
             75*2*(p.search.model.geometry.max_points as u64)*(p.search.model.sampling.trials+1),
            &factors(),
            &budget,
            |_, _, _, b, cancel| {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    if cancel_owner {
                        generation.store(2, Ordering::Release);
                        assert!(cancel());
                        generation.store(1, Ordering::Release);
                    } else {
                        return Err(std::io::Error::other("fatal provider after unknown").into());
                    }
                }
                model_fixture::proposals(b)
            },
            || false,
        );
        if cancel_owner {
            assert!(matches!(result, Err(PortfolioCollectionError::Cancelled)));
        } else {
            assert!(matches!(result, Err(PortfolioCollectionError::Pair { .. })));
        }
        assert_eq!(calls.get(), 2);
        assert_eq!(budget.used(), 0);
    }
    Ok(())
}

#[test]
fn joint_model_collection_projection_preflight_and_canonical_exhaustive_support() -> TestResult {
    use rrrah_dedup::portfolio_collection::scan_joint_model_portfolio_outcome_collection_with as scan;
    let p=tiled_collection();let per=2*(p.search.model.geometry.max_points as u64)*(p.search.model.sampling.trials+1);
    let budget=MemoryBudget::new(128*1024*1024);
    for (attempt,pair,total) in [(per-1,per*25,per*75),(per,per*25-1,per*75),(per,per*25,per*75-1),(per,u64::MAX,u64::MAX)] {
        let files=std::iter::from_fn(||->Option<(u64,DecodeRequest)>{panic!("iterator before full projection admission")});
        assert!(scan(files,std::path::Path::new("missing"),p,attempt,pair,total,&factors(),&budget,fake,||false).is_err());
        assert_eq!(budget.used(),0);
    }
    let dir=tempfile::tempdir()?;let (files,weights)=three(dir.path())?;
    let report=scan(files,&weights,p,per,25*per,75*per,&factors(),&budget,fake,||false)?;
    assert_eq!(report.file_ids(),&[0,7,u64::MAX]);assert_eq!(report.pairs().len(),3);
    assert!(report.pairs().iter().all(|p|p.evidence.support()==Some(true)));
    let retained=budget.used();assert!(retained>0);let report=Arc::new(report);let cloned=report.clone();drop(report);assert_eq!(budget.used(),retained);drop(cloned);assert_eq!(budget.used(),0);
    Ok(())
}

fn bank_setup(mut p:SearchPortfolioPolicy)->(SearchPortfolioPolicy,rrrah_dedup::joint_witnesses::JointBankPolicy,rrrah_dedup::model_tiles::ModelTileWorkLimits) {
 use rrrah_dedup::{geometry::JointProjectivePolicy,joint_witnesses::{JointBankPolicy,JointWitnessPolicy},model_tiles::ModelTileWorkLimits};
 p.model.anchors.local.maximum_points=400;p.model.anchors.local.maximum_point_checks=100000;p.model.anchors.maximum_selection_checks=250000;
 let m=p.model;let n=25*m.proposals.max_proposals as u64;let fit=2*m.geometry.max_points as u64*(m.sampling.trials+1);let sep=n*(n-1)/2;let mut target=m.geometry;target.min_inliers=4;
 let banks=JointBankPolicy{hypothesis:JointProjectivePolicy{target,source_tolerance:m.anchors.local.source_tolerance,maximum_projection_checks:fit},sampling:m.sampling,witnesses:JointWitnessPolicy{source_tolerance:m.anchors.local.source_tolerance,target_tolerance:m.geometry.tolerance,minimum_separation:2.,maximum_points:n as usize,maximum_projection_checks:2*n,maximum_separation_checks:sep},minimum_witnesses:10,maximum_banks:25,maximum_total_projection_checks:25*(fit+2*n),maximum_total_separation_checks:25*sep};
 let mut work=ModelTileWorkLimits::for_model(m).unwrap();work.model_input_reads=52*m.input.max_pixel_reads;work.point_checks*=2;work.selection_checks*=2;work.rank_sites*=2;work.pixel_reads*=2;p.work=rrrah_dedup::search_portfolio::required_model_bank_portfolio_work(p,work).unwrap();(p,banks,work)
}
fn bank_collection()->(PortfolioCollectionPolicy,rrrah_dedup::joint_witnesses::JointBankPolicy,rrrah_dedup::model_tiles::ModelTileWorkLimits,ModelBankCollectionWorkLimits) {
    let mut search=combined(SearchOrder::ModelFirst);native_miss(&mut search);
    let(search,banks,bank_work)=bank_setup(search);
    (PortfolioCollectionPolicy{search,max_files:3,max_pairs:3,work:work(search.work,3)},banks,bank_work,
     ModelBankCollectionWorkLimits{projection_checks:3*banks.maximum_total_projection_checks,separation_checks:3*banks.maximum_total_separation_checks})
}
#[test]
fn bank_collection_all_pairs_both_orders_and_scheduling_preserve_canonical_records()->TestResult {
    let dir=tempfile::tempdir()?;let(files,weights)=three(dir.path())?;let budget=MemoryBudget::new(128*1024*1024);
    let schedule=rrrah_dedup::pair_schedule::CompletePairSchedule::new(&[u64::MAX,0,7],&[],3,3,0,&budget,||false)?;
    for order in [SearchOrder::ModelFirst,SearchOrder::NativeFirst] {
        for scheduled in [false,true] {
            let(mut p,banks,w,total)=bank_collection();p.search.order=order;let calls=Cell::new(0);
            let report=scan_model_bank_portfolio_outcome_collection_with(files.clone(),scheduled.then_some(&schedule),&weights,p,banks,w,&[12,1],total,&factors(),&budget,|a,l,r,b,c|{calls.set(calls.get()+1);fake(a,l,r,b,c)},||false)?;
            assert_eq!(calls.get(),75);assert_eq!(report.file_ids(),&[0,7,u64::MAX]);assert_eq!(report.pairs().len(),3);assert!(report.pairs().iter().all(|p|p.evidence.support()==Some(true)));
            let retained=budget.used();let report=Arc::new(report);let clone=report.clone();drop(report);assert_eq!(budget.used(),retained);drop(clone);
        }
    }
    drop(schedule);assert_eq!(budget.used(),0);Ok(())
}
#[test]
fn bank_collection_complete_work_and_all_radii_precede_iterator_consumption()->TestResult {
    let budget=MemoryBudget::new(128*1024*1024);
    for case in 0..6 {
        let(mut p,banks,w,mut total)=bank_collection();let mut radii=&[2,6][..];
        match case {0=>p.work.pixel_reads-=1,1=>total.projection_checks-=1,2=>total.separation_checks-=1,3=>radii=&[],4=>p.search.work.model_input_reads-=1,_=>p.max_pairs=usize::MAX}
        let files=std::iter::from_fn(||->Option<(u64,DecodeRequest)>{panic!("iterator before admission")});
        let e=scan_model_bank_portfolio_outcome_collection_with(files,None,std::path::Path::new("missing"),p,banks,w,radii,total,&factors(),&budget,fake,||false);assert!(e.is_err(),"case{case}");assert_eq!(budget.used(),0);
    }
    let(p,banks,w,total)=bank_collection();let files=std::iter::from_fn(||->Option<(u64,DecodeRequest)>{panic!("cancel before iterator")});let e=scan_model_bank_portfolio_outcome_collection_with(files,None,std::path::Path::new("missing"),p,banks,w,&[],total,&factors(),&budget,fake,||true);assert!(matches!(e,Err(PortfolioCollectionError::Cancelled)));assert_eq!(budget.used(),0);Ok(())
}
#[test]
fn bank_collection_unknown_is_retained_and_later_fatal_or_weight_change_discards_batch()->TestResult {
    for failure in 0..4 {
        let dir=tempfile::tempdir()?;let(files,weights)=three(dir.path())?;let budget=MemoryBudget::new(128*1024*1024);let(p,banks,w,total)=bank_collection();let calls=Cell::new(0);
        let generation=Arc::new(AtomicU64::new(1));let mut owned=files.clone();owned[2].1.cancellation=Some(rrrah_decode::GenerationToken::new(generation.clone(),1));
        let e=scan_model_bank_portfolio_outcome_collection_with(owned,None,&weights,p,banks,w,&[2,6],total,&factors(),&budget,|_,_,_,b,c|{calls.set(calls.get()+1);if calls.get()==26 {match failure {1=>return Err(std::io::Error::other("provider after first unknown").into()),2=>{generation.store(2,Ordering::Release);assert!(c());generation.store(1,Ordering::Release);},3=>std::fs::write(&weights,b"changed")?,_=>{}}}Ok(b.try_buffer(0,rrrah_dedup::proposals::ScoredProposal{source:[0.;2],target:[0.;2],confidence:0.})?.freeze())},||false);
        if failure==0 {let report=e?;assert_eq!(calls.get(),75);assert_eq!(report.pairs().len(),3);assert!(report.pairs().iter().all(|p|p.evidence.support().is_none()));drop(report);}
        else {assert!(e.is_err());assert_eq!(calls.get(),26);if failure==2{assert!(matches!(e,Err(PortfolioCollectionError::Cancelled)));}}
        assert_eq!(budget.used(),0);
    }
    Ok(())
}

fn bank_recovery_collection()->(RecoveryCollectionPolicy,rrrah_dedup::joint_witnesses::JointBankPolicy,rrrah_dedup::model_tiles::ModelTileWorkLimits,ModelBankCollectionWorkLimits,u64) {
    let mut p=recovery_collection();native_miss(&mut p.search.primary);let(primary,banks,w)=bank_setup(p.search.primary);p.search.primary=primary;p.search.work=rrrah_dedup::search_portfolio::required_model_bank_recovery_work(p.search,w).unwrap();p.work=p.required_work().unwrap();let g=p.search.opposite.search.search.search.search.search.gradient.search;let projections=12*g.local.geometry.max_points as u64*(g.sampling.trials+1);
    (p,banks,w,ModelBankCollectionWorkLimits{projection_checks:3*banks.maximum_total_projection_checks,separation_checks:3*banks.maximum_total_separation_checks},projections)
}
#[test]
fn bank_recovery_collection_checks_all_cumulative_work_before_iterator() {
    let b=MemoryBudget::new(128*1024*1024);
    for case in 0..6 {let(mut p,banks,w,mut total,cap)=bank_recovery_collection();let mut opposite_total=3*cap;let mut radii=&[2,6][..];match case{0=>p.work.pixel_reads-=1,1=>total.projection_checks-=1,2=>total.separation_checks-=1,3=>opposite_total-=1,4=>radii=&[],_=>p.max_pairs=usize::MAX}
        let files=std::iter::from_fn(||->Option<(u64,DecodeRequest)>{panic!("iterator before complete recovery admission")});let e=scan_model_bank_joint_recovery_collection_with(files,None,std::path::Path::new("missing"),p,banks,w,radii,total,cap,opposite_total,&factors(),&b,fake,||false);assert!(e.is_err());assert_eq!(b.used(),0);
    }
}
#[test]
fn bank_recovery_collection_primary_support_canonical_all_pairs_both_orders_and_schedule()->TestResult {
    let dir=tempfile::tempdir()?;let(files,weights)=three(dir.path())?;let b=MemoryBudget::new(128*1024*1024);let schedule=rrrah_dedup::pair_schedule::CompletePairSchedule::new(&[u64::MAX,0,7],&[],3,3,0,&b,||false)?;
    for order in [SearchOrder::ModelFirst,SearchOrder::NativeFirst] {let(mut p,banks,w,total,cap)=bank_recovery_collection();p.search.primary.order=order;let calls=Cell::new(0);let report=scan_model_bank_joint_recovery_collection_with(files.clone(),Some(&schedule),&weights,p,banks,w,&[12,1],total,cap,3*cap,&factors(),&b,|a,l,r,b,c|{calls.set(calls.get()+1);fake(a,l,r,b,c)},||false)?;assert_eq!(report.file_ids(),&[0,7,u64::MAX]);assert_eq!(report.pairs().len(),3);assert!(report.pairs().iter().all(|p|p.evidence.supported()==Some(true)));assert_eq!(calls.get(),75);drop(report);}
    drop(schedule);assert_eq!(b.used(),0);Ok(())
}
#[test]
fn bank_recovery_collection_unknown_advances_to_opposite_and_retains_managed_evidence()->TestResult {
    let dir=tempfile::tempdir()?;let weights=dir.path().join("weights");std::fs::write(&weights,b"fixture")?;let source=DecodeRequest::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));let b=MemoryBudget::new(128*1024*1024);let(mut p,banks,mut w,total,cap)=bank_recovery_collection();p.search.primary.model.decode.max_pixels=200000;p.search.primary.model.input.max_source_pixels=200000;p.search.primary.model.input.max_pixel_reads=800000;w.model_input_reads=52*p.search.primary.model.input.max_pixel_reads;p.search.primary.work=rrrah_dedup::search_portfolio::required_model_bank_portfolio_work(p.search.primary,w)?;p.search.work=rrrah_dedup::search_portfolio::required_model_bank_recovery_work(p.search,w)?;p.work=p.required_work()?;
    let report=scan_model_bank_joint_recovery_collection_with([(9,source.clone()),(2,source)],None,&weights,p,banks,w,&[2,6],total,cap,3*cap,&factors(),&b,|_,_,_,b,_|Ok(b.try_buffer(0,rrrah_dedup::proposals::ScoredProposal{source:[0.;2],target:[0.;2],confidence:0.})?.freeze()),||false)?;assert_eq!(report.pairs().len(),1);assert_eq!(report.pairs()[0].evidence.supported(),Some(true));assert!(matches!(report.pairs()[0].evidence,rrrah_dedup::search_portfolio::SearchRecoveryOutcome::Opposite{primary_insufficient:Some(_),..}));let retained=b.used();assert!(retained>0);let report=Arc::new(report);let clone=report.clone();drop(report);assert_eq!(b.used(),retained);drop(clone);assert_eq!(b.used(),0);Ok(())
}
#[test]
fn bank_recovery_collection_late_fatal_discards_earlier_unknown_and_cancel_wins()->TestResult {
    for failure in 0..3 {let dir=tempfile::tempdir()?;let(files,weights)=three(dir.path())?;let b=MemoryBudget::new(128*1024*1024);let(mut p,banks,w,total,cap)=bank_recovery_collection();p.search.opposite.search=p.search.primary.native;let calls=Cell::new(0);let generation=Arc::new(AtomicU64::new(1));let mut owned=files.clone();owned[2].1.cancellation=Some(rrrah_decode::GenerationToken::new(generation.clone(),1));let e=scan_model_bank_joint_recovery_collection_with(owned,None,&weights,p,banks,w,&[2,6],total,cap,3*cap,&factors(),&b,|_,_,_,b,c|{calls.set(calls.get()+1);if calls.get()==26{match failure{0=>return Err(std::io::Error::other("fatal after unknown").into()),1=>std::fs::write(&weights,b"changed")?,_=>{generation.store(2,Ordering::Release);assert!(c());generation.store(1,Ordering::Release);}}}Ok(b.try_buffer(0,rrrah_dedup::proposals::ScoredProposal{source:[0.;2],target:[0.;2],confidence:0.})?.freeze())},||false);assert!(e.is_err());assert_eq!(calls.get(),26);if failure==2{assert!(matches!(e,Err(PortfolioCollectionError::Cancelled)));}assert_eq!(b.used(),0);}
    Ok(())
}

#[test]
fn native_bank_collection_checks_all_cumulative_work_before_iterator() {
    let b=MemoryBudget::new(128*1024*1024);
    for case in 0..6 {let(mut p,banks,w,mut total,cap)=bank_recovery_collection();let mut opposite_total=3*cap;let mut radii=&[2,6][..];match case{0=>p.work.pixel_reads-=1,1=>total.projection_checks-=1,2=>total.separation_checks-=1,3=>opposite_total-=1,4=>radii=&[],_=>p.max_pairs=usize::MAX}
        let files=std::iter::from_fn(||->Option<(u64,DecodeRequest)>{panic!("iterator before complete recovery admission")});let e=rrrah_dedup::portfolio_collection::scan_native_bank_recovery_collection_with(files,None,std::path::Path::new("missing"),p,banks,w,radii,total,cap,opposite_total,&factors(),&b,fake,||false);assert!(e.is_err());assert_eq!(b.used(),0);
    }
}
#[test]
fn native_bank_collection_primary_support_canonical_all_pairs_both_orders_and_schedule()->TestResult {
    let dir=tempfile::tempdir()?;let(files,weights)=three(dir.path())?;let b=MemoryBudget::new(128*1024*1024);let schedule=rrrah_dedup::pair_schedule::CompletePairSchedule::new(&[u64::MAX,0,7],&[],3,3,0,&b,||false)?;
    for order in [SearchOrder::ModelFirst,SearchOrder::NativeFirst] {let(mut p,banks,w,total,cap)=bank_recovery_collection();p.search.primary.order=order;p.search.opposite.search=p.search.primary.native;let calls=Cell::new(0);let report=rrrah_dedup::portfolio_collection::scan_native_bank_recovery_collection_with(files.clone(),Some(&schedule),&weights,p,banks,w,&[12,1],total,cap,3*cap,&factors(),&b,|a,l,r,b,c|{calls.set(calls.get()+1);fake(a,l,r,b,c)},||false)?;assert_eq!(report.file_ids(),&[0,7,u64::MAX]);assert_eq!(report.pairs().len(),3);assert!(report.pairs().iter().all(|p|p.evidence.supported()==Some(true)));assert_eq!(calls.get(),75);drop(report);}
    drop(schedule);assert_eq!(b.used(),0);Ok(())
}
#[test]
fn native_bank_collection_opposite_support_skips_model_and_retains_managed_evidence()->TestResult {
    let dir=tempfile::tempdir()?;let weights=dir.path().join("weights");std::fs::write(&weights,b"fixture")?;let source=DecodeRequest::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));let b=MemoryBudget::new(128*1024*1024);let(mut p,banks,mut w,total,cap)=bank_recovery_collection();p.search.primary.model.decode.max_pixels=200000;p.search.primary.model.input.max_source_pixels=200000;p.search.primary.model.input.max_pixel_reads=800000;w.model_input_reads=52*p.search.primary.model.input.max_pixel_reads;p.search.primary.work=rrrah_dedup::search_portfolio::required_model_bank_portfolio_work(p.search.primary,w)?;p.search.work=rrrah_dedup::search_portfolio::required_model_bank_recovery_work(p.search,w)?;p.work=p.required_work()?;
    let report=rrrah_dedup::portfolio_collection::scan_native_bank_recovery_collection_with([(9,source.clone()),(2,source)],None,&weights,p,banks,w,&[2,6],total,cap,3*cap,&factors(),&b,|_,_,_,_,_|panic!("opposite support must skip model"),||false)?;assert_eq!(report.pairs().len(),1);assert_eq!(report.pairs()[0].evidence.supported(),Some(true));assert!(matches!(report.pairs()[0].evidence,rrrah_dedup::search_portfolio::NativeBankRecoveryEvidence{native_miss:Some(_),opposite_insufficient:Some(false),terminal:rrrah_dedup::search_portfolio::NativeBankRecoveryTerminal::Opposite(_),..}));let retained=b.used();assert!(retained>0);let report=Arc::new(report);let clone=report.clone();drop(report);assert_eq!(b.used(),retained);drop(clone);assert_eq!(b.used(),0);Ok(())
}
#[test]
fn native_bank_collection_late_fatal_discards_earlier_unknown_and_cancel_wins()->TestResult {
    for failure in 0..3 {let dir=tempfile::tempdir()?;let(files,weights)=three(dir.path())?;let b=MemoryBudget::new(128*1024*1024);let(mut p,banks,w,total,cap)=bank_recovery_collection();p.search.opposite.search=p.search.primary.native;let calls=Cell::new(0);let generation=Arc::new(AtomicU64::new(1));let mut owned=files.clone();owned[2].1.cancellation=Some(rrrah_decode::GenerationToken::new(generation.clone(),1));let e=rrrah_dedup::portfolio_collection::scan_native_bank_recovery_collection_with(owned,None,&weights,p,banks,w,&[2,6],total,cap,3*cap,&factors(),&b,|_,_,_,b,c|{calls.set(calls.get()+1);if calls.get()==26{match failure{0=>return Err(std::io::Error::other("fatal after unknown").into()),1=>std::fs::write(&weights,b"changed")?,_=>{generation.store(2,Ordering::Release);assert!(c());generation.store(1,Ordering::Release);}}}Ok(b.try_buffer(0,rrrah_dedup::proposals::ScoredProposal{source:[0.;2],target:[0.;2],confidence:0.})?.freeze())},||false);assert!(e.is_err());assert_eq!(calls.get(),26);if failure==2{assert!(matches!(e,Err(PortfolioCollectionError::Cancelled)));}assert_eq!(b.used(),0);}
    Ok(())
}

#[test]
fn native_bank_collection_all_unknown_pairs_preserve_stage_trace() -> TestResult {
    use rrrah_dedup::portfolio_collection::scan_native_bank_recovery_collection_with as scan;
    use rrrah_dedup::search_portfolio::NativeBankRecoveryTerminal;
    let dir = tempfile::tempdir()?;
    let (files, weights) = three(dir.path())?;
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    let (mut p, banks, w, total, cap) = bank_recovery_collection();
    p.search.opposite.search = p.search.primary.native;
    let calls = Cell::new(0);
    let report = scan(files, None, &weights, p, banks, w, &[2, 6], total,
        cap, 3 * cap, &factors(), &budget, |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            Ok(b.try_buffer(0, rrrah_dedup::proposals::ScoredProposal {
                source: [0.; 2], target: [0.; 2], confidence: 0.,
            })?.freeze())
        }, || false)?;
    assert_eq!(calls.get(), 75);
    assert_eq!(report.file_ids(), &[0, 7, u64::MAX]);
    assert_eq!(report.pairs().len(), 3);
    for pair in report.pairs() {
        assert_eq!(pair.evidence.supported(), None);
        assert!(pair.evidence.model_attempted());
        assert!(pair.evidence.native_miss.is_some());
        assert_eq!(pair.evidence.opposite_insufficient, Some(true));
        assert!(matches!(pair.evidence.terminal, NativeBankRecoveryTerminal::Inconclusive(_)));
        let model = pair.evidence.model_search.as_ref().expect("unknown retains model diagnostics");
        assert_eq!(model.attempts, 25);
        assert_eq!(model.comparison.raw_proposals, 0);
        assert_eq!(model.comparison.selected_points, 0);
        assert_eq!(model.pixel_attempts, 0);
        assert_eq!(model.diagnostics, Default::default());
    }
    drop(report);
    assert_eq!(budget.used(), 0);
    Ok(())
}

#[test]
fn native_bank_collection_final_publication_rechecks_every_file_after_last_callback() -> TestResult {
    use rrrah_dedup::portfolio_collection::scan_native_bank_recovery_collection_with as scan;
    for changed in 0..3 {
        let dir = tempfile::tempdir()?;
        let (files, weights) = three(dir.path())?;
        let budget = MemoryBudget::new(128 * 1024 * 1024);
        let (mut p, banks, w, total, cap) = bank_recovery_collection();
        p.search.opposite.search = p.search.primary.native;
        let empty = |_: &std::path::Path, _: &rrrah_dedup::model_input::GrayModelInput,
            _: &rrrah_dedup::model_input::GrayModelInput, b: &MemoryBudget,
            _: &dyn Fn() -> bool| -> Result<_, rrrah_dedup::model_compare::ProposalProviderError> {
            Ok(b.try_buffer(0, rrrah_dedup::proposals::ScoredProposal {
                source: [0.; 2], target: [0.; 2], confidence: 0.,
            })?.freeze())
        };
        let checkpoints = Cell::new(0);
        let report = scan(files.clone(), None, &weights, p, banks, w, &[2, 6], total,
            cap, 3 * cap, &factors(), &budget, empty,
            || { checkpoints.set(checkpoints.get() + 1); false })?;
        drop(report);
        assert_eq!(budget.used(), 0);
        let last = checkpoints.get();
        checkpoints.set(0);
        let result = scan(files.clone(), None, &weights, p, banks, w, &[2, 6], total,
            cap, 3 * cap, &factors(), &budget, empty, || {
                checkpoints.set(checkpoints.get() + 1);
                if checkpoints.get() == last {
                    std::fs::write(&files[changed].1.path, b"source changed at publication").unwrap();
                }
                false
            });
        assert!(matches!(result, Err(PortfolioCollectionError::Source { id, .. })
            if id == files[changed].0), "{result:?}");
        assert_eq!(budget.used(), 0);
    }
    Ok(())
}
