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
    if args.len() != 7 {
        return Err("usage: native_bank_collection_files source target weights max_side source_tolerance opposite_trials third_source".into());
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
        order:rrrah_dedup::search_portfolio::SearchOrder::NativeFirst,model_tiles:None,
        work:rrrah_dedup::search_portfolio::PortfolioWorkLimits{point_checks:0,selection_checks:0,rank_sites:0,pixel_reads:0,geometry_hypotheses:0,model_input_reads:0},
    };
    portfolio.work=rrrah_dedup::search_portfolio::required_model_bank_portfolio_work(portfolio,work)?;
    let mut recovery=rrrah_dedup::search_portfolio::SearchRecoveryPolicy{
        primary:portfolio,opposite:rrrah_dedup::anchor_rank_file::OppositeAnchorSearchPolicy{
            search:portfolio.native,maximum_swapped_points:portfolio.native.search.search.search.max_union_points},work:portfolio.work};
    let trials:u64=args[5].parse()?;if trials==0||trials>1_000_000{return Err("trial limit".into());}
    let phase=&mut recovery.opposite.search.search.search.search.search.gradient.search;
    phase.sampling.trials=trials;phase.local.geometry.max_hypotheses=trials;
    let projections=12*phase.local.geometry.max_points as u64*(trials+1);
    recovery.work=rrrah_dedup::search_portfolio::required_model_bank_recovery_work(recovery,work)?;
    let mut third=rrrah_decode::DecodeRequest::new(&args[6]);third.assume_untagged_linear_srgb=true;
    let mut collection=rrrah_dedup::portfolio_collection::RecoveryCollectionPolicy{search:recovery,max_files:3,max_pairs:3,work:recovery.work};
    collection.work=collection.required_work()?;
    let total=rrrah_dedup::portfolio_collection::ModelBankCollectionWorkLimits{projection_checks:3*banks.maximum_total_projection_checks,separation_checks:3*banks.maximum_total_separation_checks};
    let schedule=rrrah_dedup::pair_schedule::CompletePairSchedule::new(&[9,2,6],&[(2,9)],3,3,1,&budget,||false)?;
    let report=rrrah_dedup::model_loftr::scan_loftr_native_bank_recovery_collection([(9,right),(2,left),(6,third)],Some(&schedule),std::path::Path::new(&args[2]),collection,banks,work,&[2,6],total,projections,3*projections,&native_policy::factors(),&budget,||false)?;
    let ids=report.file_ids().to_vec();
    let pairs:Vec<_>=report.pairs().iter().map(|p| {
        let detail=match &p.evidence.terminal {
            rrrah_dedup::search_portfolio::NativeBankRecoveryTerminal::Opposite(evidence)=>serde_json::json!({
                "terminal":"joint_opposite_native",
                "native_recipes":evidence.attempted_recipes,"reflected_proposal_source":evidence.reflected_proposal_source,
                "candidate_points":evidence.correspondences.len(),"transform":evidence.transform.map(|h|h.matrix),"pixels":evidence.anchors.map(pixels)}),
            rrrah_dedup::search_portfolio::NativeBankRecoveryTerminal::Inconclusive(reason)=>serde_json::json!({"terminal":"inconclusive","reason":format!("{reason:?}")}),
            other=>serde_json::json!({"terminal":"native_or_model","evidence":format!("{other:?}")}),
        };
        serde_json::json!({"left":p.left,"right":p.right,"support":p.evidence.supported(),"model_attempted":p.evidence.model_attempted(),"native_miss":format!("{:?}",p.evidence.native_miss),"opposite_insufficient":p.evidence.opposite_insufficient,"detail":detail})
    }).collect();
    let retained=budget.used();drop(report);drop(schedule);assert_eq!(budget.used(),0);
    println!("{}",serde_json::json!({"files":ids,"pairs":pairs,"schedule_hints":[[2,9]],"opposite_trials":trials,"managed_retained_before_drop":retained,"managed_used":budget.used(),"managed_peak":budget.peak(),"scope":"Actual CPU complete scheduled bank+joint recovery collection3 distinct files; explicit2/6 coverage,model filter12,opposite filter8. Different publisher origin is a control label, not general semantic-negative qualification."}));Ok(())
}

fn pixels(p: rrrah_dedup::anchor_rank::AnchorRankEvidence) -> serde_json::Value {
    serde_json::json!({"supported":p.supported,"anchors":[p.forward_anchors,p.reverse_anchors],"directions":[{"sites":p.forward.sites,"valid_sites":p.forward.valid_sites,"informative_pairs":p.forward.informative_pairs,"agreeing_pairs":p.forward.agreeing_pairs},{"sites":p.reverse.sites,"valid_sites":p.reverse.valid_sites,"informative_pairs":p.reverse.informative_pairs,"agreeing_pairs":p.reverse.agreeing_pairs}]})
}

