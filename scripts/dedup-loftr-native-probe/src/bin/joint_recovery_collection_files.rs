//! Real native LoFTR provider for the unified library lifecycle diagnostic.
//! Tensor allocations remain outside admission; cancellation is cooperative between kernels.
#[path = "../native_policy.rs"]
mod native_policy;
use rrrah_dedup::{
    anchor_rank::AnchorRankPolicy,
    animated::AnimationBudget,
    geometry::{GeometryPolicy, ProjectiveSamplingPolicy},
    local_rank::LocalRankPolicy,
    model_compare::ModelComparisonPolicy,
    model_input::ModelInputPolicy,
    model_loftr::scan_loftr_joint_recovery_collection,
    portfolio_collection::RecoveryCollectionPolicy,
    proposals::ProposalPolicy,
    rank_region::RankRegionPolicy,
    search_portfolio::{PortfolioWorkLimits, SearchOrder, SearchPortfolioPolicy, SearchStageEvidence},
};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 5 {
        return Err(
            "usage: recovery_collection_files weights max_side source_tolerance native-first|model-first pair-list-or-dash [files...]"
                .into(),
        );
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
            max_side: args[1].parse()?,
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
                source_tolerance: args[2].parse()?,
                minimum_coverage: 0.3,
                minimum_agreement: 0.9,
            },
            window_radius: 2,
            maximum_selection_checks: 1176014000,
        },
        max_model_bytes: 128 * 1024 * 1024,
    };

    let native = native_policy::portfolio(args[2].parse()?);
    let n = native.search.anchors;
    let m = policy.anchors;
    let order = match args[3].as_str() {
        "native-first" => SearchOrder::NativeFirst,
        "model-first" => SearchOrder::ModelFirst,
        _ => return Err("invalid search order".into()),
    };
    let mut search = SearchPortfolioPolicy {
        native,
        model: policy,
        order,
        model_tiles: Some(rrrah_dedup::model_tiles::ModelTileWorkLimits::for_model(policy)?),
        work: PortfolioWorkLimits {
            point_checks: 6 * n.local.maximum_point_checks + m.local.maximum_point_checks,
            selection_checks: 6 * n.maximum_selection_checks + m.maximum_selection_checks,
            rank_sites: 6 * n.local.rank.maximum_sites + m.local.rank.maximum_sites,
            pixel_reads: 6 * n.local.rank.maximum_pixel_reads + m.local.rank.maximum_pixel_reads,
            geometry_hypotheses: 6 * 4096 + policy.sampling.trials,
            model_input_reads: 2 * policy.input.max_pixel_reads,
        },
    };
    search.work = search.required_work()?;
    let mut recovery = rrrah_dedup::search_portfolio::SearchRecoveryPolicy {
        primary: search,
        opposite: rrrah_dedup::anchor_rank_file::OppositeAnchorSearchPolicy {
            search: native,
            maximum_swapped_points: native.search.search.search.max_union_points,
        },
        work: search.work,
    };
    recovery.work = recovery.required_work()?;
    let files: Vec<_> = args[5..]
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut request = rrrah_decode::DecodeRequest::new(p);
            request.assume_untagged_linear_srgb = true;
            (i as u64, request)
        })
        .collect();
    let count = files
        .len()
        .checked_mul(files.len().saturating_sub(1))
        .map(|v| v / 2)
        .ok_or("pair count overflow")?;
    let count64 = u64::try_from(count)?;
    let multiply = |v: u64| v.checked_mul(count64).ok_or("collection work overflow");
    let p = recovery.work;
    let config = RecoveryCollectionPolicy {
        search: recovery,
        max_files: files.len(),
        max_pairs: count,
        work: PortfolioWorkLimits {
            point_checks: multiply(p.point_checks)?,
            selection_checks: multiply(p.selection_checks)?,
            rank_sites: multiply(p.rank_sites)?,
            pixel_reads: multiply(p.pixel_reads)?,
            geometry_hypotheses: multiply(p.geometry_hypotheses)?,
            model_input_reads: multiply(p.model_input_reads)?,
        },
    };
    let hints = if args[4] == "-" {
        Vec::new()
    } else {
        args[4]
            .split(',')
            .map(|pair| -> Result<(u64, u64), Box<dyn Error>> {
                let (left, right) = pair.split_once(':').ok_or("expected left:right proposal")?;
                Ok((left.parse()?, right.parse()?))
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    let ids: Vec<_> = files.iter().map(|v| v.0).collect();
    let schedule = rrrah_dedup::pair_schedule::CompletePairSchedule::new(
        &ids,
        &hints,
        files.len(),
        count64,
        hints.len(),
        &budget,
        || false,
    )?;
    let geometry = recovery.opposite.search.search.search.search.search.gradient.search;
    let pair_projections = 12u64.checked_mul(u64::try_from(geometry.local.geometry.max_points)?)
        .and_then(|n| geometry.sampling.trials.checked_add(1).and_then(|t| n.checked_mul(t)))
        .ok_or("projection work overflow")?;
    let total_projections=multiply(pair_projections)?;
    let report = scan_loftr_joint_recovery_collection(
        files,
        Some(&schedule),
        std::path::Path::new(&args[0]),
        config,
        &native_policy::factors(),
        &budget,
        pair_projections,
        total_projections,
        || false,
    )?;
    let pairs:Vec<_>=report.pairs().iter().map(|p| {
        let evidence = match &p.evidence {
            rrrah_dedup::search_portfolio::SearchRecoveryOutcome::Primary(evidence) => evidence,
            rrrah_dedup::search_portfolio::SearchRecoveryOutcome::Opposite { primary_insufficient, evidence } => {
                return serde_json::json!({"left":p.left,"right":p.right,"evidence":{
                    "terminal":"joint_opposite_native","supported":p.evidence.supported(),
                    "primary_insufficient":format!("{primary_insufficient:?}"),
                    "native_recipes":evidence.attempted_recipes,
                    "reflected_proposal_source":evidence.reflected_proposal_source,
                    "candidate_points":evidence.correspondences.len(),
                    "transform":evidence.transform.map(|t|t.matrix),"pixels":evidence.anchors.map(pixels)}});
            }
            rrrah_dedup::search_portfolio::SearchRecoveryOutcome::Inconclusive { primary_insufficient, opposite_insufficient } => {
                return serde_json::json!({"left":p.left,"right":p.right,"evidence":{
                    "terminal":"inconclusive","supported":serde_json::Value::Null,
                    "primary_insufficient":format!("{primary_insufficient:?}"),
                    "opposite_insufficient":opposite_insufficient}});
            }
        };
        let common=match &evidence.terminal {
            SearchStageEvidence::ModelTiles(e)=>serde_json::json!({"terminal":"model_tiles","supported":e.supported(),"attempts":e.attempts().iter().map(|a|serde_json::json!({"left_rectangle":a.left_rectangle,"right_rectangle":a.right_rectangle,"evidence":a.evidence.comparison().map(|e|model(&e)).unwrap_or_else(||serde_json::json!({"inconclusive":"insufficient_anchor_geometry"}))})).collect::<Vec<_>>()}),
            SearchStageEvidence::Model(e)=>serde_json::json!({"terminal":"model","supported":e.supported(),"model_digest":e.model_digest,"raw_proposals":e.raw_proposals,"selected_points":e.selected_points,"geometry_inliers":e.geometry_inliers,"hypotheses":e.hypotheses,"transform":e.transform.map(|t|t.matrix),"pixels":e.pixels.map(pixels)}),
            SearchStageEvidence::Native(e)=>serde_json::json!({"terminal":"native","supported":e.evidence.anchors.is_some_and(|p|p.supported),"native_recipes":e.attempted_recipes,"reflected_source":e.reflected_source,"candidate_points":e.evidence.search.correspondences.len(),"transform":e.evidence.search.geometry.as_ref().map(|g|g.transform.matrix),"pixels":e.evidence.anchors.map(pixels)}),
        };
        serde_json::json!({"left":p.left,"right":p.right,"attempted_stages":evidence.attempted_stages,"first_miss":evidence.first_miss.map(miss),"evidence":common})
    }).collect();
    let ids = report.file_ids().to_vec();
    let retained = budget.used();
    drop(report);
    drop(schedule);
    assert_eq!(budget.used(), 0);
    println!(
        "{}",
        serde_json::json!({"status":"ok","files":ids,"source_paths":&args[5..],"proposal_pairs":hints,"pairs":pairs,"managed_retained_before_drop":retained,"managed_used":budget.used(),"managed_peak":budget.peak(),"scope":"Proposal-first exhaustive fallback, no retrieval pruning or transitive grouping; diagnostic assumes untagged input linear sRGB. Metadata slot capacities and managed inputs/evidence are admitted; provider tensors/workspaces remain outside total RSS admission. Final caller cancellation checkpoint is followed by read-only source revalidation with generation checks."})
    );
    Ok(())
}
fn pixels(p: rrrah_dedup::anchor_rank::AnchorRankEvidence) -> serde_json::Value {
    serde_json::json!({"supported":p.supported,"anchors":[p.forward_anchors,p.reverse_anchors],"directions":[{"sites":p.forward.sites,"valid_sites":p.forward.valid_sites,"informative_pairs":p.forward.informative_pairs,"agreeing_pairs":p.forward.agreeing_pairs},{"sites":p.reverse.sites,"valid_sites":p.reverse.valid_sites,"informative_pairs":p.reverse.informative_pairs,"agreeing_pairs":p.reverse.agreeing_pairs}]})
}

fn model(e: &rrrah_dedup::model_compare::ModelComparisonEvidence) -> serde_json::Value {
    serde_json::json!({"supported":e.supported(),"original_dimensions":e.original_dimensions,"model_digest":e.model_digest,"raw_proposals":e.raw_proposals,"selected_points":e.selected_points,"geometry_inliers":e.geometry_inliers,"hypotheses":e.hypotheses,"transform":e.transform.map(|t|t.matrix),"pixels":e.pixels.map(pixels)})
}
fn miss(e: rrrah_dedup::search_portfolio::SearchStageMiss) -> serde_json::Value {
    use rrrah_dedup::search_portfolio::SearchStageMiss;
    match e {
        SearchStageMiss::InsufficientAnchorGeometry(stage) => match stage {
            rrrah_dedup::search_portfolio::InsufficientAnchorStage::Native => {
                serde_json::json!({"stage":"native","inconclusive":"insufficient_anchor_geometry"})
            }
            rrrah_dedup::search_portfolio::InsufficientAnchorStage::Model { model_digest } => {
                serde_json::json!({"stage":"model","inconclusive":"insufficient_anchor_geometry","model_digest":model_digest})
            }
            rrrah_dedup::search_portfolio::InsufficientAnchorStage::ModelTiles {
                attempts,
                insufficient_attempts,
                model_digest,
            } => {
                serde_json::json!({"stage":"model_tiles","inconclusive":"insufficient_anchor_geometry","attempts":attempts,"insufficient_attempts":insufficient_attempts,"model_digest":model_digest})
            }
        },
        SearchStageMiss::Native { recipes, candidates } => {
            serde_json::json!({"stage":"native","recipes":recipes,"candidates":candidates})
        }
        SearchStageMiss::Model {
            model_digest,
            raw_proposals,
            selected_points,
        } => {
            serde_json::json!({"stage":"model","model_digest":model_digest,"raw_proposals":raw_proposals,"selected_points":selected_points})
        }
        SearchStageMiss::ModelTiles {
            attempts,
            model_digest,
            raw_proposals,
            selected_points,
        } => {
            serde_json::json!({"stage":"model_tiles","attempts":attempts,"model_digest":model_digest,"raw_proposals":raw_proposals,"selected_points":selected_points})
        }
    }
}
