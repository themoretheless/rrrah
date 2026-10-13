//! Diagnostic only: real native correspondences, unchanged caller thresholds.
#[path = "../native_policy.rs"]
mod native_policy;
use rrrah_dedup::{anchor_rank::AnchorRankEvidence, anchor_rank_file::compare_anchor_rank_files, geometry::Correspondence, local_scan::{find_projective_candidate_union_geometry_files_managed, find_projective_candidate_union_reflected_geometry_files_managed}};
use std::error::Error;
fn pixels(e: AnchorRankEvidence) -> serde_json::Value {
    serde_json::json!({"supported":e.supported,"anchors":[e.forward_anchors,e.reverse_anchors],"directions":[{"sites":e.forward.sites,"valid_sites":e.forward.valid_sites,"informative_pairs":e.forward.informative_pairs,"agreeing_pairs":e.forward.agreeing_pairs},{"sites":e.reverse.sites,"valid_sites":e.reverse.valid_sites,"informative_pairs":e.reverse.informative_pairs,"agreeing_pairs":e.reverse.agreeing_pairs}]})
}
fn main() -> Result<(),Box<dyn Error>> {
    let args:Vec<_>=std::env::args().skip(1).collect();
    if args.len()!=5 {return Err("usage: native_direction_trace original query forward_source_tolerance reverse_source_tolerance forward|0..5".into());}
    tch::set_num_threads(2);
    let mut original=rrrah_decode::DecodeRequest::new(&args[0]);original.assume_untagged_linear_srgb=true;
    let mut query=rrrah_decode::DecodeRequest::new(&args[1]);query.assume_untagged_linear_srgb=true;
    let forward=args[4]=="forward";
    let phase:usize=if forward {0} else {args[4].parse()?};if phase>=6 {return Err("phase out of bounds".into());}
    let tol:f64=if forward {args[2].parse()?} else {args[3].parse()?};
    let portfolio=native_policy::portfolio(tol);let base=portfolio.search;let config=base.search.search;
    let radii=[[config.smoothing_radius;2],[base.search.asymmetric_radius,0],[0,base.search.asymmetric_radius]];
    let (left,right)=if forward {(&original,&query)} else {(&query,&original)};
    let budget=rrrah_core::MemoryBudget::new(512*1024*1024);let factors=native_policy::factors();
    let evidence=if phase<3 {find_projective_candidate_union_geometry_files_managed(left,right,config,&factors,radii[phase%3],&budget,||false)?} else {find_projective_candidate_union_reflected_geometry_files_managed(left,right,config,&factors,radii[phase%3],portfolio.maximum_reflection_bins/3,&budget,||false)?};
    let decode=config.search.gradient.search.local.decode;
    let mut output=serde_json::json!({"direction":if forward {"forward"} else {"reverse"},"phase":phase,"smoothing_radii":radii[phase%3],"reflected_source":phase>=3,"source_tolerance":tol,"correspondences":evidence.correspondences.iter().map(|p|serde_json::json!({"source":p.source,"target":p.target})).collect::<Vec<_>>(),"geometry":serde_json::Value::Null});
    if let Some(g)=&evidence.geometry {
        output["geometry"]=serde_json::json!({"transform":g.transform.matrix,"inliers":g.inliers.iter().copied().collect::<Vec<_>>(),"squared_error":g.squared_error,"hypotheses":g.hypotheses});
        output["anchor_result"]=match compare_anchor_rank_files(left,right,g.transform,&evidence.correspondences,decode,base.anchors,&budget,||false) {Ok(e)=>pixels(e),Err(e)=>serde_json::json!({"error":format!("{e:?}")})};
        if forward {
            let count=evidence.correspondences.len();let bytes=count.checked_mul(std::mem::size_of::<Correspondence>()).ok_or("size overflow")?;let credit=budget.try_reserve(u64::try_from(bytes)?)?;let mut points=Vec::new();points.try_reserve_exact(count)?;points.extend(evidence.correspondences.iter().map(|p|Correspondence{source:p.target,target:p.source}));let points=credit.try_adopt(points)?;
            let inverse=g.transform.inverse()?;let reverse_policy=native_policy::portfolio(args[3].parse()?).search.anchors;
            output["inverse_transform"]=serde_json::json!(inverse.matrix);
            output["swapped_actual_correspondence_anchor_result"]=match compare_anchor_rank_files(&query,&original,inverse,&points,decode,reverse_policy,&budget,||false) {Ok(e)=>pixels(e),Err(e)=>serde_json::json!({"error":format!("{e:?}")})};
        }
    }
    drop(evidence);output["managed_used"]=serde_json::json!(budget.used());output["managed_peak"]=serde_json::json!(budget.peak());output["scope"]=serde_json::json!("Diagnostic JSON containers excluded from managed admission. Actual descriptor-qualified correspondences; no synthetic anchors or threshold changes. Geometry alone is not support. Independent guarded calls, not one atomic lifecycle across both directions.");assert_eq!(budget.used(),0);println!("{output}");Ok(())
}
