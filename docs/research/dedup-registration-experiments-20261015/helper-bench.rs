#![allow(dead_code)]
include!("view.rs");
#[derive(Clone,Copy,PartialEq)] enum FilterColorSpace {LinearSrgb,EncodedSrgb}
include!("encode.rs");
mod baseline {use super::*;include!("baseline.rs");pub(super) fn run(v:&LinearRgbaView<'_>,s:FilterColorSpace)->Option<[f64;4]>{premultiplied_in_space(v,0,0,s)}}
mod candidate {use super::*;include!("candidate.rs");pub(super) fn run(v:&LinearRgbaView<'_>,s:FilterColorSpace)->Option<[f64;4]>{premultiplied_in_space(v,0,0,s)}}
fn main(){let a:Vec<_>=std::env::args().collect();let equal=a[2]=="equal";let start=std::time::Instant::now();let mut sum=0.;for i in 0..1000000 {let r=std::hint::black_box((i%4096) as f32/1000.);let p=if equal{[r,r,r,1.]}else{[r,r*0.7,r*0.3,1.]};let v=LinearRgbaView{width:1,height:1,samples:std::hint::black_box(&p)};let out=if a[1]=="baseline"{baseline::run(&v,FilterColorSpace::EncodedSrgb)}else{candidate::run(&v,FilterColorSpace::EncodedSrgb)};sum+=std::hint::black_box(out.unwrap())[0];}println!("{} {}",start.elapsed().as_nanos(),sum.to_bits());}
