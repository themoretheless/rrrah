from pathlib import Path
import subprocess
root=Path.cwd();dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-detector-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
source=root/'.dedup-work/visualvault076-20261021/crates/core/src/duplicate_detector.rs';models=root/'.dedup-work/visualvault076-20261021/crates/models/src/duplicate.rs'
code='''extern crate self as visualvault_models;
use std::{path::PathBuf,sync::Arc};
#[derive(Clone)] pub enum FileType { Image }
pub mod media_file {
 use super::*;
 #[derive(Clone)] pub struct MediaFile {
 pub path:PathBuf,pub name:Arc<str>,pub extension:Arc<str>,pub file_type:FileType,pub size:u64,
 pub created:chrono::DateTime<chrono::Local>,pub modified:chrono::DateTime<chrono::Local>,pub hash:Option<Arc<str>>,pub metadata:Option<()>,
 }
}
pub use media_file::MediaFile;
'''+f'#[path={str(models)!r}] mod duplicate_models;\npub use duplicate_models::{{DuplicateGroup,DuplicateStats}};\n#[path={str(source)!r}] mod detector;\n'.replace("'",'"')
# Debug is required by the original DuplicateGroup derives.
code=code.replace('#[derive(Clone)] pub enum','#[derive(Debug,Clone)] pub enum').replace('#[derive(Clone)] pub struct','#[derive(Debug,Clone)] pub struct')
code+='''
fn media(path:PathBuf,size:u64)->Arc<MediaFile>{Arc::new(MediaFile{path,name:Arc::from("fixture"),extension:Arc::from("bin"),file_type:FileType::Image,size,created:chrono::Local::now(),modified:chrono::Local::now(),hash:None,metadata:None})}
#[tokio::test] async fn control_quick_mode_admits_unequal_files(){
 let root=tempfile::tempdir().unwrap();let a=root.path().join("a");let b=root.path().join("b");let x=vec![7u8;8192];let mut y=x.clone();y[4096]=9;std::fs::write(&a,x).unwrap();std::fs::write(&b,y).unwrap();
 let files=[media(a,8192),media(b,8192)];let d=detector::DuplicateDetector::new();assert_eq!(d.detect_duplicates(&files,true).await.unwrap().total_groups,1);assert_eq!(d.detect_duplicates(&files,false).await.unwrap().total_groups,0);
}
#[tokio::test] async fn control_missing_zero_size_paths_form_phantom_group(){
 let root=tempfile::tempdir().unwrap();let files=[media(root.path().join("missing-a"),0),media(root.path().join("missing-b"),0)];let d=detector::DuplicateDetector::new();let r=d.detect_duplicates(&files,true).await.unwrap();assert_eq!(r.total_groups,1);assert_eq!(r.total_duplicates,1);
}
#[tokio::test] async fn control_full_mode_counts_hardlink_alias_as_duplicate(){
 let root=tempfile::tempdir().unwrap();let a=root.path().join("a");let b=root.path().join("b");std::fs::write(&a,b"same").unwrap();std::fs::hard_link(&a,&b).unwrap();let r=detector::DuplicateDetector::new().detect_duplicates(&[media(a,4),media(b,4)],false).await.unwrap();assert_eq!(r.total_duplicates,1);assert_eq!(r.total_wasted_space,4);
}
'''
(dest/'src/lib.rs').write_text(code)
(dest/'Cargo.toml').write_text('[package]\nname="visualvault076-detector"\nversion="0.0.0"\nedition="2021"\n[dependencies]\ncolor-eyre="0.6"\nsha2="0.10"\nsmallvec="1"\nahash="0.8"\nchrono="0.4"\ntracing="0.1"\ntempfile="3"\ntokio={version="1",features=["fs","io-util","macros","rt-multi-thread"]}\n')
with (root/'docs/research/dedup-500/visualvault076-controls-v3-20261021/run.log').open('w') as f:subprocess.run(['cargo','+1.98.1','test','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-detector-target-20261021')],stdout=f,stderr=subprocess.STDOUT,check=True)
