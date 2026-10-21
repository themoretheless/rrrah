from pathlib import Path
import subprocess
root=Path.cwd();dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-controls-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
s=(root/'.dedup-work/visualvault076-20261021/crates/core/src/duplicate_detector.rs').read_text()
def method(name):
 start=s.index('    async fn '+name);brace=s.index('{',start);depth=1;i=brace+1
 while depth:
  depth+=(s[i]=='{')-(s[i]=='}');i+=1
 return s[start:i]
code='use std::path::Path; use anyhow::Result; use sha2::{Digest,Sha256}; use tokio::{fs::File,io::{AsyncReadExt,BufReader}};\nstruct DuplicateDetector;\nimpl DuplicateDetector {\n'+method('calculate_file_hash')+'\n'+method('calculate_quick_hash')+'\n}\n'
code+='''
#[tokio::main(flavor="current_thread")]
async fn main()->Result<()> {
 let dir=tempfile::tempdir()?;
 for size in [4097usize,8192,16384] {
  let a=dir.path().join(format!("a-{size}"));let b=dir.path().join(format!("b-{size}"));
  let x=vec![7u8;size];let mut y=x.clone();y[4096]=9;
  tokio::fs::write(&a,&x).await?;tokio::fs::write(&b,&y).await?;
  assert_eq!(DuplicateDetector::calculate_quick_hash(&a,size as u64).await?,DuplicateDetector::calculate_quick_hash(&b,size as u64).await?);
  assert_ne!(DuplicateDetector::calculate_file_hash(&a).await?,DuplicateDetector::calculate_file_hash(&b).await?);
  println!("PASS original quick hash misses differing byte4096 at size{size}; original full SHA256 distinguishes");
 }
 let missing=dir.path().join("missing");
 assert_eq!(DuplicateDetector::calculate_quick_hash(&missing,0).await?,"empty");
 assert!(DuplicateDetector::calculate_file_hash(&missing).await.is_err());
 println!("PASS original quick hash accepts missing path with observed size0; full hash errors");Ok(())
}
'''
(dest/'src/main.rs').write_text(code);(dest/'Cargo.toml').write_text('[package]\nname="visualvault076-controls"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nanyhow="1"\nsha2="0.10"\ntempfile="3"\ntokio={version="1",features=["fs","io-util","macros","rt"]}\n')
with (root/'docs/research/dedup-500/visualvault076-controls-v1-20261021/run.log').open('w') as f:
 subprocess.run(['cargo','+1.98.1','run','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-controls-target-20261021')],stdout=f,stderr=subprocess.STDOUT,check=True)
