use std::{fs,path::PathBuf,time::{SystemTime,UNIX_EPOCH}};
use dupfile_analyzer::{config::{ScanConfig,ScanMode},scanner::{scan,ScanResults},export::{export,Format}};
fn main(){
 let root=std::env::temp_dir().join(format!("rrrah-dfa416-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
 fs::create_dir(&root).unwrap();
 let hard=root.join("hard");fs::create_dir(&hard).unwrap();fs::write(hard.join("a"),b"same").unwrap();fs::hard_link(hard.join("a"),hard.join("alias")).unwrap();fs::write(hard.join("copy"),b"same").unwrap();
 let r=scan(&ScanConfig::new(hard,ScanMode::ExactFiles)).unwrap();assert_eq!(r.groups.len(),1);assert_eq!(r.groups[0].files.len(),3);assert_eq!(r.reclaimable_bytes(),8);println!("PASS hardlink alias counted as redundant physical copy; report8bytes versus4 logical duplicate bytes");
 #[cfg(unix)]{
 let sym=root.join("sym");fs::create_dir(&sym).unwrap();fs::write(sym.join("real"),b"same").unwrap();std::os::unix::fs::symlink("real",sym.join("link")).unwrap();
 let cfg=ScanConfig::new(sym,ScanMode::ExactFiles);assert!(!cfg.follow_symlinks);let r=scan(&cfg).unwrap();assert_eq!(r.scanned,2);assert_eq!(r.groups[0].files.len(),2);println!("PASS symlink-to-file included and grouped with follow_symlinks=false");
 }
 let r=ScanResults{mode:ScanMode::ExactFiles,algorithm:None,threshold:None,scanned:1,errors:vec!["authored unreadable source".into()],groups:vec![]};
 let dest:PathBuf=root.join("report.json");export(&r,Format::Json,&dest).unwrap();let s=fs::read_to_string(dest).unwrap();assert!(!s.contains("authored unreadable source"));assert!(!s.contains("errors"));println!("PASS original JSON exporter drops explicit authored scan errors");
 fs::remove_dir_all(root).unwrap();
}
