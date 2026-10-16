//! Paired full-decoder experiment; snapshots differ only in Control Vec reservation.
#![allow(dead_code, clippy::pedantic)]
pub use rrrah_animation::*;
mod baseline;
mod candidate;
use std::{alloc::{GlobalAlloc, Layout, System}, cell::Cell, io::Write, time::Instant};

#[derive(Clone, Copy, Default)]
struct Counts { allocs: u64, reallocs: u64, live: i64, peak: i64, invalid: bool }
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static COUNTS: Cell<Counts> = const { Cell::new(Counts { allocs:0,reallocs:0,live:0,peak:0,invalid:false }) };
}
fn account(delta: i64, alloc: bool, realloc: bool) {
    if ENABLED.try_with(Cell::get).unwrap_or(false) {
        let _ = COUNTS.try_with(|slot| {
            let mut s = slot.get();
            s.allocs += u64::from(alloc); s.reallocs += u64::from(realloc);
            s.live += delta; s.peak = s.peak.max(s.live); s.invalid |= s.live < 0;
            slot.set(s);
        });
    }
}
struct Meter;
#[global_allocator] static ALLOCATOR: Meter = Meter;
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() { account(l.size() as i64, true, false); } p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() { account(l.size() as i64, true, false); } p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        account(-(l.size() as i64), false, false); unsafe { System.dealloc(p, l); }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let new = unsafe { System.realloc(p, l, n) };
        if !new.is_null() { account(n as i64 - l.size() as i64, false, true); } new
    }
}
fn chunk(out: &mut Vec<u8>, name: &[u8;4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(name); out.extend_from_slice(data);
    let mut crc = flate2::Crc::new(); crc.update(name); crc.update(data);
    out.extend_from_slice(&crc.sum().to_be_bytes());
}
fn fixture(n: u32, edge: u32, invalid_second: bool) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new(); header.extend_from_slice(&edge.to_be_bytes());
    header.extend_from_slice(&edge.to_be_bytes()); header.extend_from_slice(&[8,6,0,0,0]);
    chunk(&mut out,b"IHDR",&header);
    let mut animation = n.to_be_bytes().to_vec(); animation.extend_from_slice(&0u32.to_be_bytes());
    chunk(&mut out,b"acTL",&animation);
    let mut raw = Vec::new();
    for _ in 0..edge { raw.push(0); for _ in 0..edge { raw.extend_from_slice(&[24,96,170,192]); } }
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(),flate2::Compression::default());
    encoder.write_all(&raw).unwrap(); let encoded = encoder.finish().unwrap();
    let mut sequence = 0u32;
    for i in 0..n {
        let mut control = Vec::new(); control.extend_from_slice(&sequence.to_be_bytes()); sequence += 1;
        control.extend_from_slice(&(if invalid_second && i==1 {0} else {edge}).to_be_bytes());
        control.extend_from_slice(&edge.to_be_bytes()); control.extend_from_slice(&[0;8]);
        control.extend_from_slice(&1u16.to_be_bytes()); control.extend_from_slice(&100u16.to_be_bytes());
        control.extend_from_slice(&[0,0]); chunk(&mut out,b"fcTL",&control);
        if i==0 { chunk(&mut out,b"IDAT",&encoded); } else {
            let mut data = sequence.to_be_bytes().to_vec(); sequence += 1;
            data.extend_from_slice(&encoded); chunk(&mut out,b"fdAT",&data);
        }
    }
    chunk(&mut out,b"IEND",&[]); out
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Summary { ok: bool, digest: [u8;32], managed_peak: u64, final_managed: u64 }
fn decode(which: bool, bytes: &[u8], index: usize, budget_limit: u64, cancelled: bool, hash_all: bool) -> Summary {
    let budget = rrrah_core::MemoryBudget::new(budget_limit);
    let mut request = DecodeRequest::new("synthetic.apng"); request.image_index=index;
    request.assume_untagged_srgb=true; request.memory_budget=Some(budget.clone());
    if cancelled {
        request.cancellation=Some(GenerationToken::new(std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2)),1));
    }
    let mut hash = blake3::Hasher::new();
    // Convert the distinct snapshot wrapper types to one actual raster contract.
    let result = if which {
        candidate::decode(bytes,&request).map(|v|(v.raster,v.delay_ms_numerator,v.delay_ms_denominator,v.num_plays))
    } else {
        baseline::decode(bytes,&request).map(|v|(v.raster,v.delay_ms_numerator,v.delay_ms_denominator,v.num_plays))
    };
    let ok = match result {
        Ok((raster,num,den,plays)) => {
            std::hint::black_box(&raster);
            for v in [raster.width(),raster.height(),num,den,plays] { hash.update(&v.to_le_bytes()); }
            if hash_all {
                let rrrah_core::RasterPixels::Rgba32Float(values)=raster.pixels() else { panic!("unexpected output type") };
                for v in values.iter() { hash.update(&v.to_bits().to_le_bytes()); }
            }
            true
        }
        Err(error) => { hash.update(error.to_string().as_bytes()); false }
    };
    Summary { ok, digest:*hash.finalize().as_bytes(),managed_peak:budget.peak(),final_managed:budget.used() }
}
fn resources() -> (u64,u64) {
    let mut r = std::mem::MaybeUninit::<libc::rusage>::uninit();
    assert_eq!(unsafe{libc::getrusage(libc::RUSAGE_SELF,r.as_mut_ptr())},0);
    let r = unsafe{r.assume_init()};
    let cpu = (r.ru_utime.tv_sec+r.ru_stime.tv_sec) as u64*1_000_000_000
        +(r.ru_utime.tv_usec+r.ru_stime.tv_usec) as u64*1000;
    let rss = r.ru_maxrss as u64 * if cfg!(target_os="linux") {1024} else {1}; (cpu,rss)
}
fn main() {
    let args:Vec<_>=std::env::args().collect();
    let which=args[1]=="candidate";let count:u32=args[2].parse().unwrap();let case=&args[3];
    let bytes = if case=="empty" {Vec::new()} else {fixture(count,if case=="large" {32}else{1},case=="invalid")};
    let mut bytes=bytes; if case=="truncated" {bytes.truncate(bytes.len()-5);}
    let index=(count as usize).saturating_sub(1);let limit=if case=="no_memory" {0}else{8*1024*1024};
    let small = fixture(16,1,false);
    let workload = |hash_all| {
        let large = decode(which,&bytes,index,limit,case=="cancelled",hash_all);
        if case != "alternating" { return large; }
        let small = decode(which,&small,15,limit,false,hash_all);
        let mut hash = blake3::Hasher::new(); hash.update(&large.digest); hash.update(&small.digest);
        Summary {ok:large.ok && small.ok,digest:*hash.finalize().as_bytes(),
            managed_peak:large.managed_peak.max(small.managed_peak),final_managed:large.final_managed+small.final_managed}
    };
    for _ in 0..3 {std::hint::black_box(workload(true));}
    COUNTS.with(|v|v.set(Counts::default())); ENABLED.with(|v|v.set(true));
    let summary=workload(true);
    ENABLED.with(|v|v.set(false));let stats=COUNTS.with(Cell::get);
    assert!(!stats.invalid && stats.live==0,"allocation balance invalid: {}",stats.live);
    assert_eq!(summary.final_managed,0);
    let mut samples=Vec::new(); let repeats=32;
    for _ in 0..10 {
        let (cpu0,_)=resources(); let wall=Instant::now();
        for _ in 0..repeats { std::hint::black_box(workload(false)); }
        let elapsed=wall.elapsed().as_nanos() as u64;let(cpu1,rss)=resources();
        samples.push(serde_json::json!({"wall_ns":elapsed/repeats,"cpu_ns":(cpu1-cpu0)/repeats,"rss_peak_bytes":rss}));
    }
    println!("{}",serde_json::json!({"variant":args[1],"count":count,"case":case,"input_bytes":bytes.len(),"decodes_per_workload":if case=="alternating" {2}else{1},"ok":summary.ok,
        "output_or_error_hash":blake3::Hash::from(summary.digest).to_hex().to_string(),"managed_peak_bytes":summary.managed_peak,
        "final_managed_bytes":summary.final_managed,"allocation_calls":stats.allocs,"realloc_calls":stats.reallocs,
        "tracked_peak_live_bytes":stats.peak,"balanced_live_bytes":stats.live,"samples":samples}));
}
