#[derive(Debug)] enum DecodeError { Memory(rrrah_core::BufferError) }
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
mod raster{#[derive(Debug)]pub enum RasterDecodeError{InvalidAvif(&'static str),Source(crate::DecodeError)}}
struct DecodeRequest;
impl DecodeRequest{fn check_cancelled(&self)->Result<(),raster::RasterDecodeError>{Ok(())}}
#[path="../../../crates/rrrah-decode/src/avif_color.rs"] mod avif_color;
fn bx(tag:&[u8;4],body:&[u8])->Vec<u8>{let mut b=((body.len()+8)as u32).to_be_bytes().to_vec();b.extend(tag);b.extend(body);b}
fn main(){
 let mut aux=2u16.to_be_bytes().to_vec();aux.extend(40000u16.to_be_bytes());
 for _ in 0..40000 {aux.extend(1u16.to_be_bytes());}
 let refs=[vec![0;4],bx(b"auxl",&aux),bx(b"auxl",&aux),bx(b"prem",&[0,1,0,1,0,2])].concat();
 let alpha=[vec![0;4],b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha\0".to_vec()].concat();
 let iprp=[bx(b"ipco",&bx(b"auxC",&alpha)),bx(b"ipma",&[0,0,0,0,0,0,0,1,0,2,1,1])].concat();
 let meta=[vec![0;4],bx(b"pitm",&[0,0,0,0,0,1]),bx(b"iref",&refs),bx(b"iprp",&iprp)].concat();
 let bytes=[bx(b"ftyp",b"avif\0\0\0\0avif"),bx(b"meta",&meta)].concat();
 drop(aux);drop(refs);drop(alpha);drop(iprp);drop(meta);
 let before=USED.load(Ordering::Relaxed);PEAK.store(before,Ordering::Relaxed);
 let properties=avif_color::properties_with_budget(&bytes,&||Ok(()),None).unwrap();
 assert!(properties.premultiplied_alpha);drop(properties);
 let extra=PEAK.load(Ordering::Relaxed)-before;let retained=USED.load(Ordering::Relaxed)-before;
 println!("input_bytes={}; auxiliary_targets=80000; extra_rust_capacity_peak={extra}; retained_extra={retained}",bytes.len());
 assert!(extra<=4096);assert_eq!(retained,0);
}
