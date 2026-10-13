use rrrah_dedup::{geometry::ProjectiveTransform,linear::LinearRgbaView};
fn sample(image:&LinearRgbaView<'_>,p:[f64;2])->Option<f64>{
 let(w,h)=image.dimensions();if p.iter().any(|v|!v.is_finite()||*v<0.0)||p[0]+1.0>=f64::from(w)||p[1]+1.0>=f64::from(h){return None;}
 let x=p[0].floor()as u32;let y=p[1].floor()as u32;let tx=p[0]-f64::from(x);let ty=p[1]-f64::from(y);
 let lum=|x,y|{let p=image.rgba(x,y)?;if p[3]!=1.0{return None;}Some(f64::from(p[0])*0.2126+f64::from(p[1])*0.7152+f64::from(p[2])*0.0722)};
 let a=lum(x,y)?;let b=lum(x+1,y)?;let c=lum(x,y+1)?;let d=lum(x+1,y+1)?;Some((a+(b-a)*tx)*(1.0-ty)+(c+(d-c)*tx)*ty)
}
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

 let mut rows=Vec::new();
 for case in input["cases"].as_array().unwrap(){
 let model=ProjectiveTransform{matrix:std::array::from_fn(|i|std::array::from_fn(|j|case["transform"][i][j].as_f64().unwrap()))};
 for radius in [2.0,4.0,8.0] {
 let mut patches=Vec::new();
 for point in case["witnesses"].as_array().unwrap(){
 let center=[point["source"][0].as_f64().unwrap(),point["source"][1].as_f64().unwrap()];
 let mut pairs=Vec::new();let mut reads=0;
 for y in -4..=4 {for x in -4..=4 {
 let p=[center[0]+f64::from(x)*radius/4.0,center[1]+f64::from(y)*radius/4.0];
 let Some(q)=model.apply(p) else {continue};
 reads+=8;
 if let(Some(a),Some(b))=(sample(&av,p),sample(&bv,q)){pairs.push([a,b]);}
 }}
 let mut means=[0.0;2];let mut energy=[0.0;2];let mut cov=0.0;
 for (i,p) in pairs.iter().enumerate(){let n=(i+1)as f64;let da=p[0]-means[0];let db=p[1]-means[1];means[0]+=da/n;means[1]+=db/n;energy[0]+=da*(p[0]-means[0]);energy[1]+=db*(p[1]-means[1]);cov+=da*(p[1]-means[1]);}
 let correlation=if energy[0]>0.0&&energy[1]>0.0 {Some(cov/(energy[0]*energy[1]).sqrt())}else{None};
 patches.push(serde_json::json!({"source_center":center,"pairs":pairs,"means":means,"energy":energy,"covariance":cov,"correlation":correlation,"pixel_reads_upper":reads}));
 }
 rows.push(serde_json::json!({"case":case["name"],"source_radius":radius,"patches":patches}));
 }}
 ls.verify(&||false)?;rs.verify(&||false)?;
 serde_json::json!({"rows":rows,"scope":"Research original linear luminance patches mapped in common source coordinates. No contrast floor or acceptance threshold, no copy verdict. Patch samples overlap training anchors; no statistical independence claim. Diagnostic Vec/JSON scratch external to managed payload."})
 };assert_eq!(budget.used(),0);println!("{}",serde_json::json!({"evidence":output,"managed_used":budget.used(),"managed_peak":budget.peak()}));Ok(())
}
