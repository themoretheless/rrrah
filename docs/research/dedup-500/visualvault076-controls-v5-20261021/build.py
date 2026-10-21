from pathlib import Path
import subprocess
root=Path.cwd();old=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-realmodel-20261021');dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-inputhash-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
s=(old/'src/lib.rs').read_text()+'''
#[tokio::test] async fn control_input_hash_stays_unset() {
 let root=tempfile::tempdir().unwrap();let a=root.path().join("a");let b=root.path().join("b");std::fs::write(&a,b"same").unwrap();std::fs::write(&b,b"same").unwrap();
 let files=[media(a,4),media(b,4)];assert!(files.iter().all(|f|f.hash.is_none()));
 let r=detector::DuplicateDetector::new().detect_duplicates(&files,false).await.unwrap();assert_eq!(r.total_groups,1);
 assert!(r.groups[0].files.iter().all(|f|f.hash.is_some()));assert!(files.iter().all(|f|f.hash.is_none()));
}
'''
(dest/'src/lib.rs').write_text(s);(dest/'Cargo.toml').write_text((old/'Cargo.toml').read_text().replace('visualvault076-realmodel','visualvault076-inputhash'))
with (root/'docs/research/dedup-500/visualvault076-controls-v5-20261021/run.log').open('w') as f:subprocess.run(['cargo','+1.98.1','test','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-detector-target-20261021'),'control_input_hash_stays_unset'],stdout=f,stderr=subprocess.STDOUT,check=True)
