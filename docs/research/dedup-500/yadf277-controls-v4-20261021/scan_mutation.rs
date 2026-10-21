// Authored deterministic checkpoint using public original yadf::Hasher API.
// Standard DefaultHasher hashes all supplied bytes; no forced digest collision.
use std::{hash::Hasher as _, path::PathBuf, sync::{OnceLock, atomic::{AtomicUsize,Ordering}}};
static FINISHES:AtomicUsize=AtomicUsize::new(0);
static CHANGE:OnceLock<PathBuf>=OnceLock::new();
#[derive(Default)] struct CheckpointHasher(std::collections::hash_map::DefaultHasher);
impl yadf::Hasher for CheckpointHasher {
 type Hash=u64;
 fn write(&mut self,b:&[u8]){self.0.write(b)}
 fn finish(self)->u64{
  let digest=self.0.finish();
  if FINISHES.fetch_add(1,Ordering::SeqCst)==1 {std::fs::write(CHANGE.get().unwrap(),b"bbb").unwrap();}
  digest
 }
}
fn main(){
 let root=std::env::temp_dir().join(format!("rrrah-yadf277-public-scan-{}",std::process::id()));std::fs::create_dir(&root).unwrap();
 let a=root.join("a");let b=root.join("b");std::fs::write(&a,b"aaa").unwrap();std::fs::write(&b,b"aaa").unwrap();CHANGE.set(b.clone()).unwrap();
 let bag=yadf::Yadf::builder().paths([root.clone()].as_ref()).io_threads(1).build().scan::<CheckpointHasher>();
 assert_eq!(FINISHES.load(Ordering::SeqCst),2);
 assert_ne!(std::fs::read(&a).unwrap(),std::fs::read(&b).unwrap());
 let duplicates=bag.duplicates();let groups:Vec<_>=duplicates.iter().collect();assert_eq!(groups.len(),1);assert_eq!(groups[0].len(),2);
 println!("PASS original public scanner reports one2file duplicate group after same-length mutation; fresh bytes differ; two real hash finishes, no forced collision");
 std::fs::remove_dir_all(root).unwrap();
}
