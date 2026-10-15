#[path="exact.rs"] pub mod exact;
use std::{fs::OpenOptions,io::{Seek,SeekFrom,Write},cell::Cell};
fn main(){
 let root=tempfile::tempdir().unwrap();let a=root.path().join("a.bin");let b=root.path().join("b.bin");
 let length=u64::from(u32::MAX)+65_537;assert_eq!(length,4_294_967_296+65_536);
 for path in [&a,&b]{let mut f=OpenOptions::new().write(true).create_new(true).open(path).unwrap();f.set_len(length).unwrap();for (offset,bytes)in [(0,b"start!!!"),(4_294_967_296,b"over4GiB"),(length-8,b"last!!!!")]{f.seek(SeekFrom::Start(offset)).unwrap();f.write_all(bytes).unwrap();}f.sync_all().unwrap();}
 let checkpoints=Cell::new(0u64);let timer=std::time::Instant::now();
 let equal=exact::compare_files(&a,&b,length,||{checkpoints.set(checkpoints.get()+1);false}).unwrap();assert!(equal);let equal_ns=timer.elapsed().as_nanos();
 {let mut f=OpenOptions::new().write(true).open(&b).unwrap();f.seek(SeekFrom::Start(length-1)).unwrap();f.write_all(b"?").unwrap();f.sync_all().unwrap();}
 let timer=std::time::Instant::now();let different=exact::compare_files(&a,&b,length,||{checkpoints.set(checkpoints.get()+1);false}).unwrap();assert!(!different);assert!(checkpoints.get()>0);let different_ns=timer.elapsed().as_nanos();
 #[cfg(unix)]let blocks={use std::os::unix::fs::MetadataExt;std::fs::metadata(&a).unwrap().blocks()+std::fs::metadata(&b).unwrap().blocks()};
 #[cfg(not(unix))]let blocks=0u64;
 println!("LENGTH={length} EQUAL={equal} LATE_DIFFERENT={different} CHECKPOINTS={} EQUAL_NS={equal_ns} DIFFERENT_NS={different_ns} ALLOCATED_512_BLOCKS={blocks}",checkpoints.get());
}
