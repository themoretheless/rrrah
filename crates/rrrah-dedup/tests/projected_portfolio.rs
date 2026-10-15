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

use rrrah_core::{MemoryBudget, SharedBuffer};
use rrrah_decode::DecodeRequest;
use rrrah_dedup::{
    anchor_rank_file::NativePixelRefusal,
    model_compare::ProposalProviderError,
    model_input::ModelInputPolicy,
    model_projected_compare::ProjectedModelPolicy,
    model_projected_tiles::ProjectedTileWorkLimits,
    model_projection::ModelInputProjection,
    projected_portfolio::*,
    projected_portfolio_collection::*,
    proposals::{ProposalPolicy, ScoredProposal},
    search_portfolio::SearchOrder,
};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
};
fn projected_policy() -> ProjectedModelPolicy {
    ProjectedModelPolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 200000,
            max_file_bytes: 1048576,
        },
        input: ModelInputPolicy {
            max_side: 64,
            max_source_pixels: 200000,
            max_output_pixels: 4096,
            max_pixel_reads: 800000,
        },
        projection: ModelInputProjection::CompositeReinhardV1 {
            exposure: 1.,
            background: [0.; 3],
        },
        proposals: ProposalPolicy {
            max_proposals: 6,
            max_ranking_checks: 1000,
            max_separation_checks: 1000,
            minimum_separation: 1.,
        },
        geometry: GeometryPolicy {
            tolerance: 0.001,
            min_inliers: 6,
            max_points: 6,
            max_hypotheses: 64,
        },
        sampling: ProjectiveSamplingPolicy {
            trials: 64,
            seed: 173,
        },
        pixels: WarpPolicy {
            tolerance: 0.00001,
            max_source_pixels: 1000000,
        },
        minimum_coverage: 1.,
        minimum_agreement: 1.,
        max_model_bytes: 1024,
    }
}
fn combined(order: SearchOrder) -> ProjectedPortfolioPolicy {
    let model = projected_policy();
    let model_work = ProjectedTileWorkLimits::for_model(model).unwrap();
    let mut p = ProjectedPortfolioPolicy {
        native: portfolio(),
        model,
        model_work,
        order,
        work: ProjectedPortfolioWorkLimits {
            point_checks: 0,
            selection_checks: 0,
            rank_sites: 0,
            rank_pixel_reads: 0,
            geometry_hypotheses: 0,
            model: model_work,
        },
    };
    p.work = p.required_work().unwrap();
    p
}
fn write_alpha(path: &Path, alpha: u8) {
    let mut b = b"qoif".to_vec();
    b.extend(8u32.to_be_bytes());
    b.extend(8u32.to_be_bytes());
    b.extend([4, 1]);
    for _ in 0..64 {
        b.extend([0xff, 0, 0, 0, alpha]);
    }
    b.extend([0, 0, 0, 0, 0, 0, 0, 1]);
    std::fs::write(path, b).unwrap();
}
fn fixture(dir: &Path, alpha: bool) -> ([DecodeRequest; 2], PathBuf) {
    let r = std::array::from_fn(|i| {
        let path = dir.join(format!("{i}.{}", if alpha { "qoi" } else { "pfm" }));
        if alpha {
            write_alpha(&path, 64);
        } else {
            let mut b = b"PF\n8 8\n-1.0\n".to_vec();
            for _ in 0..64 * 3 {
                b.extend(4f32.to_le_bytes());
            }
            std::fs::write(&path, b).unwrap();
        }
        let mut r = DecodeRequest::new(path);
        r.assume_untagged_linear_srgb = true;
        r
    });
    let w = dir.join("weights");
    std::fs::write(&w, b"fixture provider").unwrap();
    (r, w)
}
fn proposals(b: &MemoryBudget) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
    let mut p = b.try_buffer(
        6,
        ScoredProposal {
            source: [0.; 2],
            target: [0.; 2],
            confidence: 1.,
        },
    )?;
    for (i, p) in p.iter_mut().enumerate() {
        let q = [[0.75, 3.25, 6.25][i % 3], [0.75, 6.25][i / 3]];
        p.source = q;
        p.target = q;
    }
    Ok(p.freeze())
}
fn empty(b: &MemoryBudget) -> Result<SharedBuffer<ScoredProposal>, ProposalProviderError> {
    Ok(b.try_buffer(
        0,
        ScoredProposal {
            source: [0.; 2],
            target: [0.; 2],
            confidence: 1.,
        },
    )?
    .freeze())
}
fn reduce(work: &mut ProjectedPortfolioWorkLimits, field: usize) {
    let v = match field {
        0 => &mut work.point_checks,
        1 => &mut work.selection_checks,
        2 => &mut work.rank_sites,
        3 => &mut work.rank_pixel_reads,
        4 => &mut work.geometry_hypotheses,
        5 => &mut work.model.model_input_reads,
        6 => &mut work.model.proposal_ranking_checks,
        7 => &mut work.model.proposal_separation_checks,
        8 => &mut work.model.geometry_hypotheses,
        9 => &mut work.model.original_pixel_work,
        10 => &mut work.model.identity_input_checks,
        _ => &mut work.model.identity_point_checks,
    };
    *v -= 1;
}
#[test]
fn native_alpha_refusal_recovers_with_projected_original_support_in_both_orders() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path(), true);
    let b = MemoryBudget::new(8 * 1024 * 1024);
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        let calls = Cell::new(0);
        let e = search_projected_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            combined(order),
            &[1.],
            &b,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                proposals(b)
            },
            || false,
        )
        .unwrap();
        assert!(e.supported());
        assert_eq!(calls.get(), 1);
        assert_eq!(
            e.attempted_stages,
            if order == SearchOrder::NativeFirst { 2 } else { 1 }
        );
        if order == SearchOrder::NativeFirst {
            assert!(matches!(
                e.first_miss,
                Some(ProjectedPortfolioMiss::NativeInconclusive(
                    NativeInconclusive::PixelDomain(NativePixelRefusal::GradientOpacity)
                ))
            ));
        } else {
            assert!(e.first_miss.is_none());
        }
        let ledger = match &e.terminal {
            ProjectedPortfolioStage::Model(e) => e.clone(),
            _ => panic!("model support expected"),
        };
        drop(e);
        assert!(b.used() > 0);
        drop(ledger);
        assert_eq!(b.used(), 0);
    }
}
#[test]
fn alpha_collision_exhausts_projected_and_preserves_typed_native_inconclusive() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path(), true);
    write_alpha(&r[1].path, 255);
    let b = MemoryBudget::new(8 * 1024 * 1024);
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        let calls = Cell::new(0);
        let e = search_projected_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            combined(order),
            &[1.],
            &b,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                proposals(b)
            },
            || false,
        );
        match e {
            Err(ProjectedPortfolioError::InconclusiveNative {
                reason: NativeInconclusive::PixelDomain(NativePixelRefusal::GradientOpacity),
                projected,
            }) => {
                assert_eq!(projected.attempts, 25);
                assert!(projected.pixel_work > 0);
            }
            other => panic!("wrong outcome: {other:?}"),
        }
        assert_eq!(calls.get(), 25);
        assert_eq!(b.used(), 0);
    }
}
#[test]
fn hdr_regions_recover_after_whole_miss_and_exhaust_before_native_negative() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path(), false);
    let b = MemoryBudget::new(8 * 1024 * 1024);
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        let calls = Cell::new(0);
        let e = search_projected_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            combined(order),
            &[1.],
            &b,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                if calls.get() == 7 { proposals(b) } else { empty(b) }
            },
            || false,
        )
        .unwrap();
        assert!(e.supported());
        assert_eq!(calls.get(), 7);
        drop(e);
        assert_eq!(b.used(), 0);
    }
    let calls = Cell::new(0);
    let e = search_projected_portfolio_files_with(
        &r[0],
        &r[1],
        &w,
        combined(SearchOrder::ModelFirst),
        &[1.],
        &b,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            empty(b)
        },
        || false,
    )
    .unwrap();
    assert!(!e.supported());
    assert_eq!(calls.get(), 25);
    assert!(matches!(e.terminal, ProjectedPortfolioStage::Native(_)));
    assert!(matches!(
        e.first_miss,
        Some(ProjectedPortfolioMiss::Model(ProjectedStageMiss {
            attempts: 25,
            ..
        }))
    ));
    drop(e);
    assert_eq!(b.used(), 0);
}
#[test]
fn native_early_support_avoids_inference_and_projected_exhaustion_can_recover_natively() {
    let dir = tempfile::tempdir().unwrap();
    let (_, w) = fixture(dir.path(), false);
    let source =
        DecodeRequest::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));
    let b = MemoryBudget::new(128 * 1024 * 1024);
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        let calls = Cell::new(0);
        let e = search_projected_portfolio_files_with(
            &source,
            &source,
            &w,
            combined(order),
            &factors(),
            &b,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                empty(b)
            },
            || false,
        )
        .unwrap();
        assert!(e.supported());
        assert!(matches!(e.terminal, ProjectedPortfolioStage::Native(_)));
        assert_eq!(
            calls.get(),
            if order == SearchOrder::NativeFirst { 0 } else { 25 }
        );
        drop(e);
        assert_eq!(b.used(), 0);
    }
    std::fs::remove_file(&w).unwrap();
    assert!(matches!(
        search_projected_portfolio_files_with(
            &source,
            &source,
            &w,
            combined(SearchOrder::NativeFirst),
            &factors(),
            &b,
            |_, _, _, _, _| panic!("inference"),
            || false
        ),
        Err(ProjectedPortfolioError::Source(_))
    ));
    assert_eq!(b.used(), 0);
}
#[test]
fn aggregate_and_constituent_work_refuse_before_io_and_cancel_wins() {
    let r = DecodeRequest::new("missing");
    let b = MemoryBudget::new(0);
    for field in 0..12 {
        let mut p = combined(SearchOrder::NativeFirst);
        reduce(&mut p.work, field);
        for cancel in [false, true] {
            let e = search_projected_portfolio_files_with(
                &r,
                &r,
                Path::new("missing"),
                p,
                &[1.],
                &b,
                |_, _, _, _, _| panic!("provider"),
                || cancel,
            );
            if cancel {
                assert!(matches!(e, Err(ProjectedPortfolioError::Cancelled)));
            } else {
                assert!(matches!(e, Err(ProjectedPortfolioError::Work)));
            }
            assert_eq!(b.peak(), 0);
        }
    }
    let mut p = combined(SearchOrder::NativeFirst);
    p.model_work.original_pixel_work -= 1;
    assert!(matches!(
        search_projected_portfolio_files_with(
            &r,
            &r,
            Path::new("missing"),
            p,
            &[1.],
            &b,
            |_, _, _, _, _| panic!(),
            || false
        ),
        Err(ProjectedPortfolioError::Model(_))
    ));
    p = combined(SearchOrder::NativeFirst);
    p.native.search.anchors.local.maximum_point_checks = u64::MAX;
    assert!(matches!(p.required_work(), Err(ProjectedPortfolioError::Work)));
    let mut work = combined(SearchOrder::NativeFirst).work;
    work.geometry_hypotheses = u64::MAX;
    assert!(matches!(work.for_pairs(2), Err(ProjectedPortfolioError::Work)));
}
#[test]
fn provider_and_source_weight_failures_after_native_inconclusive_remain_terminal() {
    for kind in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let (r, w) = fixture(dir.path(), true);
        let b = MemoryBudget::new(8 * 1024 * 1024);
        let calls = Cell::new(0);
        let e = search_projected_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            combined(SearchOrder::NativeFirst),
            &[1.],
            &b,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                if kind == 0 {
                    return Err("provider failed".into());
                }
                let path = if kind == 1 { &r[1].path } else { &w };
                let mut bytes = std::fs::read(path).unwrap();
                bytes.push(0);
                std::fs::write(path, bytes).unwrap();
                proposals(b)
            },
            || false,
        );
        assert!(matches!(e, Err(ProjectedPortfolioError::Model(_))), "{e:?}");
        assert_eq!(calls.get(), 1);
        assert_eq!(b.used(), 0);
    }
}
#[test]
fn final_callback_source_and_weight_mutations_precede_support_and_inconclusive() {
    for collision in [false, true] {
        for target in 0..3 {
            let dir = tempfile::tempdir().unwrap();
            let (r, w) = fixture(dir.path(), true);
            if collision {
                write_alpha(&r[1].path, 255);
            }
            let b = MemoryBudget::new(8 * 1024 * 1024);
            let calls = Cell::new(0);
            let baseline = search_projected_portfolio_files_with(
                &r[0],
                &r[1],
                &w,
                combined(SearchOrder::NativeFirst),
                &[1.],
                &b,
                |_, _, _, b, _| proposals(b),
                || {
                    calls.set(calls.get() + 1);
                    false
                },
            );
            if collision {
                assert!(matches!(
                    baseline,
                    Err(ProjectedPortfolioError::InconclusiveNative { .. })
                ));
            } else {
                assert!(baseline.unwrap().supported());
            }
            assert_eq!(b.used(), 0);
            let last = calls.replace(0);
            let e = search_projected_portfolio_files_with(
                &r[0],
                &r[1],
                &w,
                combined(SearchOrder::NativeFirst),
                &[1.],
                &b,
                |_, _, _, b, _| proposals(b),
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == last {
                        let path = if target == 2 { &w } else { &r[target].path };
                        let mut bytes = std::fs::read(path).unwrap();
                        bytes.push(0);
                        std::fs::write(path, bytes).unwrap();
                    }
                    false
                },
            );
            assert!(matches!(e, Err(ProjectedPortfolioError::Source(_))), "{e:?}");
            assert_eq!(calls.get(), last);
            assert_eq!(b.used(), 0);
        }
    }
}
#[test]
fn global_and_both_owner_generations_cancel_first_middle_and_last_checkpoints() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    for owner in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let (mut r, w) = fixture(dir.path(), true);
        let generation = Arc::new(AtomicU64::new(1));
        if owner < 2 {
            r[owner].cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
        }
        let b = MemoryBudget::new(8 * 1024 * 1024);
        let calls = Cell::new(0);
        drop(
            search_projected_portfolio_files_with(
                &r[0],
                &r[1],
                &w,
                combined(SearchOrder::NativeFirst),
                &[1.],
                &b,
                |_, _, _, b, _| proposals(b),
                || {
                    calls.set(calls.get() + 1);
                    false
                },
            )
            .unwrap(),
        );
        let last = calls.get();
        for trigger in [1, last / 2, last] {
            calls.set(0);
            generation.store(1, Ordering::Release);
            let e = search_projected_portfolio_files_with(
                &r[0],
                &r[1],
                &w,
                combined(SearchOrder::NativeFirst),
                &[1.],
                &b,
                |_, _, _, b, _| proposals(b),
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == trigger {
                        if owner < 2 {
                            generation.store(2, Ordering::Release);
                        }
                        return owner == 2;
                    }
                    if calls.get() > trigger {
                        generation.store(1, Ordering::Release);
                    }
                    false
                },
            );
            assert!(matches!(e, Err(ProjectedPortfolioError::Cancelled)), "{e:?}");
            assert_eq!(calls.get(), trigger);
            assert_eq!(b.used(), 0);
        }
    }
}
fn collection_policy(order: SearchOrder) -> ProjectedPortfolioCollectionPolicy {
    let search = combined(order);
    ProjectedPortfolioCollectionPolicy {
        search,
        max_files: 3,
        max_pairs: 3,
        work: search.work.for_pairs(3).unwrap(),
    }
}
fn collection_fixture(dir: &Path) -> (Vec<(u64, DecodeRequest)>, PathBuf) {
    let (r, w) = fixture(dir, true);
    let path = dir.join("third.qoi");
    write_alpha(&path, 255);
    (
        vec![
            (9, DecodeRequest::new(path)),
            (3, r[1].clone()),
            (1, r[0].clone()),
        ],
        w,
    )
}
#[test]
fn collection_retains_supported_and_explicit_unknown_pairs_in_both_orders() {
    let dir = tempfile::tempdir().unwrap();
    let (files, w) = collection_fixture(dir.path());
    let b = MemoryBudget::new(8 * 1024 * 1024);
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        let calls = Cell::new(0);
        let e = scan_projected_portfolio_collection_with(
            files.clone(),
            &w,
            collection_policy(order),
            &[1.],
            &b,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                proposals(b)
            },
            || false,
        )
        .unwrap();
        assert_eq!(e.file_ids(), &[1, 3, 9]);
        assert_eq!(
            e.pairs().iter().map(|p| (p.left, p.right)).collect::<Vec<_>>(),
            [(1, 3), (1, 9), (3, 9)]
        );
        assert_eq!(
            e.pairs().iter().map(|p| p.evidence.support()).collect::<Vec<_>>(),
            [Some(true), None, None]
        );
        assert_eq!(calls.get(), 51);
        for pair in &e.pairs()[1..] {
            assert!(matches!(
                pair.evidence,
                ProjectedPortfolioPairOutcome::Inconclusive {
                    reason: NativeInconclusive::PixelDomain(NativePixelRefusal::GradientOpacity),
                    projected: ProjectedStageMiss { attempts: 25, .. },
                    ..
                }
            ));
        }
        drop(e);
        assert_eq!(b.used(), 0);
    }
}
#[test]
fn collection_unknown_does_not_swallow_late_source_or_provider_failure() {
    for kind in 0..2 {
        let dir = tempfile::tempdir().unwrap();
        let (files, w) = collection_fixture(dir.path());
        let third = files[0].1.path.clone();
        let b = MemoryBudget::new(8 * 1024 * 1024);
        let calls = Cell::new(0);
        let e = scan_projected_portfolio_collection_with(
            files,
            &w,
            collection_policy(SearchOrder::NativeFirst),
            &[1.],
            &b,
            |_, _, _, b, _| {
                calls.set(calls.get() + 1);
                if kind == 0 && calls.get() == 1 {
                    write_alpha(&third, 32);
                }
                if kind == 1 && calls.get() == 2 {
                    return Err("provider failed".into());
                }
                proposals(b)
            },
            || false,
        );
        if kind == 0 {
            assert!(matches!(
                e,
                Err(ProjectedPortfolioCollectionError::Source { id: 9, .. })
            ));
        } else {
            assert!(matches!(
                e,
                Err(ProjectedPortfolioCollectionError::Pair {
                    left: 1,
                    right: 9,
                    ..
                })
            ));
        }
        assert_eq!(calls.get(), if kind == 0 { 1 } else { 2 });
        assert_eq!(b.used(), 0);
    }
}
#[test]
fn collection_work_admission_precedes_iterator_and_unknown_reports_keep_final_source_guards() {
    let b = MemoryBudget::new(8 * 1024 * 1024);
    for field in 0..12 {
        let mut p = collection_policy(SearchOrder::NativeFirst);
        reduce(&mut p.work, field);
        assert!(matches!(
            scan_projected_portfolio_collection_with(
                std::iter::from_fn(|| panic!("iterator")),
                Path::new("missing"),
                p,
                &[1.],
                &b,
                |_, _, _, _, _| panic!("provider"),
                || false
            ),
            Err(ProjectedPortfolioCollectionError::Work)
        ));
        assert_eq!(b.peak(), 0);
    }
    let dir = tempfile::tempdir().unwrap();
    let (files, w) = collection_fixture(dir.path());
    let third = files[0].1.path.clone();
    let calls = Cell::new(0);
    drop(
        scan_projected_portfolio_collection_with(
            files.clone(),
            &w,
            collection_policy(SearchOrder::NativeFirst),
            &[1.],
            &b,
            |_, _, _, b, _| proposals(b),
            || {
                calls.set(calls.get() + 1);
                false
            },
        )
        .unwrap(),
    );
    let last = calls.replace(0);
    let e = scan_projected_portfolio_collection_with(
        files,
        &w,
        collection_policy(SearchOrder::NativeFirst),
        &[1.],
        &b,
        |_, _, _, b, _| proposals(b),
        || {
            calls.set(calls.get() + 1);
            if calls.get() == last {
                write_alpha(&third, 32);
            }
            false
        },
    );
    assert!(matches!(
        e,
        Err(ProjectedPortfolioCollectionError::Source { id: 9, .. })
    ));
    assert_eq!(calls.get(), last);
    assert_eq!(b.used(), 0);
}

#[test]
fn native_anchor_inconclusive_recovers_or_stays_inconclusive_after_projected_exhaustion() {
    let dir = tempfile::tempdir().unwrap();
    let (_, weights) = fixture(dir.path(), false);
    let source =
        DecodeRequest::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation/base.png"));
    let budget = MemoryBudget::new(128 * 1024 * 1024);
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        let mut policy = combined(order);
        policy.native.search.anchors.local.minimum_witnesses = 1000;
        policy.work = policy.required_work().unwrap();
        for supported in [false, true] {
            let calls = Cell::new(0);
            let result = search_projected_portfolio_files_with(
                &source,
                &source,
                &weights,
                policy,
                &factors(),
                &budget,
                |_, _, _, b, _| {
                    calls.set(calls.get() + 1);
                    if supported { proposals(b) } else { empty(b) }
                },
                || false,
            );
            if supported {
                let e = result.unwrap();
                assert!(e.supported());
                if order == SearchOrder::NativeFirst {
                    assert!(matches!(
                        e.first_miss,
                        Some(ProjectedPortfolioMiss::NativeInconclusive(
                            NativeInconclusive::InsufficientAnchorGeometry
                        ))
                    ));
                }
                assert_eq!(calls.get(), 1);
                drop(e);
            } else {
                assert!(
                    matches!(
                        result,
                        Err(ProjectedPortfolioError::InconclusiveNative {
                            reason: NativeInconclusive::InsufficientAnchorGeometry,
                            projected: ProjectedStageMiss { attempts: 25, .. }
                        })
                    ),
                    "{result:?}"
                );
                assert_eq!(calls.get(), 25);
            }
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
fn external_palette_change_discards_projected_portfolio_collection_in_both_orders() {
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        for remove in [false, true] {
            for final_checkpoint in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let (mut files, weights) = collection_fixture(dir.path());
                std::fs::create_dir(dir.path().join("textures")).unwrap();
                std::fs::create_dir(dir.path().join("pics")).unwrap();
                let wal = dir.path().join("textures/third.wal");
                let palette = dir.path().join("pics/colormap.pcx");
                std::fs::write(
                    &wal,
                    include_bytes!("../../../tests/fixtures/raster/palette-grid.wal"),
                )
                .unwrap();
                std::fs::write(
                    &palette,
                    include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx"),
                )
                .unwrap();
                files[0].1 = DecodeRequest::new(wal);
                let policy = collection_policy(order);
                let budget = MemoryBudget::new(128 * 1024 * 1024);
                let callbacks = Cell::new(0usize);
                let providers = Cell::new(0usize);
                let baseline = scan_projected_portfolio_collection_with(
                    files.clone(),
                    &weights,
                    policy,
                    &[1.],
                    &budget,
                    |_, _, _, b, _| {
                        providers.set(providers.get() + 1);
                        if providers.get() == 1 {
                            proposals(b)
                        } else {
                            empty(b)
                        }
                    },
                    || {
                        callbacks.set(callbacks.get() + 1);
                        false
                    },
                )
                .unwrap();
                assert_eq!(baseline.pairs()[0].evidence.support(), Some(true));
                assert_eq!(baseline.pairs().len(), 3);
                assert_eq!(providers.get(), 51);
                let last = callbacks.replace(0);
                providers.set(0);
                drop(baseline);
                assert_eq!(budget.used(), 0);
                let mutate = || {
                    if remove {
                        std::fs::remove_file(&palette).unwrap();
                    } else {
                        let mut bytes = std::fs::read(&palette).unwrap();
                        bytes.push(0);
                        std::fs::write(&palette, bytes).unwrap();
                    }
                };
                let result = scan_projected_portfolio_collection_with(
                    files,
                    &weights,
                    policy,
                    &[1.],
                    &budget,
                    |_, _, _, b, _| {
                        providers.set(providers.get() + 1);
                        let result = if providers.get() == 1 {
                            proposals(b)
                        } else {
                            empty(b)
                        };
                        if !final_checkpoint && providers.get() == 1 {
                            mutate();
                        }
                        result
                    },
                    || {
                        callbacks.set(callbacks.get() + 1);
                        if final_checkpoint && callbacks.get() == last {
                            mutate();
                        }
                        false
                    },
                );
                assert!(
                    matches!(
                        result,
                        Err(ProjectedPortfolioCollectionError::Source { id: 9, .. })
                    ),
                    "{order:?} remove={remove} final={final_checkpoint}: {result:?}"
                );
                assert_eq!(providers.get(), if final_checkpoint { 51 } else { 1 });
                if final_checkpoint {
                    assert_eq!(callbacks.get(), last);
                }
                assert_eq!(budget.used(), 0);
            }
        }
    }
}

use rrrah_dedup::exact_portfolio::{ExactPortfolioEvidence, search_exact_projected_portfolio_files_with};

#[test]
fn exact_first_accepts_undecodable_copies_without_weights_or_provider() {
    let dir = tempfile::tempdir().unwrap();
    let (r, w) = fixture(dir.path(), true);
    for request in &r {
        std::fs::write(&request.path, b"not an image").unwrap();
    }
    std::fs::remove_file(&w).unwrap();
    let b = MemoryBudget::new(0);
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        let evidence = search_exact_projected_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            combined(order),
            &[1.],
            &b,
            |_, _, _, _, _| panic!("exact bytes must not invoke inference"),
            || false,
        )
        .unwrap();
        assert!(matches!(evidence, ExactPortfolioEvidence::ExactBytes));
        assert_eq!(b.used(), 0);
    }
    let mut p = combined(SearchOrder::NativeFirst);
    p.work.point_checks = 0;
    assert!(matches!(
        search_exact_projected_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            p,
            &[1.],
            &b,
            |_, _, _, _, _| panic!("preflight must precede IO"),
            || false,
        ),
        Err(ProjectedPortfolioError::Work)
    ));
}

#[test]
fn exact_first_mismatch_runs_visual_and_keeps_projection_collision_inconclusive() {
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        for collision in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let (r, w) = fixture(dir.path(), true);
            if collision {
                write_alpha(&r[1].path, 128);
            } else {
                // A different QOI colorspace byte, with identical black RGBA pixels.
                let mut bytes = std::fs::read(&r[1].path).unwrap();
                bytes[13] = 0;
                std::fs::write(&r[1].path, bytes).unwrap();
            }
            let b = MemoryBudget::new(8 * 1024 * 1024);
            let calls = Cell::new(0);
            let result = search_exact_projected_portfolio_files_with(
                &r[0],
                &r[1],
                &w,
                combined(order),
                &[1.],
                &b,
                |_, _, _, b, _| {
                    calls.set(calls.get() + 1);
                    proposals(b)
                },
                || false,
            );
            if collision {
                assert!(
                    matches!(result, Err(ProjectedPortfolioError::InconclusiveNative { .. })),
                    "{result:?}"
                );
            } else {
                assert!(
                    matches!(result, Ok(ExactPortfolioEvidence::Visual(ref e)) if e.supported()),
                    "{result:?}"
                );
            }
            assert!(calls.get() > 0);
            drop(result);
            assert_eq!(b.used(), 0);
        }
    }
}

#[test]
fn exact_first_last_callback_revalidates_sources_and_visual_weights() {
    for visual in [false, true] {
        for target in 0..if visual { 3 } else { 2 } {
            let dir = tempfile::tempdir().unwrap();
            let (r, w) = fixture(dir.path(), true);
            if visual {
                let mut bytes = std::fs::read(&r[1].path).unwrap();
                bytes[13] = 0;
                std::fs::write(&r[1].path, bytes).unwrap();
            }
            let b = MemoryBudget::new(8 * 1024 * 1024);
            let calls = Cell::new(0);
            search_exact_projected_portfolio_files_with(
                &r[0],
                &r[1],
                &w,
                combined(SearchOrder::ModelFirst),
                &[1.],
                &b,
                |_, _, _, b, _| proposals(b),
                || {
                    calls.set(calls.get() + 1);
                    false
                },
            )
            .unwrap();
            let last = calls.get();
            calls.set(0);
            let result = search_exact_projected_portfolio_files_with(
                &r[0],
                &r[1],
                &w,
                combined(SearchOrder::ModelFirst),
                &[1.],
                &b,
                |_, _, _, b, _| proposals(b),
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == last {
                        let path = if target == 2 { &w } else { &r[target].path };
                        let mut bytes = std::fs::read(path).unwrap();
                        bytes.push(0);
                        std::fs::write(path, bytes).unwrap();
                    }
                    false
                },
            );
            assert!(
                matches!(result, Err(ProjectedPortfolioError::Source(_))),
                "{result:?}"
            );
            assert_eq!(b.used(), 0);
        }
    }
}

#[test]
fn exact_first_sticky_global_and_both_owners_cancel_every_exact_checkpoint() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    for owner in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let (mut r, w) = fixture(dir.path(), true);
        let generation = Arc::new(AtomicU64::new(1));
        if owner < 2 {
            r[owner].cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
        }
        let b = MemoryBudget::new(0);
        let calls = Cell::new(0);
        search_exact_projected_portfolio_files_with(
            &r[0],
            &r[1],
            &w,
            combined(SearchOrder::NativeFirst),
            &[1.],
            &b,
            |_, _, _, _, _| panic!("no inference"),
            || {
                calls.set(calls.get() + 1);
                false
            },
        )
        .unwrap();
        for stop in 1..=calls.get() {
            generation.store(1, Ordering::Release);
            let current = Cell::new(0);
            let result = search_exact_projected_portfolio_files_with(
                &r[0],
                &r[1],
                &w,
                combined(SearchOrder::NativeFirst),
                &[1.],
                &b,
                |_, _, _, _, _| panic!("no inference"),
                || {
                    current.set(current.get() + 1);
                    if current.get() == stop && owner < 2 {
                        generation.store(2, Ordering::Release);
                    }
                    owner == 2 && current.get() == stop
                },
            );
            assert!(
                matches!(result, Err(ProjectedPortfolioError::Cancelled)),
                "owner={owner} stop={stop}: {result:?}"
            );
            assert_eq!(b.used(), 0);
        }
    }
}

use rrrah_dedup::exact_portfolio_collection::*;
fn exact_collection_policy(order: SearchOrder) -> ExactPortfolioCollectionPolicy {
    let p = collection_policy(order);
    ExactPortfolioCollectionPolicy {
        search: p.search,
        max_files: p.max_files,
        max_pairs: p.max_pairs,
        work: p.work,
    }
}
#[test]
fn exact_collection_missing_sources_never_publish_partial_duplicates() {
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        for missing in 0..3 {
            let dir = tempfile::tempdir().unwrap();
            let (files, weights) = collection_fixture(dir.path());
            for (_, request) in &files {
                std::fs::write(&request.path, b"same opaque bytes").unwrap();
            }
            std::fs::remove_file(&weights).unwrap();
            std::fs::remove_file(&files[missing].1.path).unwrap();
            let budget = MemoryBudget::new(8 * 1024 * 1024);
            for reverse in [false, true] {
                let mut input = files.clone();
                if reverse {
                    input.reverse();
                }
                let result = scan_exact_projected_portfolio_collection_with(
                    input,
                    &weights,
                    exact_collection_policy(order),
                    &[1.],
                    &budget,
                    |_, _, _, _, _| panic!("missing source must not invoke inference"),
                    || false,
                );
                assert!(
                    matches!(result, Err(ExactPortfolioCollectionError::Source { id, .. })
                        if id == files[missing].0),
                    "{order:?} missing={missing} reverse={reverse}: {result:?}"
                );
                assert_eq!(budget.used(), 0);
            }
            std::fs::write(&files[missing].1.path, b"same opaque bytes").unwrap();
            let report = scan_exact_projected_portfolio_collection_with(
                files,
                &weights,
                exact_collection_policy(order),
                &[1.],
                &budget,
                |_, _, _, _, _| panic!("recovered exact copies must skip inference"),
                || false,
            )
            .unwrap();
            assert_eq!(report.pairs().len(), 3);
            assert!(
                report
                    .pairs()
                    .iter()
                    .all(|pair| matches!(pair.evidence, ExactPortfolioPairOutcome::ExactBytes))
            );
            drop(report);
            assert_eq!(budget.used(), 0);
        }
    }
}

#[test]
fn exact_collections_empty_singleton_and_copies_need_no_decoder_weights_or_provider() {
    for n in 0..=3 {
        let dir = tempfile::tempdir().unwrap();
        let (mut files, w) = collection_fixture(dir.path());
        for (_, r) in &files {
            std::fs::write(&r.path, b"undecodable exact copies").unwrap();
        }
        std::fs::remove_file(&w).unwrap();
        files.truncate(n);
        let b = MemoryBudget::new(8 * 1024 * 1024);
        let e = scan_exact_projected_portfolio_collection_with(
            files,
            &w,
            exact_collection_policy(SearchOrder::NativeFirst),
            &[1.],
            &b,
            |_, _, _, _, _| panic!("exact collection must skip inference"),
            || false,
        )
        .unwrap();
        assert_eq!(e.file_ids().len(), n);
        assert!(e.file_ids().windows(2).all(|p| p[0] < p[1]));
        assert_eq!(e.pairs().len(), n * n.saturating_sub(1) / 2);
        assert!(
            e.pairs()
                .iter()
                .all(|p| matches!(p.evidence, ExactPortfolioPairOutcome::ExactBytes))
        );
        drop(e);
        assert_eq!(b.used(), 0);
    }
}
#[test]
fn exact_collection_preserves_exact_visual_and_unknown_evidence_in_both_orders() {
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        for visual_copy in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let (files, w) = collection_fixture(dir.path());
            if visual_copy {
                let mut bytes = std::fs::read(&files[1].1.path).unwrap();
                bytes[13] = 0;
                std::fs::write(&files[1].1.path, bytes).unwrap();
            }
            let b = MemoryBudget::new(8 * 1024 * 1024);
            let e = scan_exact_projected_portfolio_collection_with(
                files,
                &w,
                exact_collection_policy(order),
                &[1.],
                &b,
                |_, _, _, b, _| proposals(b),
                || false,
            )
            .unwrap();
            assert_eq!(e.file_ids(), [1, 3, 9]);
            assert_eq!(
                e.pairs().iter().map(|p| p.evidence.support()).collect::<Vec<_>>(),
                [Some(true), None, None]
            );
            assert_eq!(
                matches!(e.pairs()[0].evidence, ExactPortfolioPairOutcome::ExactBytes),
                !visual_copy
            );
            if visual_copy {
                assert!(matches!(
                    e.pairs()[0].evidence,
                    ExactPortfolioPairOutcome::Complete(_)
                ));
            }
            drop(e);
            assert_eq!(b.used(), 0);
        }
    }
}
#[test]
fn exact_collection_admission_refuses_before_iterator_or_model_io() {
    let p = exact_collection_policy(SearchOrder::NativeFirst);
    let b = MemoryBudget::new(8 * 1024 * 1024);
    let absent = Path::new("absent-model");
    let never = || {
        std::iter::from_fn(|| -> Option<(u64, DecodeRequest)> { panic!("preflight must precede iteration") })
    };
    let mut bad = p;
    bad.work.point_checks = 0;
    assert!(matches!(
        scan_exact_projected_portfolio_collection_with(
            never(),
            absent,
            bad,
            &[1.],
            &b,
            |_, _, _, _, _| panic!(),
            || false
        ),
        Err(ExactPortfolioCollectionError::Work)
    ));
    assert!(matches!(
        scan_exact_projected_portfolio_collection_with(
            never(),
            absent,
            p,
            &[1.],
            &MemoryBudget::new(0),
            |_, _, _, _, _| panic!(),
            || false
        ),
        Err(ExactPortfolioCollectionError::Memory(_))
    ));
    assert!(matches!(
        scan_exact_projected_portfolio_collection_with(
            never(),
            absent,
            p,
            &[1.],
            &b,
            |_, _, _, _, _| panic!(),
            || true
        ),
        Err(ExactPortfolioCollectionError::Cancelled)
    ));
    for kind in 0..3 {
        let mut p = p;
        let files: Vec<_> = (0..if kind == 0 { 4 } else { 3 })
            .map(|id| {
                (
                    if kind == 1 { 0 } else { id },
                    DecodeRequest::new("absent-source"),
                )
            })
            .collect();
        if kind == 2 {
            p.max_pairs = 2;
            p.work = p.search.work.for_pairs(2).unwrap();
        }
        let e = scan_exact_projected_portfolio_collection_with(
            files,
            absent,
            p,
            &[1.],
            &b,
            |_, _, _, _, _| panic!(),
            || false,
        );
        assert!(match kind {
            0 => matches!(e, Err(ExactPortfolioCollectionError::Files)),
            1 => matches!(e, Err(ExactPortfolioCollectionError::DuplicateId(0))),
            _ => matches!(e, Err(ExactPortfolioCollectionError::Pairs)),
        });
        assert_eq!(b.used(), 0);
    }
}
#[test]
fn exact_collection_final_callback_revalidates_every_file_and_started_weights() {
    for visual in [false, true] {
        for target in 0..if visual { 4 } else { 3 } {
            let dir = tempfile::tempdir().unwrap();
            let (files, w) = collection_fixture(dir.path());
            if !visual {
                for (_, r) in &files {
                    std::fs::write(&r.path, b"copies").unwrap();
                }
            }
            let b = MemoryBudget::new(8 * 1024 * 1024);
            let callbacks = Cell::new(0);
            let e = scan_exact_projected_portfolio_collection_with(
                files.clone(),
                &w,
                exact_collection_policy(SearchOrder::ModelFirst),
                &[1.],
                &b,
                |_, _, _, b, _| proposals(b),
                || {
                    callbacks.set(callbacks.get() + 1);
                    false
                },
            )
            .unwrap();
            let last = callbacks.replace(0);
            drop(e);
            let e = scan_exact_projected_portfolio_collection_with(
                files.clone(),
                &w,
                exact_collection_policy(SearchOrder::ModelFirst),
                &[1.],
                &b,
                |_, _, _, b, _| proposals(b),
                || {
                    callbacks.set(callbacks.get() + 1);
                    if callbacks.get() == last {
                        let path = if target == 3 { &w } else { &files[target].1.path };
                        let mut bytes = std::fs::read(path).unwrap();
                        bytes.push(0);
                        std::fs::write(path, bytes).unwrap();
                    }
                    false
                },
            );
            assert!(
                matches!(
                    e,
                    Err(ExactPortfolioCollectionError::Source { .. }
                        | ExactPortfolioCollectionError::ModelSource(_))
                ),
                "{e:?}"
            );
            assert_eq!(callbacks.get(), last);
            assert_eq!(b.used(), 0);
        }
    }
}

#[test]
fn exact_collection_external_palette_change_discards_report_in_both_orders() {
    for order in [SearchOrder::NativeFirst, SearchOrder::ModelFirst] {
        for remove in [false, true] {
            for final_checkpoint in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let (mut files, weights) = collection_fixture(dir.path());
                std::fs::create_dir(dir.path().join("textures")).unwrap();
                std::fs::create_dir(dir.path().join("pics")).unwrap();
                let wal = dir.path().join("textures/third.wal");
                let palette = dir.path().join("pics/colormap.pcx");
                std::fs::write(
                    &wal,
                    include_bytes!("../../../tests/fixtures/raster/palette-grid.wal"),
                )
                .unwrap();
                std::fs::write(
                    &palette,
                    include_bytes!("../../../tests/fixtures/raster/wal-colormap.pcx"),
                )
                .unwrap();
                files[0].1 = DecodeRequest::new(wal);
                // First pair is a byte-different visual copy; WAL id 9 is idle.
                let mut bytes = std::fs::read(&files[1].1.path).unwrap();
                bytes[13] = 0;
                std::fs::write(&files[1].1.path, bytes).unwrap();
                let policy = exact_collection_policy(order);
                let budget = MemoryBudget::new(128 * 1024 * 1024);
                let callbacks = Cell::new(0usize);
                let providers = Cell::new(0usize);
                let baseline = scan_exact_projected_portfolio_collection_with(
                    files.clone(),
                    &weights,
                    policy,
                    &[1.],
                    &budget,
                    |_, _, _, b, _| {
                        providers.set(providers.get() + 1);
                        if providers.get() == 1 {
                            proposals(b)
                        } else {
                            empty(b)
                        }
                    },
                    || {
                        callbacks.set(callbacks.get() + 1);
                        false
                    },
                )
                .unwrap();
                assert_eq!(baseline.pairs()[0].evidence.support(), Some(true));
                assert_eq!(baseline.pairs().len(), 3);
                assert_eq!(providers.get(), 51);
                let last = callbacks.replace(0);
                providers.set(0);
                drop(baseline);
                assert_eq!(budget.used(), 0);
                let mutate = || {
                    if remove {
                        std::fs::remove_file(&palette).unwrap();
                    } else {
                        let mut bytes = std::fs::read(&palette).unwrap();
                        bytes.push(0);
                        std::fs::write(&palette, bytes).unwrap();
                    }
                };
                let result = scan_exact_projected_portfolio_collection_with(
                    files,
                    &weights,
                    policy,
                    &[1.],
                    &budget,
                    |_, _, _, b, _| {
                        providers.set(providers.get() + 1);
                        let result = if providers.get() == 1 {
                            proposals(b)
                        } else {
                            empty(b)
                        };
                        if !final_checkpoint && providers.get() == 1 {
                            mutate();
                        }
                        result
                    },
                    || {
                        callbacks.set(callbacks.get() + 1);
                        if final_checkpoint && callbacks.get() == last {
                            mutate();
                        }
                        false
                    },
                );
                assert!(
                    matches!(result, Err(ExactPortfolioCollectionError::Source { id: 9, .. })),
                    "{order:?} remove={remove} final={final_checkpoint}: {result:?}"
                );
                assert_eq!(providers.get(), if final_checkpoint { 51 } else { 1 });
                if final_checkpoint {
                    assert_eq!(callbacks.get(), last);
                }
                assert_eq!(budget.used(), 0);
            }
        }
    }
}

#[test]
fn exact_collection_global_and_every_owner_cancel_without_partial_report() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    for owner in 0..4 {
        let dir = tempfile::tempdir().unwrap();
        let (mut files, w) = collection_fixture(dir.path());
        for (_, r) in &files {
            std::fs::write(&r.path, b"copies").unwrap();
        }
        let generation = Arc::new(AtomicU64::new(1));
        if owner < 3 {
            files[owner].1.cancellation = Some(rrrah_decode::GenerationToken::new(generation.clone(), 1));
        }
        let b = MemoryBudget::new(8 * 1024 * 1024);
        let callbacks = Cell::new(0);
        let e = scan_exact_projected_portfolio_collection_with(
            files.clone(),
            &w,
            exact_collection_policy(SearchOrder::NativeFirst),
            &[1.],
            &b,
            |_, _, _, _, _| panic!(),
            || {
                callbacks.set(callbacks.get() + 1);
                false
            },
        )
        .unwrap();
        drop(e);
        for stop in [1, callbacks.get() / 2, callbacks.get()] {
            generation.store(1, Ordering::Release);
            let current = Cell::new(0);
            let e = scan_exact_projected_portfolio_collection_with(
                files.clone(),
                &w,
                exact_collection_policy(SearchOrder::NativeFirst),
                &[1.],
                &b,
                |_, _, _, _, _| panic!(),
                || {
                    current.set(current.get() + 1);
                    if current.get() == stop && owner < 3 {
                        generation.store(2, Ordering::Release);
                    }
                    owner == 3 && current.get() == stop
                },
            );
            assert!(
                matches!(e, Err(ExactPortfolioCollectionError::Cancelled)),
                "owner={owner} stop={stop}: {e:?}"
            );
            assert_eq!(b.used(), 0);
        }
    }
}

#[test]
fn exact_collection_late_provider_failure_discards_prior_exact_and_unknown_pairs() {
    let dir = tempfile::tempdir().unwrap();
    let (files, w) = collection_fixture(dir.path());
    let b = MemoryBudget::new(8 * 1024 * 1024);
    let calls = Cell::new(0);
    let e = scan_exact_projected_portfolio_collection_with(
        files,
        &w,
        exact_collection_policy(SearchOrder::ModelFirst),
        &[1.],
        &b,
        |_, _, _, b, _| {
            calls.set(calls.get() + 1);
            if calls.get() == 26 {
                return Err("late provider failure".into());
            }
            proposals(b)
        },
        || false,
    );
    assert!(
        matches!(
            e,
            Err(ExactPortfolioCollectionError::Pair {
                left: 3,
                right: 9,
                ..
            })
        ),
        "{e:?}"
    );
    assert_eq!(calls.get(), 26);
    assert_eq!(b.used(), 0);
}

#[test]
#[ignore = "bounded collection I/O scaling measurement; run explicitly"]
fn exact_collection_scaling_latency_probe() {
    use std::time::Instant;
    const SIZE: usize = 1024 * 1024;
    for n in [8usize, 32, 64] {
        for families in [1usize] {
            let dir = tempfile::tempdir().unwrap();
            let mut payload = vec![0u8; SIZE];
            let mut state = 173u32;
            for byte in &mut payload {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                *byte = (state >> 24) as u8;
            }
            let files: Vec<_> = (0..n)
                .map(|id| {
                    let path = dir.path().join(format!("{id}.bytes"));
                    payload[..8].copy_from_slice(&(id as u64 % families as u64).to_le_bytes());
                    std::fs::write(&path, &payload).unwrap();
                    (id as u64, DecodeRequest::new(path))
                })
                .collect();
            let pairs = n * (n - 1) / 2;
            let mut p = exact_collection_policy(SearchOrder::NativeFirst);
            p.max_files = n;
            p.max_pairs = pairs;
            p.work = p.search.work.for_pairs(pairs).unwrap();
            let b = MemoryBudget::new(256 * 1024 * 1024);
            let weights = dir.path().join("weights");
            assert!(!weights.exists());
            let mut samples = Vec::new();
            let mut exact_pairs = 0;
            for _ in 0..3 {
                let start = Instant::now();
                let result = scan_exact_projected_portfolio_collection_with(
                    files.clone(),
                    &weights,
                    p,
                    &[1.],
                    &b,
                    |_, _, _, _, _| panic!("exact copies must never reach inference"),
                    || false,
                );
                let report = result.unwrap();
                assert_eq!(report.pairs().len(), pairs);
                exact_pairs = report
                    .pairs()
                    .iter()
                    .filter(|p| matches!(p.evidence, ExactPortfolioPairOutcome::ExactBytes))
                    .count();
                assert_eq!(exact_pairs, pairs);
                drop(report);
                assert_eq!(b.used(), 0);
                samples.push(start.elapsed().as_nanos());
            }
            if families == 1 {
                // Analytical payload bytes for this all-copy source branch:
                // initial/final full observations plus per-pair verification and
                // streaming byte confirmation. This is not measured device I/O.
                let expected_source_bytes = (4 * n + 6 * pairs) as u64 * SIZE as u64;
                println!(
                    "RRRAH_EXACT_SCALING {{\"files\":{n},\"families\":{families},\"pairs\":{pairs},\"exact_pairs\":{exact_pairs},\"file_bytes\":{SIZE},\"analytical_source_bytes\":{expected_source_bytes},\"elapsed_ns\":{samples:?},\"managed_peak\":{}}}",
                    b.peak()
                );
            }
        }
    }
}
