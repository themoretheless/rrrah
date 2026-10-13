use rrrah_dedup::{anchor_rank::{compare_anchor_rank,AnchorRankPolicy},local_rank::LocalRankPolicy,rank_region::RankRegionPolicy,geometry::{ProjectiveTransform,Correspondence},exposure::calibrate_rank_exposure_into};
fn main()->Result<(),Box<dyn std::error::Error>> {
 let args:Vec<_>=std::env::args().collect();let input:serde_json::Value=serde_json::from_slice(&std::fs::read(&args[1])?)?;
 let budget=rrrah_core::MemoryBudget::new(512*1024*1024);
 let output={
 let mut left=rrrah_decode::DecodeRequest::new(input["left"].as_str().unwrap());let mut right=rrrah_decode::DecodeRequest::new(input["right"].as_str().unwrap());
 left.assume_untagged_linear_srgb=true;right.assume_untagged_linear_srgb=true;
 let limits=rrrah_dedup::animated::AnimationBudget{max_frames:1,max_pixels:12800000,max_file_bytes:5242880};
 let ls=rrrah_dedup::exact::ContentSnapshot::read(&left.path,limits.max_file_bytes,&||false)?;let rs=rrrah_dedup::exact::ContentSnapshot::read(&right.path,limits.max_file_bytes,&||false)?;
 let a=rrrah_dedup::decode::decode_selected_frame_bounded(&left,limits,&budget,||false)?;let b=rrrah_dedup::decode::decode_selected_frame_bounded(&right,limits,&budget,||false)?;
 let av=a.view(||false)?;let bv=b.view(||false)?;
 let size=|v:&rrrah_dedup::linear::LinearRgbaView| {let(w,h)=v.dimensions();w as usize*h as usize};
 let _ac=budget.try_reserve((size(&av)*16) as u64)?;let _bc=budget.try_reserve((size(&bv)*16) as u64)?;
 let mut ab=vec![0.0;size(&av)*4];let mut bb=vec![0.0;size(&bv)*4];
 let (ca,ae)=calibrate_rank_exposure_into(&av,&mut ab,3*size(&av) as u64,||false)?;let(cb,be)=calibrate_rank_exposure_into(&bv,&mut bb,3*size(&bv) as u64,||false)?;
 let mut rows=Vec::new();
 for case in input["cases"].as_array().unwrap(){
 let model=ProjectiveTransform{matrix:std::array::from_fn(|i|std::array::from_fn(|j|case["transform"][i][j].as_f64().unwrap()))};
 let points:Vec<_>=case["witnesses"].as_array().unwrap().iter().map(|p|Correspondence{source:[p["source"][0].as_f64().unwrap(),p["source"][1].as_f64().unwrap()],target:[p["target"][0].as_f64().unwrap(),p["target"][1].as_f64().unwrap()]}).collect();

 let n=points.len();let sampling=rrrah_dedup::geometry::ProjectiveSamplingPolicy{trials:4096,seed:173};
 let refit=rrrah_dedup::geometry::verify_projective_sampled_joint_for_domains(&points,rrrah_dedup::geometry::JointProjectivePolicy{target:rrrah_dedup::geometry::GeometryPolicy{tolerance:2.0,min_inliers:n,max_points:n,max_hypotheses:4096},source_tolerance:2.0,maximum_projection_checks:2*n as u64*4097},sampling,av.dimensions(),bv.dimensions(),||false)?;
 for refined in [false,true] {let selected=if refined{let Some(e)=&refit else {rows.push(serde_json::json!({"case":case["name"],"refined":true,"refit":"no_all_witness_model"}));continue;};e.transform}else{model};
 for rank in [8,2] {for calibrated in [false,true]{
 let p=AnchorRankPolicy{window_radius:2,maximum_selection_checks:1176014000,local:LocalRankPolicy{rank:RankRegionPolicy{radius:rank,minimum_contrast:0.005,minimum_pairs:1000,maximum_sites:80000000,maximum_pixel_reads:400000000},filter_radius:12,minimum_witnesses:10,maximum_points:28000,maximum_point_checks:392014000,minimum_point_separation:2.,target_tolerance:2.,source_tolerance:2.,minimum_coverage:0.3,minimum_agreement:0.9}};
 let(x,y)=if calibrated{(&ca,&cb)}else{(&av,&bv)};let e=compare_anchor_rank(x,y,selected,&points,p,&budget,||false);
 rows.push(serde_json::json!({"case":case["name"],"refined":refined,"matrix":selected.matrix,"refit":format!("{refit:?}"),"calibrated":calibrated,"rank":rank,"result":format!("{e:?}"),"supported":e.as_ref().is_ok_and(|e|e.supported)}));
 }}} }
 ls.verify(&||false)?;rs.verify(&||false)?;

 serde_json::json!({"left_exposure":format!("{ae:?}"),"right_exposure":format!("{be:?}"),"rows":rows})
 };assert_eq!(budget.used(),0);println!("{}",serde_json::json!({"evidence":output,"managed_used":budget.used(),"managed_peak":budget.peak()}));Ok(())
}
