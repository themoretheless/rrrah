// Test-only allocator instrumentation; never linked into rrrah production.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst};
use image::{ImageDecoder, Limits};
static ENABLED: AtomicBool = AtomicBool::new(false);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);
struct Meter;
fn added(bytes: usize) {
    if ENABLED.load(SeqCst) {
        let live = LIVE.fetch_add(bytes, SeqCst) + bytes;
        PEAK.fetch_max(live, SeqCst);
        CALLS.fetch_add(1, SeqCst);
    }
}
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() { added(layout.size()); }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() { added(layout.size()); }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ENABLED.load(SeqCst) { LIVE.fetch_sub(layout.size(), SeqCst); }
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(ptr, layout, size) };
        if !result.is_null() && ENABLED.load(SeqCst) {
            LIVE.fetch_sub(layout.size(), SeqCst);
            added(size);
        }
        result
    }
}
#[global_allocator]
static ALLOCATOR: Meter = Meter;
fn measure(operation: impl FnOnce() -> bool) -> (bool, usize, usize, usize) {
    LIVE.store(0, SeqCst); PEAK.store(0, SeqCst); CALLS.store(0, SeqCst);
    ENABLED.store(true, SeqCst);
    let ok = operation();
    ENABLED.store(false, SeqCst);
    (ok, PEAK.load(SeqCst), LIVE.load(SeqCst), CALLS.load(SeqCst))
}
use rrrah_decode::DecodeRequest;
mod raster {pub use rrrah_decode::RasterDecodeError;}
#[path="../../../crates/rrrah-decode/src/png_source.rs"]
mod native;
fn main() {
    let path=std::env::args().nth(1).unwrap();let source=std::fs::read(&path).unwrap();
    let expected=std::fs::read("tests/fixtures/raster/pattern.png.rgba").unwrap();
    let mut output=vec![0;expected.len()];let request=DecodeRequest::new(&path);
    let (ok,peak,live,calls)=measure(||native::prepare(&source,&request).is_ok());
    assert!(ok);assert_eq!((peak,live,calls),(0,0,0));
    println!("{{\"phase\":\"native_prepare\",\"peak\":{peak},\"live\":{live},\"alloc_calls\":{calls}}}");
    for virtual_source in [false,true] {
        let (ok,peak,live,calls)=measure(|| {
            let mut limits=Limits::default();limits.max_alloc=Some(32*1024*1024);
            if virtual_source {
                let (source,orientation)=native::prepare(&source,&request).unwrap();
                assert_eq!(orientation,image::metadata::Orientation::Rotate90);
                let decoder=image::codecs::png::PngDecoder::with_limits_without_text_or_icc(source,limits).unwrap();
                decoder.read_image(&mut output).is_ok()
            } else {
                let mut decoder=image::codecs::png::PngDecoder::with_limits_without_text_or_icc(std::io::Cursor::new(&source),limits).unwrap();
                assert_eq!(decoder.orientation().unwrap(),image::metadata::Orientation::Rotate90);
                decoder.read_image(&mut output).is_ok()
            }
        });
        assert!(ok);assert_eq!(live,0);assert_eq!(output,expected);
        println!("{{\"virtual_source\":{virtual_source},\"peak\":{peak},\"live\":{live},\"alloc_calls\":{calls}}}");
    }
}
