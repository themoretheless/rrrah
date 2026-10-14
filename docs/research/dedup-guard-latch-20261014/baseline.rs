use std::{cell::Cell,sync::{Arc,atomic::{AtomicU64,Ordering}},hint::black_box};
#[derive(Debug, Clone)]
pub struct GenerationToken {
    generation: Arc<AtomicU64>,
    expected: u64,
    additional: Arc<[(Arc<AtomicU64>, u64)]>,
}

impl GenerationToken {
    pub fn new(generation: Arc<AtomicU64>, expected: u64) -> Self {
        Self {
            generation,
            expected,
            additional: Arc::from([]),
        }
    }

    /// Cancellation from either owner terminates the shared operation. Flattened
    /// conditions avoid recursive token traversal and deduplicate identical owners.
    pub fn combine(&self, other: &Self) -> Self {
        let mut additional = self.additional.to_vec();
        for condition in std::iter::once((&other.generation, other.expected)).chain(
            other
                .additional
                .iter()
                .map(|(owner, expected)| (owner, *expected)),
        ) {
            if Arc::ptr_eq(&self.generation, condition.0) && self.expected == condition.1 {
                continue;
            }
            if !additional
                .iter()
                .any(|(owner, expected)| Arc::ptr_eq(owner, condition.0) && *expected == condition.1)
            {
                additional.push((Arc::clone(condition.0), condition.1));
            }
        }
        Self {
            generation: Arc::clone(&self.generation),
            expected: self.expected,
            additional: additional.into(),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.generation.load(Ordering::Acquire) != self.expected
            || self
                .additional
                .iter()
                .any(|(owner, expected)| owner.load(Ordering::Acquire) != *expected)
    }
}

struct Request { cancellation: Option<GenerationToken> }
#[inline(always)] fn poll(latch:&Cell<bool>,cancel:&dyn Fn()->bool,left:&Request,right:&Request)->bool {         let stop = latch.get()
            || cancel()
            || [left, right]
                .iter()
                .any(|r| r.cancellation.as_ref().is_some_and(GenerationToken::is_cancelled));
        latch.set(stop);
        stop }
fn exercise(depth:usize,mode:usize,reps:usize,stop:usize,kind:usize)->(usize,usize){
 let a=Arc::new(AtomicU64::new(0));let b=Arc::new(AtomicU64::new(0));let c=Arc::new(AtomicU64::new(0));let ta=GenerationToken::new(a.clone(),0);let tb=GenerationToken::new(b.clone(),0);let tc=GenerationToken::new(c.clone(),0);
 let left=Request{cancellation:match mode{0=>None,1=>Some(ta.clone()),_=>Some(ta.combine(&tc))}};let right=Request{cancellation:match mode{0=>None,1=>Some(tb.clone()),_=>Some(tb.combine(&tc))}};
 let calls=Cell::new(0usize);let root=||{calls.set(calls.get()+1);if stop>0&&calls.get()==stop{match kind{0=>return true,1=>a.store(1,Ordering::Release),2=>b.store(1,Ordering::Release),_=>c.store(1,Ordering::Release)}}false};
 let l1=Cell::new(false);let one=||poll(&l1,black_box(&root),&left,&right);let l2=Cell::new(false);let two=||poll(&l2,black_box(&one),&left,&right);let l3=Cell::new(false);let three=||poll(&l3,black_box(&two),&left,&right);
 let target:&dyn Fn()->bool=if depth==1{black_box(&one)}else{black_box(&three)};let mut accepted=0;for _ in 0..reps{accepted+=usize::from(black_box(target()));}(calls.get(),accepted)
}
fn main(){let args:Vec<_>=std::env::args().collect();if args[1]=="controls"{let mut n=0;for depth in [1,3]{for mode in 0..3{for stop in 1..=17{for kind in 0..(if mode==0{1}else if mode==1{3}else{4}){let(c,t)=exercise(depth,mode,40,stop,kind);assert_eq!(c,stop);assert_eq!(t,41-stop);n+=1;}}}}println!("{{\"controls\":{n}}}");return;}let depth=args[1].parse().unwrap();let mode=args[2].parse().unwrap();let reps=args[3].parse().unwrap();let(c,t)=exercise(depth,mode,reps,0,0);assert_eq!(c,reps);assert_eq!(t,0);println!("{{\"calls\":{c},\"cancelled\":{t}}}");}
