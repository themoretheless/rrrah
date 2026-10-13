//! Real native LoFTR through exhaustive projected collection and original-RGBA confirmation.
use rrrah_dedup::{
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    model_input::ModelInputPolicy,
    model_loftr::scan_loftr_projected_collection,
    model_projected_compare::ProjectedModelPolicy,
    model_projected_tiles::ProjectedTileWorkLimits,
    model_projection::ModelInputProjection,
    projected_collection::ProjectedCollectionPolicy,
    proposals::ProposalPolicy,
    warp::WarpPolicy,
};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: projected_collection_files first second third weights tolerance".into());
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
            tolerance: args[4].parse()?,
            max_source_pixels: 4000000,
        },
        minimum_coverage: 0.99,
        minimum_agreement: 0.999,
        max_model_bytes: 128 * 1024 * 1024,
    };
    let files = args[..3].iter().enumerate().map(|(id, path)| {
        let mut request = rrrah_decode::DecodeRequest::new(path);
        request.assume_untagged_linear_srgb = true;
        (id as u64, request)
    });
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let pair_work = ProjectedTileWorkLimits::for_model(policy)?;
    let report = scan_loftr_projected_collection(
        files,
        std::path::Path::new(&args[3]),
        ProjectedCollectionPolicy {
            model: policy,
            pair_work,
            max_files: 3,
            max_pairs: 3,
            work: pair_work.for_pairs(3)?,
        },
        &budget,
        || false,
    )?;
    let pixel = |p: rrrah_dedup::warp::WarpEvidence| serde_json::json!({"source_pixels":p.source_pixels,"compared_pixels":p.compared_pixels,"matched_pixels":p.matched_pixels,"maximum_channel_error":p.maximum_channel_error,"squared_error":p.squared_error});
    let pairs: Vec<_> = report.pairs().iter().map(|pair| {
    let e = &pair.evidence;
    let attempts: Vec<_> = e.attempts().map(|attempt| {
        let p = attempt.evidence;
        serde_json::json!({"rectangles":attempt.rectangles,"supported":p.supported,
            "original_dimensions":p.original_dimensions,"model_digest":p.model_digest,
            "raw_proposals":p.raw_proposals,"selected_points":p.selected_points,"geometry_inliers":p.geometry_inliers,
            "pixel_attempts":p.pixel_attempts,"pixel_work":p.pixel_work,
            "transform":p.transform.map(|t|t.matrix),"pixels":p.pixels.map(|p|[pixel(p.forward),pixel(p.reverse)])})
    }).collect();
    serde_json::json!({"left":pair.left,"right":pair.right,"supported":e.supported(),
        "prepared_inputs":e.prepared_inputs(),"pixel_work":e.pixel_work(),"attempts":attempts})
    }).collect();
    let ids = report.file_ids().to_vec();
    drop(report);
    assert_eq!(budget.used(), 0);
    println!(
        "{}",
        serde_json::json!({"status":"ok","file_ids":ids,"pairs":pairs,
        "managed_used":budget.used(),"managed_peak":budget.peak(),
        "scope":"CPU LoFTR, automatic CompositeReinhardV1 full/quadrant regions and retained original RGBA; model/tensor/workspace RSS outside managed admission; caller thresholds uncalibrated."})
    );
    Ok(())
}
