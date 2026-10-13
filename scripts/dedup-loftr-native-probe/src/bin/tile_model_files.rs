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
    model_tiles::ModelTileWorkLimits,
    proposals::ProposalPolicy,
    rank_region::RankRegionPolicy,
};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: tile_model_files source target weights max_side source_tolerance".into());
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
    let report = search_loftr_model_tiles_files(
        &left,
        &right,
        std::path::Path::new(&args[2]),
        policy,
        ModelTileWorkLimits::for_model(policy)?,
        &budget,
        || false,
    )?;
    let attempts: Vec<_> = report.attempts().iter().map(|a| {
        let Some(e) = a.evidence.comparison() else {
            return serde_json::json!({"left_rectangle":a.left_rectangle,"right_rectangle":a.right_rectangle,
                "inconclusive":"insufficient_anchor_geometry"});
        };
        serde_json::json!({"left_rectangle":a.left_rectangle,"right_rectangle":a.right_rectangle,
            "supported":e.supported(),"original_dimensions":e.original_dimensions,
            "raw_proposals":e.raw_proposals,"selected_points":e.selected_points,
            "geometry_inliers":e.geometry_inliers,"hypotheses":e.hypotheses,
            "transform":e.transform.map(|t|t.matrix),
            "pixels":e.pixels.map(|p|serde_json::json!({"supported":p.supported,
                "anchors":[p.forward_anchors,p.reverse_anchors],
                "directions":[{"sites":p.forward.sites,"valid_sites":p.forward.valid_sites,"informative_pairs":p.forward.informative_pairs,"agreeing_pairs":p.forward.agreeing_pairs},
                {"sites":p.reverse.sites,"valid_sites":p.reverse.valid_sites,"informative_pairs":p.reverse.informative_pairs,"agreeing_pairs":p.reverse.agreeing_pairs}]}))})
    }).collect();
    let supported = report.supported();
    let digest = report.model_digest();
    drop(report);
    assert_eq!(budget.used(), 0);
    println!(
        "{}",
        serde_json::json!({"status":"ok","supported":supported,
        "model_digest":digest,"attempts":attempts,"managed_used":budget.used(),
        "managed_peak":budget.peak(),"scope":"Automatic fixed full/quadrant search; original-file pixel confirmation. Tensor/workspace allocations outside managed budget."})
    );
    Ok(())
}
