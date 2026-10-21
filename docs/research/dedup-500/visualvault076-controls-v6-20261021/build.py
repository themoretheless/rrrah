from pathlib import Path
import subprocess
root=Path.cwd();dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-cache-20261021');(dest/'src').mkdir(parents=True,exist_ok=True);base=root/'.dedup-work/visualvault076-20261021'
s='extern crate self as visualvault_models;\n'+f'#[path="{base}/crates/models/src/media_file.rs"] pub mod media_file;\npub use media_file::{{MediaFile,FileType,MediaMetadata}};\n#[path="{base}/crates/core/src/database_cache.rs"] mod database_cache;\n'
s+='''
use database_cache::{DatabaseCache,CacheEntry};
use std::{path::PathBuf,os::unix::ffi::OsStringExt};
fn entry(path:PathBuf,hash:&str,modified:chrono::DateTime<chrono::Local>)->CacheEntry{CacheEntry{path,name:"fixture".into(),extension:"jpg".into(),size:4,modified,hash:Some(hash.into()),metadata:None}}
#[tokio::main] async fn main()->color_eyre::Result<()> {
 let cache=DatabaseCache::new(":memory:").await?;
 let early:chrono::DateTime<chrono::Local>=chrono::DateTime::from_timestamp(1700000000,100).unwrap().into();
 let later:chrono::DateTime<chrono::Local>=chrono::DateTime::from_timestamp(1700000000,900000000).unwrap().into();
 let p=PathBuf::from("/authored/time.jpg");cache.insert(p.clone(),entry(p.clone(),"old",early)).await?;
 let hit=cache.get(&p,4,&later).await?.unwrap();assert_eq!(hit.hash.as_deref(),Some("old"));assert_ne!(early,later);
 println!("PASS original cache accepts different subsecond modified time and returns old hash");
 let cache=DatabaseCache::new(":memory:").await?;let root=tempfile::tempdir()?;
 let a=root.path().join(std::ffi::OsString::from_vec(vec![0xff,b'.',b'j',b'p',b'g']));
 let b=root.path().join(std::ffi::OsString::from_vec(vec![0xfe,b'.',b'j',b'p',b'g']));
 assert_ne!(a,b);assert_eq!(a.to_string_lossy(),b.to_string_lossy());
 cache.insert(a.clone(),entry(a.clone(),"hash-a",early)).await?;cache.insert(b.clone(),entry(b.clone(),"hash-b",early)).await?;
 assert_eq!(cache.len().await?,1);let hit=cache.get(&a,4,&early).await?.unwrap();assert_eq!(hit.hash.as_deref(),Some("hash-b"));assert_ne!(hit.path,a);assert_ne!(hit.path,b);
 println!("PASS original SQLite cache merges two distinct nonUTF8 paths and returns other hash with lossy path");Ok(())
}
'''
(dest/'src/main.rs').write_text(s);(dest/'Cargo.toml').write_text('[package]\nname="visualvault076-cache"\nversion="0.0.0"\nedition="2021"\n[dependencies]\ncolor-eyre="0.6"\nchrono={version="0.4",features=["serde"]}\nserde={version="1",features=["derive","rc"]}\nserde_json="1"\ntracing="0.1"\ndirs="6"\ntempfile="3"\ntokio={version="1",features=["full"]}\nsqlx={version="0.8",default-features=false,features=["runtime-tokio","sqlite"]}\n')
with (root/'docs/research/dedup-500/visualvault076-controls-v6-20261021/run.log').open('w') as f:subprocess.run(['cargo','+1.98.1','run','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-cache-target-20261021')],stdout=f,stderr=subprocess.STDOUT,check=True)
