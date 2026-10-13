//! Native proposal selection and existing bounded projective fitting diagnostic.
use rrrah_dedup::geometry::{
    GeometryPolicy, ProjectiveSamplingPolicy, verify_projective_sampled_for_domains,
};
use rrrah_dedup::proposals::{
    ProposalDomain, ProposalPolicy, ScoredProposal, select_scored_proposals_managed,
};
use serde_json::Value;
use std::{error::Error, fs};

fn points(value: &Value, key: &str) -> Result<Vec<[f64; 2]>, Box<dyn Error>> {
    value[key]
        .as_array()
        .ok_or("missing points")?
        .iter()
        .map(|p| {
            let pair = p.as_array().filter(|p| p.len() == 2).ok_or("invalid point")?;
            let x = pair[0].as_f64().ok_or("invalid coordinate")?;
            let y = pair[1].as_f64().ok_or("invalid coordinate")?;
            if !x.is_finite() || !y.is_finite() {
                return Err("nonfinite point".into());
            }
            Ok([x, y])
        })
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 10 {
        return Err("usage: geometry proposals.json input_w0 input_h0 input_w1 input_h1 original_w0 original_h0 original_w1 original_h1".into());
    }
    if fs::metadata(&args[1])?.len() > 2 * 1024 * 1024 {
        return Err("proposal byte cap".into());
    }
    let value: Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let left = points(&value, "keypoints0")?;
    let right = points(&value, "keypoints1")?;
    let confidence = value["confidence"].as_array().ok_or("missing confidence")?;
    if left.len() > 5000 || left.len() != right.len() || left.len() != confidence.len() {
        return Err("proposal count cap or shape mismatch".into());
    }
    let dims: Vec<u32> = args[2..].iter().map(|v| v.parse()).collect::<Result<_, _>>()?;
    if dims.iter().any(|d| *d == 0) {
        return Err("zero dimensions".into());
    }
    let scores: Vec<f64> = confidence
        .iter()
        .map(|v| {
            v.as_f64()
                .filter(|v| v.is_finite() && (0.0..=1.0).contains(v))
                .ok_or("invalid confidence")
        })
        .collect::<Result<_, _>>()?;
    let proposals: Vec<_> = (0..left.len())
        .map(|i| ScoredProposal {
            source: left[i],
            target: right[i],
            confidence: scores[i],
        })
        .collect();
    let budget = rrrah_core::MemoryBudget::new(256 * 1024);
    let selected = select_scored_proposals_managed(
        &proposals,
        ProposalDomain {
            input: [dims[0], dims[1]],
            original: [dims[4], dims[5]],
        },
        ProposalDomain {
            input: [dims[2], dims[3]],
            original: [dims[6], dims[7]],
        },
        ProposalPolicy {
            max_proposals: 5000,
            max_ranking_checks: 200000,
            max_separation_checks: 12497500,
            minimum_separation: 2.0,
        },
        &budget,
        || false,
    )?;
    let geometry = verify_projective_sampled_for_domains(
        &selected,
        GeometryPolicy {
            tolerance: 2.0,
            min_inliers: 10,
            max_points: 5000,
            max_hypotheses: 20000,
        },
        ProjectiveSamplingPolicy {
            trials: 20000,
            seed: 173,
        },
        (dims[4], dims[5]),
        (dims[6], dims[7]),
        || false,
    )?;
    let all: Vec<_> = selected.iter().map(|p| [p.source, p.target]).collect();
    drop(selected);
    assert_eq!(budget.used(), 0);
    eprintln!(
        "proposal_memory_used={} proposal_memory_peak={}",
        budget.used(),
        budget.peak()
    );
    match geometry {
        Some(g) => println!(
            "{}",
            serde_json::json!({"status":"geometry","matrix":g.transform.matrix,"points":all,"inliers":g.inliers,"hypotheses":g.hypotheses,"squared_error":g.squared_error})
        ),
        None => println!("{}", serde_json::json!({"status":"no_geometry","points":all})),
    }
    Ok(())
}
