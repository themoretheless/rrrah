//! Real native LoFTR provider for the unified library lifecycle diagnostic.
//! Tensor allocations remain outside admission; cancellation is cooperative between kernels.
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
    if args.len() != 6 {
        return Err("usage: disc21_exposure_recovery_collection source target negative weights max_side source_tolerance".into());
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
            max_side: args[4].parse()?,
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
            min_inliers: 20,
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
                minimum_witnesses: 20,
                maximum_points: 28000,
                maximum_point_checks: 392014000,
                minimum_point_separation: 2.,
                target_tolerance: 2.,
                source_tolerance: args[5].parse()?,
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
        minimum_witnesses:20,maximum_banks:25,maximum_total_projection_checks:25*(fit+2*n),maximum_total_separation_checks:25*sep,
    };
    let mut work=rrrah_dedup::model_tiles::ModelTileWorkLimits::for_model(policy)?;
    work.model_input_reads=52*policy.input.max_pixel_reads;
    work.point_checks*=4;work.selection_checks*=4;work.rank_sites*=4;work.pixel_reads*=4;

    let recipes=[
      rrrah_dedup::model_banks::ModelBankPixelRecipe{window_radius:2,rank_radius:8},
      rrrah_dedup::model_banks::ModelBankPixelRecipe{window_radius:6,rank_radius:8},
      rrrah_dedup::model_banks::ModelBankPixelRecipe{window_radius:2,rank_radius:2},
      rrrah_dedup::model_banks::ModelBankPixelRecipe{window_radius:6,rank_radius:2},
    ];
    let recovery=rrrah_dedup::model_banks::ModelBankSearchPolicy{comparison:policy,banks,work};
    let mut primary=recovery;
    primary.comparison.geometry.min_inliers=10;
    primary.comparison.anchors.local.minimum_witnesses=10;
    primary.comparison.anchors.local.rank.radius=2;
    primary.banks.minimum_witnesses=10;
    let primary_recipes=[
      rrrah_dedup::model_banks::ModelBankPixelRecipe{window_radius:2,rank_radius:2},
      rrrah_dedup::model_banks::ModelBankPixelRecipe{window_radius:6,rank_radius:2},
    ];
    let policy=rrrah_dedup::model_banks::ModelBankExposureRecoveryPolicy::for_stages(primary,
      recovery,6*recovery.comparison.decode.max_pixels*25*recipes.len() as u64)?;
    let mut negative=rrrah_decode::DecodeRequest::new(&args[2]);negative.assume_untagged_linear_srgb=true;
    let policy=rrrah_dedup::model_banks::ModelBankExposureCollectionPolicy::for_pairs(policy,3,3)?;
    let result=rrrah_dedup::model_loftr::scan_loftr_model_banks_exposure_recovery_collection(
      [(1,left),(2,right),(3,negative)],None,std::path::Path::new(&args[3]),policy,
      &primary_recipes,&recipes,&budget,||false)?;
    let render=|e:rrrah_dedup::model_banks::ModelBankSearchEvidence|serde_json::json!({
      "supported":e.supported(),"attempts":e.attempts,"selected_bank":e.hypothesis_bank,
      "geometry_inliers":e.comparison.geometry_inliers,"pixel_attempts":e.pixel_attempts,
      "pixel_window_radius":e.pixel_window_radius,"pixel_rank_radius":e.pixel_rank_radius,
      "original_dimensions":e.comparison.original_dimensions,"raw_proposals":e.comparison.raw_proposals,
      "selected_points":e.comparison.selected_points,"transform":e.comparison.transform.map(|h|h.matrix),
      "pixels":format!("{:?}",e.comparison.pixels)});
    let pairs:Vec<_>=result.pairs().iter().map(|p|{let e=p.evidence;serde_json::json!({"left":p.left,"right":p.right,
      "supported":e.supported(),"primary":render(e.primary),"recovery":e.recovery.map(|r|serde_json::json!({"search":render(r.search),"exposures":format!("{:?}",r.exposures)}))})}).collect();
    let output=serde_json::json!({"file_ids":result.file_ids(),"pairs":pairs,
      "scope":"Actual public three-distinct-source exhaustive collection, ordinary25-bank then calibrated9-region only on miss. One cached CPU provider spans the batch. Development control, not general precision/RSS/platform/full500 qualification."});
    let report_used=budget.used();assert!(report_used>0);drop(result);assert_eq!(budget.used(),0);
    println!("{}",serde_json::json!({"evidence":output,"managed_report_used":report_used,"managed_used":budget.used(),"managed_peak":budget.peak()}));Ok(())
}
