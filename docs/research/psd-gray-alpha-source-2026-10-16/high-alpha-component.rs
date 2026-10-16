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
fn high_precision_controls(gpu: &common::GpuReadback) {
 let fixtures=std::path::Path::new("/Users/themoretheless/Documents/Sources/rrrah/tests/fixtures/raster");
 let folder=tempfile::tempdir().unwrap();let root=MemoryBudget::new(1024*1024);
 let swap=rrrah_cache::RasterSwapCache::<usize>::new_with_budgets(folder.path(),rrrah_cache::ImageSwapConfig{limits:rrrah_cache::CacheLimits{max_bytes:1024*1024,max_entries:Some(16),ttl:None},queue_bytes:262144,queue_count:4,restore_bytes:262144},MemoryBudget::new(262144),root.child(262144)).unwrap();
 let view=rrrah_gpu::ViewParameters{viewport:[128.,64.],zoom:1.,..Default::default()};let mut key=0;
 for depth in [16,32] {for mode in ["rgba","la"] {for version in [1,2] {
  let suffix=if version==1 {"psd"} else {"psb"};let name=format!("psd-tools-alpha-{mode}-{depth}-v{version}.{suffix}");
  let path=fixtures.join(&name);let bytes=std::fs::read(&path).unwrap();let mut request=DecodeRequest::new(path);request.memory_budget=Some(root.clone());request.assume_untagged_linear_srgb=true;
  let native=psd::decode(&bytes,&request).unwrap();assert_eq!(native.color_space(),&rrrah_core::RasterColorSpace::LinearSrgb);
  let frame=raster_color::prepare_raster_for_display_with_budget(&native,Some(&root)).unwrap();drop(native);
  let oracle=std::fs::read(fixtures.join(format!("{name}.f32le"))).unwrap();let oracle=oracle.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect::<Vec<_>>();let channels=if mode=="rgba" {4} else {2};
  let RasterPixels::Rgba32Float(values)=frame.pixels() else {panic!()};let expected=values.to_vec();
  for pixel in 0..4 {for c in 0..4 {let reference=oracle[pixel*channels+if c==3 {channels-1} else if channels==2 {0} else {c}];assert!((values[pixel*4+c]-reference).abs()<=if depth==16 {1.6e-5} else {2e-6});}}
  let sdr=gpu.render_raster(&frame,view,[128,64]).pixels;let hdr=gpu.render_raster_half(&frame,view,[128,64]);
  if depth==32 {assert!(values.iter().any(|&v| v>1.));assert!(hdr.chunks_exact(4).any(|p|p[..3].iter().any(|&v| v>0x3c00 && v<0x7c00)));}
  swap.enqueue(key,frame);swap.wait_idle().unwrap();assert_eq!(root.used(),0);
  let restored=swap.try_get(&key,||false).unwrap().unwrap();let RasterPixels::Rgba32Float(actual)=restored.pixels() else {panic!()};assert_eq!(&actual[..],&expected[..]);
  assert_eq!(gpu.render_raster(&restored,view,[128,64]).pixels,sdr);assert_eq!(gpu.render_raster_half(&restored,view,[128,64]),hdr);
  drop(restored);assert_eq!(root.used(),0);key+=1;
 }}}
 let stats=swap.stats();assert_eq!((stats.writes,stats.reads,stats.errors),(8,8,0));drop(swap);assert_eq!(root.used(),0);
 eprintln!("HIGH_ALPHA_COMPONENT cases=8 readbacks=32 writes=8 reads=8 hdr_above_one=true final=0");
}
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
 high_precision_controls(&gpu);
 eprintln!("GRAY_COMPONENT cases=2 pixels_per_frame=143600 readbacks=8 writes={} reads={} root_peak={} final=0",stats.writes,stats.reads,root.peak());
}
