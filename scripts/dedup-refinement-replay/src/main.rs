use rrrah_dedup::{geometry::ProjectiveTransform,linear::LinearRgbaView};
fn sample(image:&LinearRgbaView<'_>,p:[f64;2])->Option<f64>{
 let(w,h)=image.dimensions();if p.iter().any(|v|!v.is_finite()||*v<0.0)||p[0]+1.0>=f64::from(w)||p[1]+1.0>=f64::from(h){return None;}
 let x=p[0].floor()as u32;let y=p[1].floor()as u32;let tx=p[0]-f64::from(x);let ty=p[1]-f64::from(y);
 let lum=|x,y|{let p=image.rgba(x,y)?;if p[3]!=1.0{return None;}Some(f64::from(p[0])*0.2126+f64::from(p[1])*0.7152+f64::from(p[2])*0.0722)};
 let a=lum(x,y)?;let b=lum(x+1,y)?;let c=lum(x,y+1)?;let d=lum(x+1,y+1)?;Some((a+(b-a)*tx)*(1.0-ty)+(c+(d-c)*tx)*ty)
}
fn filtered_target(image:&LinearRgbaView<'_>,model:ProjectiveTransform,p:[f64;2],q:[f64;2],radius:f64)->Option<f64>{
 if radius==0.0{return sample(image,q);}
 let mut sum=0.0;
 for y in -2..=2 {for x in -2..=2 {
 let q=model.apply([p[0]+f64::from(x)*radius/2.0,p[1]+f64::from(y)*radius/2.0])?;
 sum+=sample(image,q)?;
 }}Some(sum/25.0)
}

fn patches(a:&LinearRgbaView<'_>,b:&LinearRgbaView<'_>,model:ProjectiveTransform,points:&[[f64;2]])->serde_json::Value{
 let mut result=Vec::new();let mut score=0.0;let mut complete=true;
 for center in points {let mut pairs=Vec::new();
 for y in -4..=4 {for x in -4..=4 {let p=[center[0]+f64::from(x)/2.0,center[1]+f64::from(y)/2.0];
 if let Some(q)=model.apply(p){if let(Some(a),Some(b))=(sample(a,p),filtered_target(b,model,p,q,1.0)){pairs.push([a,b]);}}
 }}
 let mut means=[0.0;2];let mut energy=[0.0;2];let mut cov=0.0;
 for (i,p) in pairs.iter().enumerate(){let n=(i+1)as f64;let da=p[0]-means[0];let db=p[1]-means[1];means[0]+=da/n;means[1]+=db/n;energy[0]+=da*(p[0]-means[0]);energy[1]+=db*(p[1]-means[1]);cov+=da*(p[1]-means[1]);}
 let c=if energy[0]>0.0&&energy[1]>0.0&&pairs.len()==81 {Some(cov/(energy[0]*energy[1]).sqrt())}else{None};
 if let Some(c)=c{score+=c;}else{complete=false;}
 result.push(serde_json::json!({"center":center,"pairs":pairs,"correlation":c}));
 }
 serde_json::json!({"patches":result,"mean_correlation":if complete&&!points.is_empty(){Some(score/points.len()as f64)}else{None}})
}
fn shifted(model:ProjectiveTransform,d:[f64;2])->ProjectiveTransform{
 let mut m=model.matrix;for row in &mut m{row[2]+=row[0]*d[0]+row[1]*d[1];}ProjectiveTransform{matrix:m}
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
 let centers:Vec<[f64;2]>=case["witnesses"].as_array().unwrap().iter().map(|p|[p["source"][0].as_f64().unwrap(),p["source"][1].as_f64().unwrap()]).collect();
 let mut xs:Vec<_>=centers.iter().map(|p|p[0]).collect();xs.sort_by(f64::total_cmp);let mid=(xs[(xs.len()-1)/2]+xs[xs.len()/2])/2.0;
 let fit:Vec<_>=centers.iter().copied().filter(|p|p[0]<=mid-8.0).collect();let check:Vec<_>=centers.iter().copied().filter(|p|p[0]>=mid+8.0).collect();
 let mut offsets=vec![[0.0,0.0]];for dy in [-1.0,-0.5,0.0,0.5,1.0]{for dx in [-1.0,-0.5,0.0,0.5,1.0]{if dx!=0.0||dy!=0.0{offsets.push([dx,dy]);}}}
 let mut trials=Vec::new();let mut best=None;let mut score=f64::NEG_INFINITY;
 for (index,d) in offsets.iter().enumerate(){let m=shifted(model,*d);let p=patches(&av,&bv,m,&fit);if let Some(s)=p["mean_correlation"].as_f64(){if s>score{score=s;best=Some(index);}}trials.push(serde_json::json!({"offset":d,"matrix":m.matrix,"fit":p}));}
 let check_before=patches(&av,&bv,model,&check);
 let check_after=best.map(|i|patches(&av,&bv,shifted(model,offsets[i]),&check));
 rows.push(serde_json::json!({"case":case["name"],"split_x":mid,"fit_centers":fit,"check_centers":check,"trials":trials,"selected":best,"check_before":check_before,"check_after":check_after,"enough_groups":fit.len()>=3&&check.len()>=3}));
 }
 ls.verify(&||false)?;rs.verify(&||false)?;
 serde_json::json!({"rows":rows,"scope":"Fixed25 common source-coordinate translations, fit left-of-median minus8px, check right-of-median plus8px. Radius2, reference aperture1. Only fit pixels choose model; check data read after selection. Spatial center split, no independent ground-truth or statistical-independence claim. No copy verdict, diagnostic scratch external."})
 };assert_eq!(budget.used(),0);println!("{}",serde_json::json!({"evidence":output,"managed_used":budget.used(),"managed_peak":budget.peak()}));Ok(())
}
