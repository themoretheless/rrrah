//! Real native LoFTR through explicit projection and original-RGBA confirmation.
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    model_input::ModelInputPolicy,
    model_loftr::compare_loftr_projected_files,
    model_projected_compare::ProjectedModelPolicy,
    model_projection::ModelInputProjection,
    proposals::ProposalPolicy,
    warp::WarpPolicy,
};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: projected_files source target weights tolerance".into());
    }
    tch::set_num_threads(2);
    tch::manual_seed(173);
    let policy = ProjectedModelPolicy {
        decode: AnimationBudget {
            max_frames: 1,
            max_pixels: 1000000,
            max_file_bytes: 16000000,
        },
        input: ModelInputPolicy {
            max_side: 320,
            max_source_pixels: 1000000,
            max_output_pixels: 102400,
            max_pixel_reads: 4000000,
        },
        projection: ModelInputProjection::CompositeReinhardV1 {
            exposure: 1.,
            background: [0.; 3],
        },
        proposals: ProposalPolicy {
            max_proposals: 5000,
            max_ranking_checks: 200000,
            max_separation_checks: 12497500,
            minimum_separation: 2.,
        },
        geometry: GeometryPolicy {
            tolerance: 0.05,
            min_inliers: 10,
            max_points: 5000,
            max_hypotheses: 2000,
        },
        sampling: ProjectiveSamplingPolicy {
            trials: 2000,
            seed: 173,
        },
        pixels: WarpPolicy {
            tolerance: args[3].parse()?,
            max_source_pixels: 4000000,
        },
        minimum_coverage: 0.99,
        minimum_agreement: 0.999,
        max_model_bytes: 128 * 1024 * 1024,
    };
    let mut left = rrrah_decode::DecodeRequest::new(&args[0]);
    let mut right = rrrah_decode::DecodeRequest::new(&args[1]);
    // Diagnostic fixtures explicitly store linear values. This is not a library default.
    left.assume_untagged_linear_srgb = true;
    right.assume_untagged_linear_srgb = true;
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let e = compare_loftr_projected_files(
        &left,
        &right,
        std::path::Path::new(&args[2]),
        policy,
        &budget,
        || false,
    )?;
    assert_eq!(budget.used(), 0);
    let pixel = |p: rrrah_dedup::warp::WarpEvidence| serde_json::json!({"source_pixels":p.source_pixels,"compared_pixels":p.compared_pixels,"matched_pixels":p.matched_pixels,"maximum_channel_error":p.maximum_channel_error,"squared_error":p.squared_error});
    println!(
        "{}",
        serde_json::json!({"status":"ok","supported":e.supported,
        "original_dimensions":e.original_dimensions,"model_digest":e.model_digest,
        "raw_proposals":e.raw_proposals,"selected_points":e.selected_points,"geometry_inliers":e.geometry_inliers,"pixel_attempts":e.pixel_attempts,"pixel_work":e.pixel_work,
        "transform":e.transform.map(|t|t.matrix),"pixels":e.pixels.map(|p|[pixel(p.forward),pixel(p.reverse)]),
        "managed_used":budget.used(),"managed_peak":budget.peak(),
        "scope":"CPU LoFTR, explicit CompositeReinhardV1 and original linear RGBA; tensor/workspace RSS outside managed admission; caller thresholds uncalibrated."})
    );
    Ok(())
}
