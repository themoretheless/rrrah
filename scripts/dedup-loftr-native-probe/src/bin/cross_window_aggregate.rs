//! Explicit diagnostic geometric selection before any pixel reads.
#[path="../native_policy.rs"] mod native_policy;
use rrrah_dedup::{geometry::{Correspondence,ProjectiveTransform},anchor_rank_file::compare_anchor_rank_files};
fn main()->Result<(),Box<dyn std::error::Error>> {
 let args:Vec<_>=std::env::args().skip(1).collect();if args.len()!=1{return Err("usage: cross_window_pixels input.json".into())}
 let v:serde_json::Value=serde_json::from_slice(&std::fs::read(&args[0])?)?;
 let raw=v["points"].as_array().ok_or("points")?;
 let point=|p:&serde_json::Value,k:&str|->Result<[f64;2],Box<dyn std::error::Error>> {Ok([f64::from_bits(p[k][0].as_u64().ok_or("bits")?),f64::from_bits(p[k][1].as_u64().ok_or("bits")?)])};
 let pts:Vec<_>=raw.iter().map(|p|Ok(Correspondence{source:point(p,"source_bits")?,target:point(p,"target_bits")?})).collect::<Result<_,Box<dyn std::error::Error>>>()?;
 let mut matrix=[[0.;3];3];for i in 0..3{for j in 0..3{matrix[i][j]=f64::from_bits(v["transform_bits"][i][j].as_u64().ok_or("transform")?);}}
 let transform=ProjectiveTransform{matrix};let tol=v["source_tolerance"].as_f64().ok_or("tolerance")?;
 let aggregate_budget=rrrah_core::MemoryBudget::new(512*1024*1024);
 let n=pts.len() as u64;
 let pts=rrrah_dedup::joint_witnesses::collect_joint_projective_witnesses(&pts,transform,(692,349),(2048,1536),rrrah_dedup::joint_witnesses::JointWitnessPolicy{source_tolerance:tol,target_tolerance:2.,minimum_separation:2.,maximum_points:669,maximum_projection_checks:2*n,maximum_separation_checks:n*(n-1)/2},&aggregate_budget,||false)?;
 let policy=native_policy::portfolio(tol);let decode=policy.search.search.search.search.gradient.search.local.decode;
 let mut left=rrrah_decode::DecodeRequest::new(v["source"].as_str().ok_or("source")?);left.assume_untagged_linear_srgb=true;
 let mut right=rrrah_decode::DecodeRequest::new(v["target"].as_str().ok_or("target")?);right.assume_untagged_linear_srgb=true;
 let budget=rrrah_core::MemoryBudget::new(512*1024*1024);let mut results=Vec::new();
 for radius in [2,6] {let mut p=policy.search.anchors;p.local.filter_radius=12;p.window_radius=radius;
 let e=compare_anchor_rank_files(&left,&right,transform,&pts,decode,p,&budget,||false);
 results.push(serde_json::json!({"window_radius":radius,"evidence":format!("{:?}",e),"supported":e.as_ref().is_ok_and(|x|x.supported)}));assert_eq!(budget.used(),0);}
 let witness_count=pts.len();drop(pts);assert_eq!(aggregate_budget.used(),0);
 println!("{}",serde_json::json!({"points":witness_count,"aggregate_used":aggregate_budget.used(),"aggregate_peak":aggregate_budget.peak(),"results":results,"managed_used":budget.used(),"managed_peak":budget.peak(),"scope":"Explicit cross-window geometrically selected witnesses, exact input H. Original pixel floors unchanged; diagnostic not production proposal union or precision qualification."}));Ok(())
}
