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
 let bytes=std::fs::read("tests/fixtures/raster/avif-alpha-icc-prem.avif").unwrap();
 let plain=avif_color::properties_with_budget(&bytes,&||Ok(()),None).unwrap();
 let n=match plain.color {Some(rrrah_core::RasterColorSpace::Icc(ref v))=>v.len() as u64,_=>panic!("missing ICC")};drop(plain);
 let short=rrrah_core::MemoryBudget::new(n-1);
 assert!(matches!(avif_color::properties_with_budget(&bytes,&||Ok(()),Some(&short)),Err(raster::RasterDecodeError::Source(DecodeError::Memory(_)))));
 assert_eq!(short.used(),0);
 let root=rrrah_core::MemoryBudget::new(n);
 let props=avif_color::properties_with_budget(&bytes,&||Ok(()),Some(&root)).unwrap();
 assert_eq!(root.used(),n);drop(props);assert_eq!(root.used(),0);
 let cancel=||if root.used()>0{Err(raster::RasterDecodeError::InvalidAvif("cancel after ICC allocation"))}else{Ok(())};
 assert!(avif_color::properties_with_budget(&bytes,&cancel,Some(&root)).is_err());assert_eq!(root.used(),0);
 let retry=avif_color::properties_with_budget(&bytes,&||Ok(()),Some(&root)).unwrap();assert_eq!(root.used(),n);drop(retry);assert_eq!(root.used(),0);
 println!("icc_bytes={n}; one_byte_short_refused; exact_budget_admitted; post_copy_cancel_released; retry_passed; root=0");
}
