from pathlib import Path
import subprocess
root=Path.cwd(); cache=root/'.dedup-work/visualvault076-20261021'
dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-folderstats-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
actions=(cache/'crates/app/src/actions.rs').read_text();start=actions.index('fn is_media_extension(');end=actions.index('\nasync fn execute_scan_background(',start)
s='use color_eyre::eyre::Result;\nuse walkdir::WalkDir;\n'
s+='#[path="'+str(cache/'crates/utils/src/folder_stats.rs')+'"] mod folder_stats;\nuse folder_stats::FolderStats;\n'+actions[start:end]
s+='''
#[tokio::test] async fn control_missing_root_returns_success_shaped_zero_stats() {
 let root=tempfile::tempdir().unwrap();let absent=root.path().join("missing");
 let stats=calculate_folder_stats_async(&absent).await;
 assert_eq!((stats.total_files,stats.total_dirs,stats.media_files,stats.total_size),(0,0,0,0));
}
#[tokio::test] async fn control_folder_stats_counts_hardlink_paths_twice() {
 let root=tempfile::tempdir().unwrap();let a=root.path().join("a.jpg");let b=root.path().join("b.jpg");
 std::fs::write(&a,b"same").unwrap();std::fs::hard_link(&a,&b).unwrap();
 let stats=calculate_folder_stats_async(root.path()).await;
 assert_eq!((stats.total_files,stats.total_dirs,stats.media_files,stats.total_size),(2,1,2,8));
}
'''
(dest/'src/lib.rs').write_text(s)
(dest/'Cargo.toml').write_text('[package]\nname="visualvault076-folderstats"\nversion="0.0.0"\nedition="2021"\n[dependencies]\ncolor-eyre="0.6"\nwalkdir="2"\ntempfile="3"\ntokio={version="1",features=["macros","rt-multi-thread"]}\n')
log=root/'docs/research/dedup-500/visualvault076-folderstats-controls-20261021/run.log'
with log.open('w') as f:subprocess.run(['cargo','+1.98.1','test','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-detector-target-20261021'),'control_'],stdout=f,stderr=subprocess.STDOUT,check=True)
log.write_text(log.read_text().rstrip()+'\n')
