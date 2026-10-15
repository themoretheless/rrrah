use std::{cell::Cell,sync::{Arc,atomic::{AtomicU64,Ordering}}};
use rrrah_decode::{DecodeRequest,GenerationToken};
include!("original-guards.rs");
fn owner_requests(requests:&[(u64,DecodeRequest)])->&[(u64,DecodeRequest)]{
 if requests.iter().any(|(_,r)|r.cancellation.is_some()){requests}else{&[]}
}
#[test]
fn owner_presence_user_cancel_and_latch_match_original(){
 for n in [0,1,8,128]{for kind in 0..3{for generation_value in [1,2]{for user_cancel in [false,true]{for initially_latched in [false,true]{
  let generation=Arc::new(AtomicU64::new(generation_value));
  let requests:Vec<_>=(0..n).map(|i|{let mut r=DecodeRequest::new("unused");if kind==1||(kind==2&&i%2==0){r.cancellation=Some(GenerationToken::new(generation.clone(),1));}(i as u64,r)}).collect();
  let selected=owner_requests(&requests);assert_eq!(selected.len(),if requests.iter().any(|(_,r)|r.cancellation.is_some()){n}else{0});
  let a=Cell::new(initially_latched);let b=Cell::new(initially_latched);let ca=Cell::new(0);let cb=Cell::new(0);
  let expected=stopped(&requests,&||{ca.set(ca.get()+1);user_cancel},&a);
  let actual=stopped(selected,&||{cb.set(cb.get()+1);user_cancel},&b);
  assert_eq!((actual,b.get(),cb.get()),(expected,a.get(),ca.get()));
  let a=Cell::new(initially_latched);let b=Cell::new(initially_latched);assert_eq!(owners_stopped(&requests,&a),owners_stopped(selected,&b));assert_eq!(a.get(),b.get());
 }}}}}
}
#[test]
fn user_checkpoint_generation_mutations_and_reset_stay_equivalent(){
 for kind in 0..3{for n in [1,8,64]{
  let generation=Arc::new(AtomicU64::new(1));
  let requests:Vec<_>=(0..n).map(|i|{let mut r=DecodeRequest::new("unused");if kind==1||(kind==2&&i==n-1){r.cancellation=Some(GenerationToken::new(generation.clone(),1));}(i as u64,r)}).collect();
  let selected=owner_requests(&requests);let a=Cell::new(false);let b=Cell::new(false);
  generation.store(1,Ordering::Release);let expected=stopped(&requests,&||{generation.store(2,Ordering::Release);false},&a);
  generation.store(1,Ordering::Release);let actual=stopped(selected,&||{generation.store(2,Ordering::Release);false},&b);assert_eq!((actual,b.get()),(expected,a.get()));
  generation.store(1,Ordering::Release);assert_eq!(owners_stopped(&requests,&a),owners_stopped(selected,&b));assert_eq!(a.get(),b.get());
 }}
}
#[test]
#[ignore="manual scalar owner-check benchmark; not whole collection performance"]
fn checkpoint_loop(){
 let n:usize=std::env::var("OWNER_BENCH_FILES").unwrap().parse().unwrap();let kind=std::env::var("OWNER_BENCH_KIND").unwrap();let candidate=std::env::var("OWNER_BENCH_VARIANT").unwrap()=="candidate";assert!([2,8,64,256].contains(&n)&&["none","all","mixed"].contains(&kind.as_str()));
 let generation=Arc::new(AtomicU64::new(1));let requests=std::hint::black_box((0..n).map(|i|{let mut r=DecodeRequest::new("unused");if kind=="all"||(kind=="mixed"&&i==n-1){r.cancellation=Some(GenerationToken::new(generation.clone(),1));}(i as u64,r)}).collect::<Vec<_>>());
 let calls=Cell::new(0u64);let callback=||{calls.set(calls.get()+1);false};let cancel:&dyn Fn()->bool=std::hint::black_box(&callback);let latch=Cell::new(false);
 let timer=std::time::Instant::now();let observed=if candidate{owner_requests(&requests)}else{requests.as_slice()};
 for _ in 0..1_000_000{assert!(!std::hint::black_box(stopped(std::hint::black_box(observed),cancel,&latch)));}
 assert_eq!(calls.get(),1_000_000);assert!(!latch.get());println!("OWNER_BENCH_NS={} CALLS={}",timer.elapsed().as_nanos(),calls.get());
}
