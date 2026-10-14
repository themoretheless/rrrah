// Rust allocation-capacity probe for the current metadata parser, not OS RSS.
use std::alloc::{GlobalAlloc,Layout,System};
use std::sync::atomic::{AtomicUsize,Ordering};
struct Counting;
static USED:AtomicUsize=AtomicUsize::new(0);
static PEAK:AtomicUsize=AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self,l:Layout)->*mut u8 {
        let p=unsafe{System.alloc(l)};
        if !p.is_null(){let n=USED.fetch_add(l.size(),Ordering::Relaxed)+l.size();PEAK.fetch_max(n,Ordering::Relaxed);}
        p
    }
    unsafe fn dealloc(&self,p:*mut u8,l:Layout){unsafe{System.dealloc(p,l)};USED.fetch_sub(l.size(),Ordering::Relaxed);}
    unsafe fn realloc(&self,p:*mut u8,l:Layout,size:usize)->*mut u8 {
        let q=unsafe{System.realloc(p,l,size)};
        if !q.is_null(){let n=USED.fetch_add(size,Ordering::Relaxed)+size;USED.fetch_sub(l.size(),Ordering::Relaxed);PEAK.fetch_max(n-l.size(),Ordering::Relaxed);}
        q
    }
}
#[global_allocator] static ALLOC:Counting=Counting;
mod raster{#[derive(Debug)]pub enum RasterDecodeError{InvalidAvif(&'static str)}}
struct DecodeRequest;
impl DecodeRequest{fn check_cancelled(&self)->Result<(),raster::RasterDecodeError>{Ok(())}}
#[path="../../../crates/rrrah-decode/src/avif_color.rs"] mod avif_color;
fn bx(tag:&[u8;4],body:&[u8])->Vec<u8>{let mut b=((body.len()+8)as u32).to_be_bytes().to_vec();b.extend(tag);b.extend(body);b}
fn main(){
    let mut assoc=vec![0;4];assoc.extend(2048u32.to_be_bytes());
    for _ in 0..2048 {assoc.extend(1u16.to_be_bytes());assoc.push(255);assoc.extend(std::iter::repeat_n(1,255));}
    let iprp=[bx(b"ipco",&bx(b"zzzz",&[])),bx(b"ipma",&assoc)].concat();
    let meta=[vec![0;4],bx(b"pitm",&[0,0,0,0,0,1]),bx(b"iprp",&iprp)].concat();
    let bytes=[bx(b"ftyp",b"avif\0\0\0\0avif"),bx(b"meta",&meta)].concat();
    drop(assoc);drop(iprp);drop(meta);
    let before=USED.load(Ordering::Relaxed);PEAK.store(before,Ordering::Relaxed);
    let properties=avif_color::properties(&bytes).unwrap();
    assert!(properties.color.is_none());assert!(properties.bit_depth.is_none());
    assert!(!properties.premultiplied_alpha);drop(properties);
    let extra=PEAK.load(Ordering::Relaxed)-before;
    let retained=USED.load(Ordering::Relaxed)-before;
    println!("input_bytes={}; primary_associations={}; extra_rust_capacity_peak={extra}; retained_extra={retained}",bytes.len(),2048*255);
    // This metadata-only malformed-image control has one actual property and
    // needs no retained per-association working array. It contains no AV1 media.
    assert!(extra<=65536,"metadata association working memory amplified beyond 64KiB for one property");
}
