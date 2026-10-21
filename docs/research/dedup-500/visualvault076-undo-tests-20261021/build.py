from pathlib import Path
import subprocess
root=Path.cwd();cache=root/'.dedup-work/visualvault076-20261021';dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-undo-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
s='extern crate self as visualvault_models;\nextern crate self as visualvault_config;\nextern crate self as visualvault_utils;\n'
for name,path,exports in [('media_file','crates/models/src/media_file.rs','MediaFile,FileType'),('duplicate','crates/models/src/duplicate.rs','DuplicateStats,DuplicateGroup'),('state','crates/models/src/state.rs','OrganizeResult'),('progress','crates/utils/src/progress.rs','Progress'),('undo_manager','crates/core/src/undo_manager.rs','UndoManager'),('organizer','crates/core/src/organizer.rs','FileOrganizer')]:
 s+='#[path="'+str(cache/path)+'"] pub mod '+name+';\npub use '+name+'::{'+exports+'};\n'
(dest/'src/settings.rs').write_text((cache/'crates/config/src/settings.rs').read_text().split('#[cfg(test)]')[0]);s+='mod settings;pub use settings::{Settings,OrganizationMode};\n'
s+='''
#[cfg(test)] mod controls {
use super::*;
use std::fs;
async fn moved(t:&tempfile::TempDir)->(UndoManager,std::path::PathBuf,std::path::PathBuf) {
 let source=t.path().join("source");let destination=t.path().join("destination");fs::write(&source,b"original").unwrap();fs::rename(&source,&destination).unwrap();let m=UndoManager::new(t.path().join("config"));m.record_move(&source,&destination).await.unwrap();(m,source,destination)
}
#[tokio::test] async fn control_undo_replaces_new_file_at_original_path() {
 let t=tempfile::tempdir().unwrap();let(m,source,dest)=moved(&t).await;fs::write(&source,b"new unrelated file").unwrap();assert!(m.undo().await.unwrap().is_some());assert_eq!(fs::read(source).unwrap(),b"original");assert!(!dest.exists());
}
#[tokio::test] async fn control_failed_redo_discards_retry_entry() {
 let t=tempfile::tempdir().unwrap();let(m,source,_)=moved(&t).await;m.undo().await.unwrap();fs::remove_file(&source).unwrap();assert!(m.redo().await.is_err());fs::write(source,b"original").unwrap();assert!(m.redo().await.unwrap().is_none());assert!(m.get_history().await[0].undone);
}
#[tokio::test] async fn control_reload_does_not_restore_redo_stack() {
 let t=tempfile::tempdir().unwrap();let(m,source,_)=moved(&t).await;m.undo().await.unwrap();drop(m);let m=UndoManager::new_with_history(t.path().join("config")).await.unwrap();assert!(m.get_history().await[0].undone);assert!(source.exists());assert!(m.redo().await.unwrap().is_none());
}
}
'''
(dest/'src/lib.rs').write_text(s)
manifest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-organizer-20261021/Cargo.toml').read_text().replace('visualvault076-organizer','visualvault076-undo').replace('"rt-multi-thread"','"rt-multi-thread","sync"')+'\nthiserror="2"\nuuid={version="1",features=["v4"]}\n';(dest/'Cargo.toml').write_text(manifest)
log=root/'docs/research/dedup-500/visualvault076-undo-tests-20261021/run.log'
with log.open('w') as f:
 for pattern in ['undo_manager::tests::','organizer::tests::','controls::']:
  subprocess.run(['cargo','+1.98.1','test','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-detector-target-20261021'),pattern],stdout=f,stderr=subprocess.STDOUT,check=True)
log.write_text(log.read_text().rstrip()+'\n')
