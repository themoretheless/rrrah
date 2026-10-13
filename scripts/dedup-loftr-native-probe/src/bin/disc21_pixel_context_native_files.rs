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
    if args.len() != 5 {
        return Err("usage: disc21_pixel_context_native_files source target weights max_side source_tolerance".into());
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
    work.point_checks*=4;work.selection_checks*=4;work.rank_sites*=4;work.pixel_reads*=4;
    let result=rrrah_dedup::model_loftr::search_loftr_model_banks_pixel_coverage_files(&left,&right,std::path::Path::new(&args[2]),rrrah_dedup::model_banks::ModelBankSearchPolicy{comparison:policy,banks,work},&[rrrah_dedup::model_banks::ModelBankPixelRecipe{window_radius:2,rank_radius:8},rrrah_dedup::model_banks::ModelBankPixelRecipe{window_radius:6,rank_radius:8},rrrah_dedup::model_banks::ModelBankPixelRecipe{window_radius:2,rank_radius:2},rrrah_dedup::model_banks::ModelBankPixelRecipe{window_radius:6,rank_radius:2}],&budget,||false)?;
    let e=result.comparison;
    assert_eq!(budget.used(),0);
    println!("{}",serde_json::json!({"attempts":result.attempts,"hypothesis_bank":result.hypothesis_bank,"pixel_window_radius":result.pixel_window_radius,"pixel_rank_radius":result.pixel_rank_radius,"pixel_attempts":result.pixel_attempts,"supported":result.supported(),"raw_proposals":e.raw_proposals,"selected_points":e.selected_points,"geometry_inliers":e.geometry_inliers,"transform":e.transform.map(|h|h.matrix),"original_dimensions":e.original_dimensions,"pixels":format!("{:?}",e.pixels),"managed_used":budget.used(),"managed_peak":budget.peak(),"scope":"Actual CPU25-bank extraction, automatic hypothesis and original pixels; explicit1m/filter12, not full corpus/precision/RSS qualification."}));Ok(())
}
