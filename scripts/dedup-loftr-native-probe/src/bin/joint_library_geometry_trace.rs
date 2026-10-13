//! Diagnostic candidate only. JSON/point scratch is not managed library admission.
#[path = "../native_policy.rs"]
mod native_policy;
use rrrah_dedup::{geometry::{Correspondence, GeometryPolicy, ProjectiveSamplingPolicy,
    JointProjectivePolicy, verify_projective_sampled_joint_for_domains},
    anchor_rank_file::compare_anchor_rank_files};
fn pair(v:&serde_json::Value)->Result<[f64;2],Box<dyn std::error::Error>> {
    let p=[v[0].as_f64().ok_or("coordinate")?,v[1].as_f64().ok_or("coordinate")?];
    if p.iter().any(|x|!x.is_finite()){return Err("nonfinite".into());}Ok(p)
}
fn exact_pair(p:&serde_json::Value, key:&str)->Result<[f64;2],Box<dyn std::error::Error>> {
    let bits=p[format!("{key}_bits")].as_array().ok_or("coordinate bits")?;
    if bits.len()!=2{return Err("bit dimensions".into());}
    let q=[f64::from_bits(bits[0].as_u64().ok_or("bits")?),f64::from_bits(bits[1].as_u64().ok_or("bits")?)];
    if q.iter().any(|x|!x.is_finite()){return Err("nonfinite bits".into());}Ok(q)
}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let args:Vec<_>=std::env::args().skip(1).collect();if args.len()!=1{return Err("usage: joint_library_geometry_trace input.json".into());}
    if std::fs::metadata(&args[0])?.len()>8*1024*1024{return Err("input limit".into());}
    let input:serde_json::Value=serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let raw=input["points"].as_array().ok_or("points")?;if raw.len()<4||raw.len()>28000{return Err("point count".into());}
    let points:Vec<_>=raw.iter().map(|p|Ok(Correspondence{source:exact_pair(p,"source")?,target:exact_pair(p,"target")?})).collect::<Result<_,Box<dyn std::error::Error>>>()?;
    let mut decimal_bit_differences=0;
    for (p,expected) in raw.iter().zip(&points) {for (key,q) in [("source",expected.source),("target",expected.target)] {let approximate=pair(&p[key])?;for i in 0..2 {decimal_bit_differences+=usize::from(q[i].to_bits()!=approximate[i].to_bits());}}}
    let mut alias_pairs=Vec::new();
    for (i,p) in points.iter().enumerate(){for (j,q) in points[..i].iter().enumerate(){for (key,a,b) in [("source",p.source,q.source),("target",p.target,q.target)]{let d=(a[0]-b[0]).hypot(a[1]-b[1]);if d<=2.{alias_pairs.push(serde_json::json!({"i":i,"j":j,"endpoint":key,"distance":d}));}}}}
    let dims=|k:&str|->Result<[u32;2],Box<dyn std::error::Error>>{let d=[u32::try_from(input[k][0].as_u64().ok_or("width")?)?,u32::try_from(input[k][1].as_u64().ok_or("height")?)?];if d.contains(&0){return Err("zero dimensions".into());}Ok(d)};
    let source=dims("source_dimensions")?;let target=dims("target_dimensions")?;
    let tol=input["source_tolerance"].as_f64().ok_or("tolerance")?;if !tol.is_finite()||tol<=0.{return Err("invalid tolerance".into());}
    let sampling=ProjectiveSamplingPolicy { trials:4096, seed:0x1234abcd };
    let maximum_projection_checks=u64::try_from(points.len())?.checked_mul(2)
        .and_then(|n|sampling.trials.checked_add(1).and_then(|trials|n.checked_mul(trials)))
        .ok_or("projection work overflow")?;
    let joint_policy=JointProjectivePolicy {
        target:GeometryPolicy { tolerance:2.,min_inliers:10,max_points:points.len(),max_hypotheses:4096 },
        source_tolerance:tol,maximum_projection_checks,
    };
    let best=verify_projective_sampled_joint_for_domains(&points,joint_policy,sampling,
        (source[0],source[1]),(target[0],target[1]),||false)?
        .map(|e|(e.transform,e.inliers,e.normalized_squared_error));
    let budget=rrrah_core::MemoryBudget::new(512*1024*1024);let mut left=rrrah_decode::DecodeRequest::new(input["source"].as_str().ok_or("source")?);left.assume_untagged_linear_srgb=true;let mut right=rrrah_decode::DecodeRequest::new(input["target"].as_str().ok_or("target")?);right.assume_untagged_linear_srgb=true;
    let policy=native_policy::portfolio(tol);let decode=policy.search.search.search.search.gradient.search.local.decode;
    let result=best.as_ref().map(|(h,_,_)| match compare_anchor_rank_files(&left,&right,*h,&points,decode,policy.search.anchors,&budget,||false) {
        Ok(e)=>serde_json::json!({"supported":e.supported,"anchors":[e.forward_anchors,e.reverse_anchors],"directions":[{"sites":e.forward.sites,"valid_sites":e.forward.valid_sites,"informative_pairs":e.forward.informative_pairs,"agreeing_pairs":e.forward.agreeing_pairs},{"sites":e.reverse.sites,"valid_sites":e.reverse.valid_sites,"informative_pairs":e.reverse.informative_pairs,"agreeing_pairs":e.reverse.agreeing_pairs}]}),
        Err(e)=>serde_json::json!({"error":format!("{e:?}")})
    });
    assert_eq!(budget.used(),0);println!("{}",serde_json::json!({"points":points.len(),"decimal_bit_differences":decimal_bit_differences,"alias_pairs":alias_pairs,"maximum_projection_checks":maximum_projection_checks,"trials":4096,"seed":0x1234abcd_u64,"source_tolerance":tol,"target_tolerance":2.,"best":best.as_ref().map(|(h,indices,score)|serde_json::json!({"transform":h.matrix,"joint_inliers":indices,"normalized_bidirectional_error":score})),"anchor_result":result,"managed_used":budget.used(),"managed_peak":budget.peak(),"scope":"Public joint sampled geometry API on exact-bit captured real points, including guarded refinement and preflight projection work. Independent original-pixel confirmation. JSON/point/geometry scratch outside managed payload admission; not automatic search integration, total RSS or full corpus qualification."}));Ok(())
}
