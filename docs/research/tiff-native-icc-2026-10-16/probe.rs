// Test-only allocator instrumentation; never linked into rrrah production.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst};
use image::ImageDecoder;
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
#[path="../../../crates/rrrah-decode/src/tiff_profile.rs"]
mod native;
fn main() {
    let source=std::fs::read(std::env::args().nth(1).unwrap()).unwrap();
    let request=DecodeRequest::new("profile.tif");
    let expected=native::profile(&source,&request).unwrap().unwrap();
    let mut decoder=image::codecs::tiff::TiffDecoder::new(std::io::Cursor::new(&source)).unwrap();
    for mode in ["legacy_getter","native_borrow","native_owned_copy"] {
        let (ok,peak,live,calls)=measure(|| {
            if mode=="legacy_getter" {decoder.icc_profile().unwrap().as_deref()==Some(expected)}
            else {let profile=native::profile(&source,&request).unwrap().unwrap();
                if mode=="native_borrow" {profile==expected} else {profile.to_vec()==expected}}
        });
        assert_eq!(live,0);
        if mode!="legacy_getter" {assert!(ok);}
        println!("{{\"mode\":\"{mode}\",\"profile_bytes\":{},\"ok\":{ok},\"peak\":{peak},\"live\":{live},\"alloc_calls\":{calls}}}",expected.len());
    }
    let path=std::env::args().nth(1).unwrap();
    let root=rrrah_memory::MemoryBudget::new((source.len()+expected.len()+16) as u64);
    let mut request=DecodeRequest::new(&path);request.memory_budget=Some(root.clone());
    let frame=rrrah_decode::decode_raster(&request).unwrap();
    assert_eq!((frame.width(),frame.height()),(1,2));
    assert_eq!(frame.color_profile_capacity_bytes(),expected.len() as u64);
    assert_eq!(root.used(),(expected.len()+8) as u64);
    assert_eq!(root.peak(),(source.len()+expected.len()+16) as u64);
    drop(frame);assert_eq!(root.used(),0);
    println!("{{\"mode\":\"public_exact_file_budget\",\"profile_bytes\":{},\"ok\":true,\"peak\":{},\"live\":0}}",expected.len(),root.peak());

}
