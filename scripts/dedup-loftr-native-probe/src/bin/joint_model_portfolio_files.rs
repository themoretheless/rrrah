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
    model_loftr::search_loftr_joint_model_portfolio_files,
    proposals::ProposalPolicy,
    rank_region::RankRegionPolicy,
};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: joint_model_portfolio_files source target weights max_side source_tolerance".into());
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
    let result=search_loftr_joint_model_portfolio_files(&left,&right,std::path::Path::new(&args[2]),portfolio,
        joint.maximum_projection_checks,25*joint.maximum_projection_checks,&native_policy::factors(),&budget,||false)?;
    let (attempts,e)=match &result.terminal {
        rrrah_dedup::search_portfolio::SearchStageEvidence::ModelTiles(tiles)=>(tiles.attempts().len(),tiles.terminal().evidence.comparison().ok_or("missing comparison")?),
        _=>return Err("unexpected native/whole terminal".into()),
    };drop(result);
    assert_eq!(budget.used(),0);
    println!("{}",serde_json::json!({"attempts":attempts,"supported":e.supported(),"raw_proposals":e.raw_proposals,"selected_points":e.selected_points,"geometry_inliers":e.geometry_inliers,"transform":e.transform.map(|h|h.matrix),"original_dimensions":e.original_dimensions,"pixels":format!("{:?}",e.pixels),"managed_used":budget.used(),"managed_peak":budget.peak(),"scope":"Real CPU proposals, explicit1m joint hypotheses and filter12; same strict residual/pixel thresholds. Whole-image diagnostic, not automatic scale/tile/collection or full corpus qualification."}));Ok(())
}
