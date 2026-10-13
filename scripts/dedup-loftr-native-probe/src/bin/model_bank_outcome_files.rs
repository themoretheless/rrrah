//! Real native LoFTR provider for the unified library lifecycle diagnostic.
//! Tensor allocations remain outside admission; cancellation is cooperative between kernels.
#[path="../native_policy.rs"] mod native_policy;
use rrrah_dedup::{
    anchor_rank::AnchorRankPolicy,
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    local_rank::LocalRankPolicy,
    model_compare::ModelComparisonPolicy,
    model_input::ModelInputPolicy,

    proposals::ProposalPolicy,
    rank_region::RankRegionPolicy,
};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: model_bank_outcome_files source target weights max_side source_tolerance".into());
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
    let n=25*policy.proposals.max_proposals as u64;
    let fit=2*policy.geometry.max_points as u64*(policy.sampling.trials+1);
    let sep=n*(n-1)/2;
    let mut target=policy.geometry;target.min_inliers=4;
    let banks=rrrah_dedup::joint_witnesses::JointBankPolicy {
        hypothesis:rrrah_dedup::geometry::JointProjectivePolicy{target,source_tolerance:policy.anchors.local.source_tolerance,maximum_projection_checks:fit},
        sampling:policy.sampling,
        witnesses:rrrah_dedup::joint_witnesses::JointWitnessPolicy{source_tolerance:policy.anchors.local.source_tolerance,target_tolerance:2.,minimum_separation:2.,maximum_points:n as usize,maximum_projection_checks:2*n,maximum_separation_checks:sep},
        minimum_witnesses:10,maximum_banks:25,maximum_total_projection_checks:25*(fit+2*n),maximum_total_separation_checks:25*sep,
    };
    let mut work=rrrah_dedup::model_tiles::ModelTileWorkLimits::for_model(policy)?;
    work.model_input_reads=52*policy.input.max_pixel_reads;
    work.point_checks*=2;work.selection_checks*=2;work.rank_sites*=2;work.pixel_reads*=2;
    let mut portfolio=rrrah_dedup::search_portfolio::SearchPortfolioPolicy{
        native:native_policy::portfolio(policy.anchors.local.source_tolerance),model:policy,
        order:rrrah_dedup::search_portfolio::SearchOrder::ModelFirst,model_tiles:None,
        work:rrrah_dedup::search_portfolio::PortfolioWorkLimits{point_checks:0,selection_checks:0,rank_sites:0,pixel_reads:0,geometry_hypotheses:0,model_input_reads:0},
    };
    portfolio.work=rrrah_dedup::search_portfolio::required_model_bank_portfolio_work(portfolio,work)?;
    let total=portfolio.work;
    
    let collection=rrrah_dedup::portfolio_collection::PortfolioCollectionPolicy{search:portfolio,max_files:2,max_pairs:1,work:total};
    let joint_total=rrrah_dedup::portfolio_collection::ModelBankCollectionWorkLimits{
        projection_checks:banks.maximum_total_projection_checks,separation_checks:banks.maximum_total_separation_checks};
    let report=rrrah_dedup::model_loftr::scan_loftr_model_bank_portfolio_outcome_collection(
        [(9,right),(2,left)],None,std::path::Path::new(&args[2]),collection,banks,work,&[2,6],joint_total,&native_policy::factors(),&budget,||false)?;
    let ids=report.file_ids().to_vec();
    let pairs:Vec<_>=report.pairs().iter().map(|p| {
        let detail=match &p.evidence {
            rrrah_dedup::portfolio_collection::PortfolioPairOutcome::Complete(e)=>match &e.terminal {
                rrrah_dedup::search_portfolio::SearchStageEvidence::Model(c)=>serde_json::json!({
                    "attempted_stages":e.attempted_stages,"first_miss":format!("{:?}",e.first_miss),
                    "transform":c.transform.map(|h|h.matrix),"pixels":format!("{:?}",c.pixels),
                    "selected_points":c.selected_points,"geometry_inliers":c.geometry_inliers}),
                other=>serde_json::json!({"stage":format!("{other:?}")}),
            },other=>serde_json::json!({"inconclusive":format!("{other:?}")}),
        };
        serde_json::json!({"left":p.left,"right":p.right,"support":p.evidence.support(),"detail":detail})
    }).collect();
    let retained=budget.used();drop(report);assert_eq!(budget.used(),0);
    println!("{}",serde_json::json!({"files":ids,"pairs":pairs,"managed_retained_before_drop":retained,"managed_used":budget.used(),"managed_peak":budget.peak(),"scope":"Actual CPU exhaustive ModelFirst bank collection2 IDs, radii2/6,filter12/1m; no full corpus/precision/RSS qualification."}));Ok(())
}
