//! Diagnostic candidate only. JSON/point scratch is not managed library admission.
#[path = "../native_policy.rs"]
mod native_policy;
use rrrah_dedup::{geometry::{Correspondence, ProjectiveTransform, fit_projective_four},
    anchor_rank_file::compare_anchor_rank_files};
fn apply_error(h: ProjectiveTransform, source: [f64;2], target: [f64;2]) -> f64 {
    h.apply(source).map_or(f64::INFINITY, |p| (p[0]-target[0]).powi(2)+(p[1]-target[1]).powi(2))
}
fn eligible(h: ProjectiveTransform, inv: ProjectiveTransform, source: [u32;2], target: [u32;2]) -> bool {
    let valid=|m:ProjectiveTransform,[w,h]:[u32;2]| {
        let v=[[0.,0.],[f64::from(w-1),0.],[0.,f64::from(h-1)],[f64::from(w-1),f64::from(h-1)]]
            .map(|[x,y]| m.matrix[2][0]*x+m.matrix[2][1]*y+m.matrix[2][2]);
        v.iter().all(|x|x.is_finite() && *x!=0. && x.is_sign_positive()==v[0].is_sign_positive())
    };valid(h,source)&&valid(inv,target)
}
fn spread(points:&[Correspondence],indices:&[usize])->bool {
    let mut mean=[0.;2];for &i in indices {for j in 0..2 {mean[j]+=points[i].source[j]/indices.len() as f64;}}
    let mut c=[0.;3];for &i in indices {let x=points[i].source[0]-mean[0];let y=points[i].source[1]-mean[1];c[0]+=x*x;c[1]+=y*y;c[2]+=x*y;}
    let d=c[0]*c[1]-c[2]*c[2];d.is_finite()&&d>(c[0]+c[1]).powi(2)*1e-8
}
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
    let args:Vec<_>=std::env::args().skip(1).collect();if args.len()!=1{return Err("usage: joint_geometry_trace input.json".into());}
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
    let mut state=0x1234abcd_u64;let mut best:Option<(ProjectiveTransform,Vec<usize>,f64)>=None;
    for _trial in 0..4096 {
        let mut selected=[0usize;4];for i in 0..4 {state^=state<<13;state^=state>>7;state^=state<<17;let mut draw=(state%((points.len()-i) as u64)) as usize;selected[..i].sort_unstable();for &previous in &selected[..i]{if draw>=previous{draw+=1;}}selected[i]=draw;}
        let Ok(h)=fit_projective_four(&selected.map(|i|points[i]),||false) else {continue;};let Ok(inv)=h.inverse() else {continue;};if !eligible(h,inv,source,target){continue;}
        let mut inliers=Vec::new();let mut score=0.;for (i,p) in points.iter().enumerate(){let a=apply_error(h,p.source,p.target);let b=apply_error(inv,p.target,p.source);if a<=4. && b<=tol*tol {inliers.push(i);score+=a/4.+b/(tol*tol);}}
        if inliers.len()<10 || !score.is_finite() || !spread(&points,&inliers){continue;}
        if best.as_ref().is_none_or(|(_,old,error)|inliers.len()>old.len()||(inliers.len()==old.len()&&score<*error)){best=Some((h,inliers,score));}
    }
    let budget=rrrah_core::MemoryBudget::new(512*1024*1024);let mut left=rrrah_decode::DecodeRequest::new(input["source"].as_str().ok_or("source")?);left.assume_untagged_linear_srgb=true;let mut right=rrrah_decode::DecodeRequest::new(input["target"].as_str().ok_or("target")?);right.assume_untagged_linear_srgb=true;
    let policy=native_policy::portfolio(tol);let decode=policy.search.search.search.search.gradient.search.local.decode;
    let result=best.as_ref().map(|(h,_,_)| match compare_anchor_rank_files(&left,&right,*h,&points,decode,policy.search.anchors,&budget,||false) {
        Ok(e)=>serde_json::json!({"supported":e.supported,"anchors":[e.forward_anchors,e.reverse_anchors],"directions":[{"sites":e.forward.sites,"valid_sites":e.forward.valid_sites,"informative_pairs":e.forward.informative_pairs,"agreeing_pairs":e.forward.agreeing_pairs},{"sites":e.reverse.sites,"valid_sites":e.reverse.valid_sites,"informative_pairs":e.reverse.informative_pairs,"agreeing_pairs":e.reverse.agreeing_pairs}]}),
        Err(e)=>serde_json::json!({"error":format!("{e:?}")})
    });
    assert_eq!(budget.used(),0);println!("{}",serde_json::json!({"points":points.len(),"decimal_bit_differences":decimal_bit_differences,"alias_pairs":alias_pairs,"trials":4096,"seed":0x1234abcd_u64,"source_tolerance":tol,"target_tolerance":2.,"best":best.as_ref().map(|(h,indices,score)|serde_json::json!({"transform":h.matrix,"joint_inliers":indices,"normalized_bidirectional_error":score})),"anchor_result":result,"managed_used":budget.used(),"managed_peak":budget.peak(),"scope":"Diagnostic candidate on captured real point sets, four-point fits, joint scoring, full-domain eligibility and spread checks; no least-squares refinement. Point/JSON/RANSAC scratch excluded from managed admission. Not integrated production API or qualified work/cancellation contract."}));Ok(())
}
