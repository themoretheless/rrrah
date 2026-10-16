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
    let path = std::env::args().nth(1).unwrap();
    let source = std::fs::read(path).unwrap();
    let mut output = vec![0; 16*16*4]; // Caller output excluded from codec measurement.
    for limit in [0, 64, 1048576] {
        let (ok, peak, live, calls) = measure(|| {
            let mut limits = Limits::default(); limits.max_alloc = Some(limit);
            image::codecs::png::PngDecoder::with_limits(std::io::Cursor::new(&source), limits).is_ok()
        });
        assert_eq!(live, 0, "constructor allocations must all be freed");
        println!("{{\"phase\":\"constructor\",\"limit\":{limit},\"ok\":{ok},\"peak\":{peak},\"live\":{live},\"alloc_calls\":{calls}}}");
        if limit == 0 { assert!(!ok && peak > 0, "best-effort zero limit is not zero allocation"); }
    }
    let (ok, peak, live, calls) = measure(|| {
        let mut limits = Limits::default(); limits.max_alloc = Some(1048576);
        let decoder = image::codecs::png::PngDecoder::with_limits(std::io::Cursor::new(&source), limits).unwrap();
        assert_eq!(decoder.total_bytes(), output.len() as u64);
        decoder.read_image(&mut output).is_ok()
    });
    assert!(ok); assert_eq!(live,0);
    println!("{{\"phase\":\"full_decode_excluding_output\",\"ok\":{ok},\"peak\":{peak},\"live\":{live},\"alloc_calls\":{calls}}}");
    assert_eq!(output, std::fs::read(std::env::args().nth(2).unwrap()).unwrap());
}
