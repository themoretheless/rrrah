from pathlib import Path
import subprocess
root=Path.cwd();cache=root/'.dedup-work/visualvault076-20261021';dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-organizer-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
original=(cache/'crates/core/src/organizer.rs').read_text();start=original.index('    async fn organize_file(');end=original.index('    pub async fn is_complete(',start)
settings=(cache/'crates/config/src/settings.rs').read_text().split('#[cfg(test)]')[0];(dest/'src/settings.rs').write_text(settings)
s='''use color_eyre::eyre::Result;
use tracing::error;
use std::{path::{Path,PathBuf},str::FromStr,sync::Arc};
use tokio::fs;
mod settings; use settings::{Settings,OrganizationMode};
'''
s+='#[path="'+str(cache/'crates/models/src/media_file.rs')+'"] mod media_file;\nuse media_file::{MediaFile,FileType};\n'
s+='''// Authored operation-record adapters; undo/history is not exercised.
struct MoveOperation { source:PathBuf, destination:PathBuf }
enum FileOperation { Move(MoveOperation) }
struct FileOrganizer;
impl FileOrganizer {
'''+original[start:end]+'}\n'
s+='''
fn media(path:PathBuf)->MediaFile {let now=chrono::Local::now();MediaFile {name:Arc::from(path.file_name().unwrap().to_str().unwrap()),path,extension:Arc::from("jpg"),file_type:FileType::Image,size:4,created:now,modified:now,metadata:None,hash:None}}
fn settings(dest:PathBuf)->Settings {Settings{destination_folder:Some(dest),organize_by:"type".into(),rename_duplicates:false,lowercase_extensions:false,..Settings::default()}}
#[tokio::test] async fn control_dry_run_still_moves_file() {
 let t=tempfile::tempdir().unwrap();let src=t.path().join("a.jpg");fs::write(&src,b"same").await.unwrap();let dest=t.path().join("dest");let mut s=settings(dest.clone());s.dry_run=true;let mut ops=vec![];
 let result=FileOrganizer.organize_file(&media(src.clone()),&dest,&s,&mut ops).await.unwrap();
 assert!(!src.exists());assert_eq!(fs::read(result).await.unwrap(),b"same");assert_eq!(ops.len(),1);
}
#[tokio::test] async fn control_existing_destination_is_replaced_without_rename() {
 let t=tempfile::tempdir().unwrap();let src=t.path().join("a.jpg");fs::write(&src,b"new!").await.unwrap();let dest=t.path().join("dest");fs::create_dir_all(dest.join("Images")).await.unwrap();let target=dest.join("Images/a.jpg");fs::write(&target,b"old!").await.unwrap();
 let s=settings(dest.clone());FileOrganizer.organize_file(&media(src.clone()),&dest,&s,&mut vec![]).await.unwrap();
 assert!(!src.exists());assert_eq!(fs::read(target).await.unwrap(),b"new!");
}
#[tokio::test] async fn control_changed_source_is_moved_using_old_observation() {
 let t=tempfile::tempdir().unwrap();let src=t.path().join("a.jpg");fs::write(&src,b"same").await.unwrap();let observed=media(src.clone());fs::write(&src,b"changed since observation").await.unwrap();let dest=t.path().join("dest");let s=settings(dest.clone());
 let result=FileOrganizer.organize_file(&observed,&dest,&s,&mut vec![]).await.unwrap();assert_eq!(observed.size,4);assert_eq!(fs::read(result).await.unwrap(),b"changed since observation");assert!(!src.exists());
}
'''
(dest/'src/lib.rs').write_text(s)
manifest=(Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-realmodel-20261021/Cargo.toml')).read_text().replace('visualvault076-realmodel','visualvault076-organizer')+'\ndirs="6"\nnum_cpus="1"\ntoml="0.8"\n';(dest/'Cargo.toml').write_text(manifest)
log=root/'docs/research/dedup-500/visualvault076-organizer-controls-20261021/run.log'
with log.open('w') as f:subprocess.run(['cargo','+1.98.1','test','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-detector-target-20261021'),'control_'],stdout=f,stderr=subprocess.STDOUT,check=True)
log.write_text(log.read_text().rstrip()+'\n')
