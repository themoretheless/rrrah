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
fn main() {
    let source = std::fs::read(std::env::args().nth(1).unwrap()).unwrap();
    let mut limits = Limits::default(); limits.max_alloc = Some(32*1024*1024);
    let mut decoder = image::codecs::png::PngDecoder::with_limits_without_text(std::io::Cursor::new(&source), limits).unwrap();
    for borrowed in [false,true] {
        let (ok,peak,live,calls) = measure(|| {
            let orientation = if borrowed {decoder.orientation().unwrap()}
            else {decoder.exif_metadata().unwrap().and_then(|bytes| image::metadata::Orientation::from_exif_chunk(&bytes)).unwrap_or(image::metadata::Orientation::NoTransforms)};
            orientation == image::metadata::Orientation::Rotate90
        });
        assert!(ok); assert_eq!(live,0);
        println!("{{\"borrowed\":{borrowed},\"ok\":{ok},\"peak\":{peak},\"live\":{live},\"alloc_calls\":{calls}}}");
    }
    let before = decoder.exif_metadata().unwrap().unwrap();
    assert_eq!(decoder.orientation().unwrap(),image::metadata::Orientation::Rotate90);
    assert_eq!(decoder.exif_metadata().unwrap().unwrap(), before);
}
