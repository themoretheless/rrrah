#[path = "../native_policy.rs"]
mod native_policy;
use rrrah_dedup::anchor_rank_file::{OppositeAnchorSearchPolicy,search_anchor_rank_opposite_files};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let args:Vec<_>=std::env::args().skip(1).collect();if args.len()!=3 {return Err("usage: opposite_native_files left right source_tolerance".into());}
    let mut left=rrrah_decode::DecodeRequest::new(&args[0]);left.assume_untagged_linear_srgb=true;let mut right=rrrah_decode::DecodeRequest::new(&args[1]);right.assume_untagged_linear_srgb=true;
    let search=native_policy::portfolio(args[2].parse()?);let policy=OppositeAnchorSearchPolicy{maximum_swapped_points:search.search.search.search.max_union_points,search};let budget=rrrah_core::MemoryBudget::new(512*1024*1024);
    let e=search_anchor_rank_opposite_files(&left,&right,policy,&native_policy::factors(),&budget,||false)?;
    let anchors=e.anchors.map(|p|serde_json::json!({"supported":p.supported,"anchors":[p.forward_anchors,p.reverse_anchors],"directions":[{"sites":p.forward.sites,"valid_sites":p.forward.valid_sites,"informative_pairs":p.forward.informative_pairs,"agreeing_pairs":p.forward.agreeing_pairs},{"sites":p.reverse.sites,"valid_sites":p.reverse.valid_sites,"informative_pairs":p.reverse.informative_pairs,"agreeing_pairs":p.reverse.agreeing_pairs}]}));
    let mut out=serde_json::json!({"source_paths":[args[0],args[1]],"source_tolerance":policy.search.search.anchors.local.source_tolerance,"target_tolerance":policy.search.search.anchors.local.target_tolerance,"attempted_recipes":e.attempted_recipes,"reflected_proposal_source":e.reflected_proposal_source,"transform":e.transform.map(|t|t.matrix),"points":e.correspondences.iter().map(|p|serde_json::json!({"source":p.source,"target":p.target})).collect::<Vec<_>>(),"pixels":anchors,"scope":"New explicit six-opposite-recipe API; not automatic portfolio integration. Diagnostic JSON containers excluded from managed admission."});drop(e);out["managed_used"]=serde_json::json!(budget.used());out["managed_peak"]=serde_json::json!(budget.peak());assert_eq!(budget.used(),0);println!("{out}");Ok(())
}
