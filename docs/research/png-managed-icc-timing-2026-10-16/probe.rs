// Test-only paired metadata-route timing; no production instrumentation.
use image::ImageDecoder;
use rrrah_decode::{DecodeError,DecodeRequest};
mod raster {pub use rrrah_decode::RasterDecodeError;pub const MAX_RASTER_BYTES:u64=512*1024*1024;}
#[path="../../../crates/rrrah-decode/src/png_profile.rs"]
mod native;
fn run(source:&[u8],output:&mut [u8],request:&DecodeRequest,new:bool)->(Vec<u8>,Option<rrrah_core::Reservation>) {
    let mut limits=image::Limits::default();limits.max_alloc=Some(512*1024*1024);
    if new {
        let profile=native::profile(source,request).unwrap().unwrap();
        let decoder=image::codecs::png::PngDecoder::with_limits_without_text_or_icc(std::io::Cursor::new(source),limits).unwrap();
        decoder.read_image(output).unwrap();profile
    } else {
        let mut decoder=image::codecs::png::PngDecoder::with_limits_without_text(std::io::Cursor::new(source),limits).unwrap();
        let profile=decoder.icc_profile().unwrap().unwrap();
        decoder.read_image(output).unwrap();(profile,None)
    }
}
fn main() {
    let path=std::env::args().nth(1).unwrap();let source=std::fs::read(&path).unwrap();
    let expected=std::fs::read("tests/fixtures/raster/pattern.png.rgba").unwrap();
    let mut output=vec![0;expected.len()];
    let mut request=DecodeRequest::new(&path);
    let budget=rrrah_core::MemoryBudget::new(32*1024*1024);request.memory_budget=Some(budget.clone());
    let reference=run(&source,&mut output,&request,false).0;assert_eq!(output,expected);
    for _ in 0..5 {for new in [false,true] {let result=run(&source,&mut output,&request,new);assert_eq!(result.0,reference);drop(result);assert_eq!(budget.used(),0);}}
    for sample in 0..40 {
        let order=if sample%2==0 {[false,true]} else {[true,false]};
        for new in order {
            let start=std::time::Instant::now();let result=run(&source,&mut output,&request,new);
            let elapsed=start.elapsed().as_nanos();
            assert_eq!(result.0,reference);assert_eq!(output,expected);
            drop(result);assert_eq!(budget.used(),0);
            println!("{{\"sample\":{sample},\"new\":{new},\"ns\":{elapsed},\"profile_bytes\":{},\"root_after_drop\":0}}",reference.len());
        }
    }
}
