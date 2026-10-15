#![allow(dead_code)]
include!("view.rs");
#[derive(Clone,Copy,PartialEq)] enum FilterColorSpace {LinearSrgb,EncodedSrgb}
include!("encode.rs");
mod baseline {use super::*;include!("baseline.rs");pub(super) fn run(v:&LinearRgbaView<'_>,s:FilterColorSpace)->Option<[f64;4]>{premultiplied_in_space(v,0,0,s)}}
mod candidate {use super::*;include!("candidate.rs");pub(super) fn run(v:&LinearRgbaView<'_>,s:FilterColorSpace)->Option<[f64;4]>{premultiplied_in_space(v,0,0,s)}}
fn main(){let mut seed=123456789u32;let mut checked=0;
 for i in 0..200000 {let mut p=[0f32;4];for c in &mut p[..3]{seed=seed.wrapping_mul(1664525).wrapping_add(1013904223);*c=(seed as i32 as f32)/1000000.;}p[3]=[0.,0.25,1.][i%3];match i%4{0=>{p[1]=p[0];p[2]=p[0]},1=>p[1]=p[0],2=>p[2]=p[1],_=>{}}let v=LinearRgbaView{width:1,height:1,samples:&p};for space in [FilterColorSpace::LinearSrgb,FilterColorSpace::EncodedSrgb]{assert_eq!(baseline::run(&v,space).map(|p|p.map(f64::to_bits)),candidate::run(&v,space).map(|p|p.map(f64::to_bits)));checked+=1;}}
 for p in [[-0.,0.,-0.,1.],[f32::NAN,f32::INFINITY,f32::NEG_INFINITY,0.],[-0.0031308,0.0031308,10.,0.5]] {let v=LinearRgbaView{width:1,height:1,samples:&p};for space in [FilterColorSpace::LinearSrgb,FilterColorSpace::EncodedSrgb]{assert_eq!(baseline::run(&v,space).map(|p|p.map(f64::to_bits)),candidate::run(&v,space).map(|p|p.map(f64::to_bits)));checked+=1;}}println!("{} bit-identical controls",checked);}
