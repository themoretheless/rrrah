extern crate self as rrrah_vector;
#[path="/Users/themoretheless/Documents/Sources/rrrah/crates/rrrah-vector/src/icc_colorimetry.rs"]
pub mod icc_colorimetry;
pub use rrrah_layered::{DecodeRequest,DecodeError,GenerationToken};
pub mod raster {pub use rrrah_layered::raster::*;}
#[path="/Users/themoretheless/Documents/Sources/rrrah/crates/rrrah-layered/src/psd.rs"]
mod psd;
#[path="/Users/themoretheless/Documents/Sources/rrrah/crates/rrrah-animation/src/raster_color.rs"]
mod raster_color;
#[path="/Users/themoretheless/Documents/Sources/rrrah/crates/rrrah-gpu/tests/common/mod.rs"]
mod common;
use rrrah_core::{MemoryBudget,RasterPixels};
fn main(){
 let fixtures=std::path::Path::new("/Users/themoretheless/Documents/Sources/rrrah/tests/fixtures/raster");
 let gpu=common::qualification_gpu().expect("actual Metal required");eprintln!("GRAY_COMPONENT adapter={}",gpu.adapter_name());
 let folder=tempfile::tempdir().unwrap();let root=MemoryBudget::new(8*1024*1024);
 let mut cache=rrrah_cache::RasterRamCache::new(rrrah_cache::CacheLimits{max_bytes:3*1024*1024,max_entries:Some(1),ttl:None});
 cache.enable_swap(rrrah_cache::RasterSwapCache::new_with_budgets(folder.path(),rrrah_cache::ImageSwapConfig{limits:rrrah_cache::CacheLimits{max_bytes:16*1024*1024,max_entries:Some(4),ttl:None},queue_bytes:3*1024*1024,queue_count:4,restore_bytes:6*1024*1024},MemoryBudget::new(3*1024*1024),root.child(6*1024*1024)).unwrap());
 let view=rrrah_gpu::ViewParameters{viewport:[400.,359.],zoom:1.,..Default::default()};
 let mut expected=Vec::new();let mut sdr=Vec::new();let mut hdr=Vec::new();
 for (key,ext) in ["psd","psb"].iter().enumerate(){
  let path=fixtures.join(format!("psd-tools-gray0.{ext}"));let bytes=std::fs::read(&path).unwrap();let mut request=DecodeRequest::new(path);request.memory_budget=Some(root.clone());
  let native=psd::decode(&bytes,&request).unwrap();let frame=raster_color::prepare_raster_for_display_with_budget(&native,Some(&root)).unwrap();drop(native);
  let RasterPixels::Rgba32Float(values)=frame.pixels() else {panic!()};expected.push(values.to_vec());
  sdr.push(gpu.render_raster(&frame,view,[400,359]).pixels);hdr.push(gpu.render_raster_half(&frame,view,[400,359]));
  cache.insert_visible(key,frame.clone());drop(frame);cache.swap().unwrap().wait_idle().unwrap();
 }
 let first=cache.get_with_cancel(&0,||false).unwrap();cache.mark_visible(0);cache.swap().unwrap().wait_idle().unwrap();
 let held=root.try_reserve(root.available_bytes()).unwrap();let hit=cache.get_with_cancel(&0,||false).unwrap();drop(hit);drop(held);
 let second=cache.get_background_with_cancel(&1,||false).unwrap();
 for (key,frame) in [&first,&second].into_iter().enumerate(){
  let RasterPixels::Rgba32Float(values)=frame.pixels() else {panic!()};assert_eq!(&values[..],&expected[key][..]);
  assert_eq!(gpu.render_raster(frame,view,[400,359]).pixels,sdr[key]);assert_eq!(gpu.render_raster_half(frame,view,[400,359]),hdr[key]);
 }
 drop(first);drop(second);cache.swap().unwrap().wait_idle().unwrap();let stats=cache.swap().unwrap().stats();assert_eq!(stats.errors,0);assert!(stats.writes>=2 && stats.reads>=2);drop(cache);assert_eq!(root.used(),0);
 eprintln!("GRAY_COMPONENT cases=2 pixels_per_frame=143600 readbacks=8 writes={} reads={} root_peak={} final=0",stats.writes,stats.reads,root.peak());
}
