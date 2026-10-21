// Research extraction: original full_hash body unchanged, explicit hash/log adapters.
// Original source MIT Copyright (c) 2020 Jacques Rimbault; see upstream LICENSE.
extern crate self as log;
#[macro_export] macro_rules! error { ($($x:tt)*) => { eprintln!($($x)*); }; }
#[path="/Users/themoretheless/Documents/Sources/rrrah/.dedup-work/yadf277-20261021/src/units.rs"] mod units;
#[path="/Users/themoretheless/Documents/Sources/rrrah/.dedup-work/yadf277-20261021/src/hasher.rs"] mod hasher;

use units::Bytes;
struct Candidate { path: std::path::PathBuf, size: Bytes }
enum HashUpdate<T> { Hash(T), Verified, Unreadable }
mod hash {
 pub const BLOCK: super::Bytes = super::Bytes::kib(4);
 pub fn full<H: crate::hasher::Hasher>(p:&std::path::Path)->std::io::Result<H::Hash>{
  let bytes=std::fs::read(p)?;let mut h=H::default();h.write(&bytes);Ok(h.finish())
 }
}
#[derive(Default)] struct Probe;
impl hasher::Hasher for Probe {type Hash=usize;fn write(&mut self,_:&[u8]){}fn finish(self)->usize{0}}
fn full_hash<H>(candidate: &Candidate) -> HashUpdate<H::Hash>
where
    H: crate::hasher::Hasher,
{
    if candidate.size < hash::BLOCK {
        // Its partial hash already covered the whole content plus the
        // size: nothing more to distinguish it by.
        return HashUpdate::Verified;
    }
    match hash::full::<H>(&candidate.path) {
        Ok(hash) => HashUpdate::Hash(hash),
        Err(error) => {
            log::error!(
                "{}, couldn't hash {:?}, excluding it from the results",
                error,
                candidate.path
            );
            HashUpdate::Unreadable
        }
    }
}
fn main(){
 let root=std::env::temp_dir().join(format!("rrrah-yadf277-probe-{}",std::process::id()));std::fs::create_dir(&root).unwrap();
 let p=root.join("small");std::fs::write(&p,b"aaa").unwrap();let c=Candidate{path:p.clone(),size:Bytes::new(3)};
 std::fs::write(&p,b"bbb").unwrap();assert!(matches!(full_hash::<Probe>(&c),HashUpdate::Verified));println!("PASS changed small file marked Verified without hash adapter call");
 std::fs::remove_file(&p).unwrap();assert!(matches!(full_hash::<Probe>(&c),HashUpdate::Verified));println!("PASS missing small file marked Verified");
 let c=Candidate{path:p,size:Bytes::new(4096)};assert!(matches!(full_hash::<Probe>(&c),HashUpdate::Unreadable));println!("PASS missing boundary-size file excluded");std::fs::remove_dir(root).unwrap();
}
