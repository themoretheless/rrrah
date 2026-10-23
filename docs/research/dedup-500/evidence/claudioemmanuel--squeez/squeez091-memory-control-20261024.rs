use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
struct Meter;
fn add(n: usize) { let live = LIVE.fetch_add(n, Relaxed) + n; PEAK.fetch_max(live, Relaxed); }
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 { let p = System.alloc(l); if !p.is_null() { add(l.size()); } p }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 { let p = System.alloc_zeroed(l); if !p.is_null() { add(l.size()); } p }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { System.dealloc(p,l); LIVE.fetch_sub(l.size(), Relaxed); }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = System.realloc(p,l,n); if !q.is_null() { if n>=l.size() { add(n-l.size()); } else { LIVE.fetch_sub(l.size()-n,Relaxed); } } q
    }
}
#[global_allocator] static ALLOC: Meter = Meter;
fn main() {
    println!("{{\"scope\":\"Original library hash function, process allocator requested bytes; excludes input construction from measured delta, not RSS or speed\",\"cases\":[");
    for (i,n) in [1000usize,10000,100000].into_iter().enumerate() {
        let input=(0..n).map(|j|format!("token{j}")).collect::<Vec<_>>().join(" ");
        let base=LIVE.load(Relaxed); PEAK.store(base,Relaxed);
        let sketch=squeez::context::hash::shingle_minhash(&input);
        let peak=PEAK.load(Relaxed)-base; let retained=LIVE.load(Relaxed)-base;
        assert_eq!(sketch.len(),96); assert!(sketch.capacity()>=n-2);
        println!("{}{{\"tokens\":{},\"input_bytes\":{},\"sketch_len\":{},\"sketch_capacity\":{},\"retained_requested_bytes\":{},\"peak_additional_requested_bytes\":{}}}",if i==0{""}else{","},n,input.len(),sketch.len(),sketch.capacity(),retained,peak);
    }
    println!("]}}");
}
