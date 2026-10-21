from pathlib import Path
import subprocess
root=Path.cwd();source=root/'.dedup-work/zfs112-20261021/src/walk.rs'
s=source.read_text().split('// Not in rustix')[0]
s=s.replace('use anyhow::{Context, Result, bail};','type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;').replace('use rustix::fs::{FsWord, statfs, statvfs};','')
s+='''
use std::os::unix::ffi::OsStringExt;
#[test] fn original_paths_roundtrip_non_utf8() {
 let path=PathBuf::from(OsString::from_vec(vec![b'/',b'a',b'/',0xff,b'x']));
 let p:Paths<u32>=[(path.clone(),7)].into_iter().collect();
 assert_eq!(p.path(&p.files[0].0),path);
}
#[test] fn original_map_batches_preserve_paths_and_order() {
 let p:Paths<u32>=(0..7).map(|i|(PathBuf::from(format!("/root/{i}")),i)).collect();
 let mut counts=vec![];
 let q=p.map_batched(3,|_,batch|{counts.push(batch.len());Ok(batch.into_iter().map(|(p,n)|(p,n*2)).collect())}).unwrap();
 assert_eq!(counts,vec![3,3,1]);
 for (i,(p,n)) in q.files.iter().enumerate(){assert_eq!(*n,i as u32*2);assert_eq!(q.path(p),PathBuf::from(format!("/root/{i}")));}
}
#[test] fn zero_batch_discards_input_without_calling_step() {
 let p:Paths<u32>=[(PathBuf::from("/root/a"),7)].into_iter().collect();let mut calls=0;
 let q=p.map_batched(0,|_,batch|{calls+=1;Ok(batch)}).unwrap();
 assert_eq!(calls,0);assert!(q.files.is_empty());
}
'''
out=root/'.dedup-work/zfs112-path-controls-20261021.rs';out.write_text(s)
binary=root/'.dedup-work/zfs112-path-controls-20261021'
subprocess.run(['rustc','+1.98.1','--edition=2024','-Awarnings','--test',str(out),'-o',str(binary)],check=True)
with (root/'docs/research/dedup-500/zfs112-path-controls-v3-20261021/run.log').open('w') as f:subprocess.run([str(binary)],stdout=f,stderr=subprocess.STDOUT,check=True)
