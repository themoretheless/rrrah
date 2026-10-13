//! CPU LoFTR exact-first complete-collection diagnostics.
#[path = "../native_policy.rs"]
mod native_policy;
use rrrah_dedup::{
    animated::AnimationBudget,
    exact_portfolio_collection::*,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    model_input::ModelInputPolicy,
    model_loftr::scan_loftr_exact_projected_portfolio_collection,
    model_projected_compare::ProjectedModelPolicy,
    model_projected_tiles::{ProjectedTileEvidence, ProjectedTileWorkLimits},
    model_projection::ModelInputProjection,
    projected_portfolio::*,
    proposals::ProposalPolicy,
    search_portfolio::SearchOrder,
    warp::WarpPolicy,
};
use std::{error::Error, path::Path};
fn model_json(e: &ProjectedTileEvidence) -> serde_json::Value {
    let pixel = |p: rrrah_dedup::warp::WarpEvidence| serde_json::json!({"source_pixels":p.source_pixels,"compared_pixels":p.compared_pixels,"matched_pixels":p.matched_pixels,"maximum_channel_error":p.maximum_channel_error,"squared_error":p.squared_error});
    let attempts: Vec<_> = e.attempts().map(|attempt| {
        let p = attempt.evidence;
        serde_json::json!({"rectangles":attempt.rectangles,"supported":p.supported,
            "original_dimensions":p.original_dimensions,"model_digest":p.model_digest,
            "raw_proposals":p.raw_proposals,"selected_points":p.selected_points,"geometry_inliers":p.geometry_inliers,
            "pixel_attempts":p.pixel_attempts,"pixel_work":p.pixel_work,
            "transform":p.transform.map(|t|t.matrix),"pixels":p.pixels.map(|p|[pixel(p.forward),pixel(p.reverse)])})
    }).collect();

    serde_json::json!({"prepared_inputs":e.prepared_inputs(),"pixel_work":e.pixel_work(),"attempts":attempts})
}
fn complete_json(e: &ProjectedPortfolioEvidence) -> serde_json::Value {
    let terminal = match &e.terminal {
        ProjectedPortfolioStage::Model(e) => serde_json::json!({"kind":"model","evidence":model_json(e)}),
        ProjectedPortfolioStage::Native(e) => {
            serde_json::json!({"kind":"native","recipes":e.attempted_recipes,
            "reflected_source":e.reflected_source,"candidate_points":e.evidence.search.correspondences.len(),
            "anchors":e.evidence.anchors.map(|a|[a.forward_anchors,a.reverse_anchors]),
            "transform":e.evidence.search.geometry.as_ref().map(|g|g.transform.matrix)})
        }
    };
    serde_json::json!({"outcome":"complete","supported":e.supported(),"order":format!("{:?}",e.order),
        "attempted_stages":e.attempted_stages,"first_miss":format!("{:?}",e.first_miss),"terminal":terminal})
}
fn inconclusive_json(
    order: SearchOrder,
    reason: NativeInconclusive,
    projected: ProjectedStageMiss,
) -> serde_json::Value {
    serde_json::json!({"outcome":"inconclusive","supported":serde_json::Value::Null,"order":format!("{order:?}"),
        "reason":format!("{reason:?}"),"projected":{"attempts":projected.attempts,"model_digest":projected.model_digest,
        "raw_proposals":projected.raw_proposals,"selected_points":projected.selected_points,"pixel_work":projected.pixel_work}})
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("usage: exact_collection_files weights native-first|model-first [source ...]".into());
    }
    let order = match args[1].as_str() {
        "native-first" => SearchOrder::NativeFirst,
        "model-first" => SearchOrder::ModelFirst,
        _ => return Err("invalid order".into()),
    };
    tch::set_num_threads(2);
    tch::manual_seed(173);
    let model = ProjectedModelPolicy {
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
            tolerance: 0.05,
            max_source_pixels: 4000000,
        },
        minimum_coverage: 0.99,
        minimum_agreement: 0.999,
        max_model_bytes: 128 * 1024 * 1024,
    };

    let model_work = ProjectedTileWorkLimits::for_model(model)?;
    let mut policy = ProjectedPortfolioPolicy {
        native: native_policy::portfolio(2.),
        model,
        model_work,
        order,
        work: ProjectedPortfolioWorkLimits {
            point_checks: 0,
            selection_checks: 0,
            rank_sites: 0,
            rank_pixel_reads: 0,
            geometry_hypotheses: 0,
            model: model_work,
        },
    };
    policy.work = policy.required_work()?;
    let requests: Vec<_> = args[2..]
        .iter()
        .map(|path| {
            let mut r = rrrah_decode::DecodeRequest::new(path);
            r.assume_untagged_linear_srgb = true;
            r
        })
        .collect();
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let weights = Path::new(&args[0]);
    let report = scan_loftr_exact_projected_portfolio_collection(
        requests.into_iter().enumerate().map(|(id, r)| (id as u64, r)),
        weights,
        ExactPortfolioCollectionPolicy {
            search: policy,
            max_files: 3,
            max_pairs: 3,
            work: policy.work.for_pairs(3)?,
        },
        &native_policy::factors(),
        &budget,
        || false,
    )?;
    let pairs: Vec<_> = report
        .pairs()
        .iter()
        .map(|pair| {
            let evidence = match &pair.evidence {
                ExactPortfolioPairOutcome::ExactBytes => serde_json::json!({"outcome":"exact_bytes"}),
                ExactPortfolioPairOutcome::Complete(e) => complete_json(e),
                ExactPortfolioPairOutcome::Inconclusive {
                    order,
                    reason,
                    projected,
                } => inconclusive_json(*order, *reason, *projected),
            };
            serde_json::json!({"left":pair.left,"right":pair.right,"evidence":evidence})
        })
        .collect();
    let output = serde_json::json!({"file_ids":report.file_ids(),"pairs":pairs});
    drop(report);
    assert_eq!(budget.used(), 0);
    println!(
        "{}",
        serde_json::json!({"status":"ok","result":output,"managed_used":budget.used(),"managed_peak":budget.peak(),
        "scope":"Exact-byte/native/projected complete collection controls; unknown is distinct from false; provider tensors/workspace RSS outside managed admission; caller thresholds uncalibrated."})
    );
    Ok(())
}
