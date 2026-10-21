use std::path::Path;
mod cache {pub struct Cache;}
mod image {
 pub struct Pixel(pub [u8;1]);
 pub struct DynamicImage {pub values:Vec<u8>}
 pub trait GenericImageView {fn dimensions(&self)->(u32,u32);fn pixels(&self)->std::vec::IntoIter<(u32,u32,Pixel)>;}
 impl GenericImageView for DynamicImage {
 fn dimensions(&self)->(u32,u32){(self.values.len() as u32,1)}
 fn pixels(&self)->std::vec::IntoIter<(u32,u32,Pixel)>{self.values.iter().enumerate().map(|(i,v)|(i as u32,0,Pixel([*v]))).collect::<Vec<_>>().into_iter()}
 }
}
mod hash {
 use super::{cache::Cache,image};use std::path::Path;
 pub enum HashType {AHash,DHash}
 pub struct Precision;
 pub struct PreparedImage {pub image:Option<image::DynamicImage>}
 pub trait PerceptualHash {fn get_hash(&self,c:&Option<Cache>)->u64;}
 pub fn prepare_image(p:&Path,_:&HashType,_:&Precision,_:&Option<Cache>)->PreparedImage {
 let values=match p.to_str().unwrap(){"first"=>{let mut v=vec![0;64];v[0]=255;v},"last"=>{let mut v=vec![0;64];v[63]=255;v},"dhash"=>vec![100,200,150,50],_=>return PreparedImage{image:None}};
 PreparedImage{image:Some(image::DynamicImage{values})}
 }
 pub mod ahash {include!(concat!(env!("PIHASH_SOURCE"),"/src/hash/ahash.rs"));}
 pub mod dhash {include!(concat!(env!("PIHASH_SOURCE"),"/src/hash/dhash.rs"));}
}
fn main(){use hash::PerceptualHash;let c=None;let p=hash::Precision;
 let a=|name|hash::ahash::AHash::new(Path::new(name),&p,&c).get_hash(&c);
 let d=hash::dhash::DHash::new(Path::new("dhash"),&p,&c).get_hash(&c);
 assert_eq!(a("first"),0);assert_eq!(a("last"),2);assert_eq!(a("missing"),0);assert_eq!(d,13);
 println!("{{\"original_hash_modules\":true,\"synthetic_image_adapter\":true,\"ahash_first_only\":0,\"ahash_last_only\":2,\"ahash_missing\":0,\"dhash_actual\":13,\"adjacent_scan_reference\":9}}");}
