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
    model_loftr::scan_loftr_joint_model_portfolio_outcome_collection,
    proposals::ProposalPolicy,
    rank_region::RankRegionPolicy,
};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 6 {
        return Err("usage: joint_model_collection_files source target weights max_side source_tolerance third_source".into());
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
            max_hypotheses: 1000000,
        },
        sampling: ProjectiveSamplingPolicy {
            trials: 1000000,
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
                filter_radius: 12,
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
    let mut left=rrrah_decode::DecodeRequest::new(&args[0]);left.assume_untagged_linear_srgb=true;
    let mut right=rrrah_decode::DecodeRequest::new(&args[1]);right.assume_untagged_linear_srgb=true;
    let joint=rrrah_dedup::model_compare::JointModelComparisonPolicy {
        comparison:policy,maximum_projection_checks:2*5000*1000001,
    };
    let work=rrrah_dedup::model_tiles::ModelTileWorkLimits::for_model(policy)?;
    let mut portfolio=rrrah_dedup::search_portfolio::SearchPortfolioPolicy {
        native:native_policy::portfolio(policy.anchors.local.source_tolerance),model:policy,
        order:rrrah_dedup::search_portfolio::SearchOrder::ModelFirst,model_tiles:Some(work),
        work:rrrah_dedup::search_portfolio::PortfolioWorkLimits {point_checks:0,selection_checks:0,rank_sites:0,pixel_reads:0,geometry_hypotheses:0,model_input_reads:0},
    };
    portfolio.work=portfolio.required_work()?;
    let mut third=rrrah_decode::DecodeRequest::new(&args[5]);third.assume_untagged_linear_srgb=true;
    let mut total=portfolio.work;
    total.point_checks*=3;total.selection_checks*=3;total.rank_sites*=3;
    total.pixel_reads*=3;total.geometry_hypotheses*=3;total.model_input_reads*=3;
    let collection=rrrah_dedup::portfolio_collection::PortfolioCollectionPolicy {
        search:portfolio,max_files:3,max_pairs:3,work:total,
    };
    let report=scan_loftr_joint_model_portfolio_outcome_collection([(9,right),(2,left),(6,third)],
        std::path::Path::new(&args[2]),collection,joint.maximum_projection_checks,
        25*joint.maximum_projection_checks,75*joint.maximum_projection_checks,
        &native_policy::factors(),&budget,||false)?;
    let ids=report.file_ids().to_vec();
    let pairs:Vec<_>=report.pairs().iter().map(|p| {
        let detail=match &p.evidence {
            rrrah_dedup::portfolio_collection::PortfolioPairOutcome::Complete(e)=> match &e.terminal {
                rrrah_dedup::search_portfolio::SearchStageEvidence::ModelTiles(t)=> {
                    let c=t.terminal().evidence.comparison();
                    serde_json::json!({"attempts":t.attempts().len(),"transform":c.and_then(|e|e.transform.map(|h|h.matrix)),"pixels":c.map(|e|format!("{:?}",e.pixels))})
                }, other=>serde_json::json!({"stage":format!("{other:?}")}),
            },other=>serde_json::json!({"inconclusive":format!("{other:?}")}),
        };
        serde_json::json!({"left":p.left,"right":p.right,"support":p.evidence.support(),"detail":detail})
    }).collect();
    let retained=budget.used();drop(report);assert_eq!(budget.used(),0);
    println!("{}",serde_json::json!({"files":ids,"pairs":pairs,"managed_retained_before_drop":retained,"managed_used":budget.used(),"managed_peak":budget.peak(),"scope":"Actual CPU exhaustive joint model collection on3 file IDs, fixed filter12/1m. No grouping or precision/full corpus/RSS claim."}));Ok(())
}
