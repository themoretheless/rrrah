//! Real native LoFTR provider for the unified library lifecycle diagnostic.
//! Tensor allocations remain outside admission; cancellation is cooperative between kernels.
use loftr::{LoftrConfig, LoftrModel};
use rrrah_dedup::{
    anchor_rank::AnchorRankPolicy,
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    local_rank::LocalRankPolicy,
    model_compare::{ModelComparisonPolicy, compare_model_files_with},
    model_input::{GrayModelInput, ModelInputPolicy},
    proposals::{ProposalPolicy, ScoredProposal},
    rank_region::RankRegionPolicy,
};
use std::{error::Error, io};
use tch::{Device, Kind, Tensor};
fn tensor(input: &GrayModelInput) -> Result<Tensor, tch::TchError> {
    Tensor::f_from_slice(&input.pixels)?
        .f_to_kind(Kind::Float)?
        .f_div_scalar(255.)?
        .f_reshape([
            1,
            1,
            i64::from(input.dimensions[1]),
            i64::from(input.dimensions[0]),
        ])
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: compare_files source target weights max_side source_tolerance".into());
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
    let e = compare_model_files_with(
        &left,
        &right,
        std::path::Path::new(&args[2]),
        policy,
        &budget,
        |weights, left, right, budget, cancel| {
            if cancel() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled before model").into());
            }
            let mut model = LoftrModel::new(Device::Cpu, LoftrConfig::outdoor())?;
            model.load_weights(weights)?;
            if cancel() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled after model load").into());
            }
            let _inference = tch::no_grad_guard();
            let raw = model.forward_cancellable(&tensor(left)?, &tensor(right)?, cancel)?;
            if cancel() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled after forward").into());
            }
            let count = raw.confidence.numel();
            if count > 5000
                || raw.keypoints0.size() != [count as i64, 2]
                || raw.keypoints1.size() != [count as i64, 2]
            {
                return Err(io::Error::other("proposal count/shape refusal").into());
            }
            let l = Vec::<Vec<f32>>::try_from(&raw.keypoints0)?;
            let r = Vec::<Vec<f32>>::try_from(&raw.keypoints1)?;
            let scores = Vec::<f32>::try_from(&raw.confidence)?;
            let mut proposals = budget.try_buffer(
                count,
                ScoredProposal {
                    source: [0.; 2],
                    target: [0.; 2],
                    confidence: 0.,
                },
            )?;
            for i in 0..count {
                if cancel() {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "cancelled during proposal adoption",
                    )
                    .into());
                }
                proposals[i] = ScoredProposal {
                    source: [f64::from(l[i][0]), f64::from(l[i][1])],
                    target: [f64::from(r[i][0]), f64::from(r[i][1])],
                    confidence: f64::from(scores[i]),
                };
            }
            Ok(proposals.freeze())
        },
        || false,
    )?;
    let pixels=e.pixels.map(|p|serde_json::json!({"supported":p.supported,"anchors":[p.forward_anchors,p.reverse_anchors],"directions":[{"sites":p.forward.sites,"valid_sites":p.forward.valid_sites,"informative_pairs":p.forward.informative_pairs,"agreeing_pairs":p.forward.agreeing_pairs},{"sites":p.reverse.sites,"valid_sites":p.reverse.valid_sites,"informative_pairs":p.reverse.informative_pairs,"agreeing_pairs":p.reverse.agreeing_pairs}]}));
    assert_eq!(budget.used(), 0);
    println!(
        "{}",
        serde_json::json!({"status":"ok","supported":e.supported(),"model_digest":e.model_digest,"raw_proposals":e.raw_proposals,"selected_points":e.selected_points,"geometry_inliers":e.geometry_inliers,"hypotheses":e.hypotheses,"transform":e.transform.map(|t|t.matrix),"pixels":pixels,"managed_used":budget.used(),"managed_peak":budget.peak(),"scope":"Unified library lifecycle; provider inference tensors/geometry scratch outside payload account. Cooperative stage/strip cancellation; no individual-kernel preemption guarantee."})
    );
    Ok(())
}
