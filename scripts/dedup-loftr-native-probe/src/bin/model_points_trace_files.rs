//! Real native LoFTR provider for the unified library lifecycle diagnostic.
//! Tensor allocations remain outside admission; cancellation is cooperative between kernels.
use rrrah_dedup::{
    anchor_rank::AnchorRankPolicy,
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    local_rank::LocalRankPolicy,
    model_compare::ModelComparisonPolicy,
    model_input::ModelInputPolicy,
    model_loftr::search_loftr_model_tiles_files,
    proposals::ProposalPolicy,
    rank_region::RankRegionPolicy,
};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: model_points_trace_files source target weights max_side source_tolerance".into());
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
    let mut left=rrrah_decode::DecodeRequest::new(&args[0]);left.assume_untagged_linear_srgb=true;
    let mut right=rrrah_decode::DecodeRequest::new(&args[1]);right.assume_untagged_linear_srgb=true;
    let work=rrrah_dedup::model_tiles::ModelTileWorkLimits::for_model(policy)?;
    let e=match search_loftr_model_tiles_files(&left,&right,std::path::Path::new(&args[2]),policy,work,&budget,||false) {
        Ok(e)=>e,
        Err(rrrah_dedup::model_tiles::ModelTileError::InsufficientAnchorGeometry(e))=>e,
        Err(error)=>return Err(error.into()),
    };
    let attempts:Vec<_>=e.attempts().iter().map(|a|serde_json::json!({"left_rectangle":a.left_rectangle,"right_rectangle":a.right_rectangle,"evidence":format!("{:?}",a.evidence)})).collect();
    let insufficient=e.insufficient_geometry_attempts();let digest=e.model_digest();drop(e);assert_eq!(budget.used(),0);
    println!("{}",serde_json::json!({"source_paths":[args[0],args[1]],"source_tolerance":policy.anchors.local.source_tolerance,"target_tolerance":policy.anchors.local.target_tolerance,"attempts":attempts,"insufficient_attempts":insufficient,"model_digest":digest,"managed_used":budget.used(),"managed_peak":budget.peak(),"scope":"Diagnostic frozen model point trace on stderr, exact mapped coordinate bits after selection. Original target-only geometry and pixel decisions unchanged. Trace Vec and logs outside managed admission; not production instrumentation or runtime memory qualification."}));Ok(())
}
