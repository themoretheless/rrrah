//! Real native LoFTR provider for the unified library lifecycle diagnostic.
//! Tensor allocations remain outside admission; cancellation is cooperative between kernels.
#[path = "../native_policy.rs"]
mod native_policy;
use rrrah_dedup::{
    anchor_rank::AnchorRankPolicy,
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    local_rank::LocalRankPolicy,
    model_compare::ModelComparisonPolicy,
    model_input::ModelInputPolicy,
    model_loftr::search_loftr_portfolio_files,
    proposals::ProposalPolicy,
    rank_region::RankRegionPolicy,
    search_portfolio::{PortfolioWorkLimits, SearchOrder, SearchPortfolioPolicy, SearchStageEvidence},
};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 6 {
        return Err(
            "usage: compare_files source target weights max_side source_tolerance native-first|model-first"
                .into(),
        );
    }
    tch::set_num_threads(2);
    tch::manual_seed(173);
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let policy = ModelComparisonPolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 12800000,
            max_file_bytes: 5242880,
        },
        input: ModelInputPolicy {
            max_side: args[3].parse()?,
            max_source_pixels: 12800000,
            max_output_pixels: 409600,
            max_pixel_reads: 51200000,
        },
        proposals: ProposalPolicy {
            max_proposals: 5000,
            max_ranking_checks: 200000,
            max_separation_checks: 12497500,
            minimum_separation: 2.,
        },
        geometry: GeometryPolicy {
            tolerance: 2.,
            min_inliers: 10,
            max_points: 5000,
            max_hypotheses: 20000,
        },
        sampling: ProjectiveSamplingPolicy {
            trials: 20000,
            seed: 173,
        },
        anchors: AnchorRankPolicy {
            local: LocalRankPolicy {
                rank: RankRegionPolicy {
                    radius: 8,
                    minimum_contrast: 0.005,
                    minimum_pairs: 1000,
                    maximum_sites: 80000000,
                    maximum_pixel_reads: 400000000,
                },
                filter_radius: 8,
                minimum_witnesses: 10,
                maximum_points: 28000,
                maximum_point_checks: 392014000,
                minimum_point_separation: 2.,
                target_tolerance: 2.,
                source_tolerance: args[4].parse()?,
                minimum_coverage: 0.3,
                minimum_agreement: 0.9,
            },
            window_radius: 2,
            maximum_selection_checks: 1176014000,
        },
        max_model_bytes: 128 * 1024 * 1024,
    };
    let left = rrrah_decode::DecodeRequest::new(&args[0]);
    let right = rrrah_decode::DecodeRequest::new(&args[1]);
    let native = native_policy::portfolio(args[4].parse()?);
    let n = native.search.anchors;
    let m = policy.anchors;
    let order = match args[5].as_str() {
        "native-first" => SearchOrder::NativeFirst,
        "model-first" => SearchOrder::ModelFirst,
        _ => return Err("invalid search order".into()),
    };
    let combined = SearchPortfolioPolicy {
        native,
        model: policy,
        order,
        model_tiles: None,
        work: PortfolioWorkLimits {
            point_checks: 6 * n.local.maximum_point_checks + m.local.maximum_point_checks,
            selection_checks: 6 * n.maximum_selection_checks + m.maximum_selection_checks,
            rank_sites: 6 * n.local.rank.maximum_sites + m.local.rank.maximum_sites,
            pixel_reads: 6 * n.local.rank.maximum_pixel_reads + m.local.rank.maximum_pixel_reads,
            geometry_hypotheses: 6 * 4096 + policy.sampling.trials,
            model_input_reads: 2 * policy.input.max_pixel_reads,
        },
    };
    let portfolio = search_loftr_portfolio_files(
        &left,
        &right,
        std::path::Path::new(&args[2]),
        combined,
        &native_policy::factors(),
        &budget,
        || false,
    )?;
    let attempted_stages = portfolio.attempted_stages;
    let first_miss = format!("{:?}", portfolio.first_miss);
    let e = match portfolio.terminal {
        SearchStageEvidence::Model(e) => e,
        SearchStageEvidence::ModelTiles(_) => unreachable!("tiles disabled in legacy diagnostic"),
        SearchStageEvidence::Native(e) => {
            let supported = e.evidence.anchors.is_some_and(|p| p.supported);
            let recipes = e.attempted_recipes;
            let reflected = e.reflected_source;
            let points = e.evidence.search.correspondences.len();
            let anchors = e.evidence.anchors.map(|p| [p.forward_anchors, p.reverse_anchors]);
            let transform = e.evidence.search.geometry.as_ref().map(|g| g.transform.matrix);
            drop(e);
            assert_eq!(budget.used(), 0);
            println!(
                "{}",
                serde_json::json!({"status":"ok","supported":supported,"terminal":"native","attempted_stages":attempted_stages,"first_miss":first_miss,"native_recipes":recipes,"reflected_source":reflected,"candidate_points":points,"anchors":anchors,"transform":transform,"managed_used":budget.used(),"managed_peak":budget.peak()})
            );
            return Ok(());
        }
    };
    let pixels=e.pixels.map(|p|serde_json::json!({"supported":p.supported,"anchors":[p.forward_anchors,p.reverse_anchors],"directions":[{"sites":p.forward.sites,"valid_sites":p.forward.valid_sites,"informative_pairs":p.forward.informative_pairs,"agreeing_pairs":p.forward.agreeing_pairs},{"sites":p.reverse.sites,"valid_sites":p.reverse.valid_sites,"informative_pairs":p.reverse.informative_pairs,"agreeing_pairs":p.reverse.agreeing_pairs}]}));
    assert_eq!(budget.used(), 0);
    println!(
        "{}",
        serde_json::json!({"status":"ok","terminal":"model","order":args[5],"attempted_stages":attempted_stages,"first_miss":first_miss,"supported":e.supported(),"model_digest":e.model_digest,"raw_proposals":e.raw_proposals,"selected_points":e.selected_points,"geometry_inliers":e.geometry_inliers,"hypotheses":e.hypotheses,"transform":e.transform.map(|t|t.matrix),"pixels":pixels,"managed_used":budget.used(),"managed_peak":budget.peak(),"scope":"Unified library lifecycle; provider inference tensors/geometry scratch outside payload account. Cooperative stage/strip cancellation; no individual-kernel preemption guarantee."})
    );
    Ok(())
}
