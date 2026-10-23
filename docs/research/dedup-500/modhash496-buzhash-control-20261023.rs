// Tests rolling recurrence with a synthetic table, not upstream GEAR vectors.
mod gear {
 const fn table()->[u64;256]{let mut t=[0;256];let mut i=0;while i<256{t[i]=(i as u64).wrapping_mul(0x9e3779b97f4a7c15).rotate_left((i%64) as u32)^0xa5a5a5a5a5a5a5a5;i+=1;}t}
 pub const GEAR:[u64;256]=table();
}
#[path="evidence/n24q02m--modhash/02-buzhash.rs"] mod original;
fn oracle(data:&[u8])->u64{
 let window=&data[data.len().saturating_sub(64)..];
 window.iter().enumerate().fold(0,|out,(i,&byte)|out^gear::GEAR[byte as usize].rotate_left((window.len()-1-i) as u32))
}
fn main(){
 let mut bytes=Vec::new();let mut seed=0x123456789abcdef0u64;
 for _ in 0..4096 {seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;bytes.push(seed as u8);}
 let mut h=original::Buzhash64::new();assert!(h.is_empty());assert!(!h.is_full());assert_eq!(h.fingerprint(),0);
 for end in 1..=bytes.len(){assert_eq!(h.push(bytes[end-1]),oracle(&bytes[..end]));assert_eq!(h.len(),end.min(64));assert_eq!(h.is_full(),end>=64);assert!(!h.is_empty());}
 let mut variants=0;
 for block in [1,2,3,7,8,31,63,64,65,127,1024,4096] {
  let mut split=original::Buzhash64::new();for chunk in bytes.chunks(block){split.update(chunk);}
  assert_eq!(split.fingerprint(),h.fingerprint());let mut cloned=split.clone();assert_eq!(cloned.push(17),split.push(17));
  split.reset();assert!(split.is_empty());assert_eq!(split.len(),0);assert!(!split.is_full());assert_eq!(split.fingerprint(),0);assert_eq!(split.update(&bytes),h.fingerprint());variants+=1;
 }
 println!("4096 per-byte independent window oracles and {variants} split/clone/reset variants passed with synthetic GEAR table");
}
