use rrrah_dedup::exact::{scan_verified, Options};
use std::fs;
fn main() {
 let list=fs::read_to_string(std::env::args().nth(1).expect("source path list argument")).unwrap();
 let source_paths:Vec<_>=list.lines().collect(); assert_eq!(source_paths.len(),41);
 let dir = tempfile::tempdir().unwrap();
 for (i, source) in source_paths.iter().enumerate() { fs::copy(source, dir.path().join(format!("asset-{i:02}.bin"))).unwrap(); }
 let roots = [dir.path().to_path_buf()];
 let run = || scan_verified(&roots, &Options::default(), || false);
 let report = run();
 assert!(report.complete(), "issues: {:?}", report.issues);
 assert_eq!(report.groups.len(), 4);
 let mut sizes: Vec<_> = report.groups.iter().map(|g| g.paths.len()).collect(); sizes.sort(); assert_eq!(sizes, vec![2,3,3,3]);
 assert!(report.aliases.is_empty());
 let actual:std::collections::BTreeSet<std::collections::BTreeSet<String>>=report.groups.iter().map(|g|g.paths.iter().map(|p|p.file_name().unwrap().to_string_lossy().into_owned()).collect()).collect();
 let expected:std::collections::BTreeSet<std::collections::BTreeSet<String>>=[vec![3,4],vec![5,22,40],vec![7,20,29],vec![8,27,28]].into_iter().map(|g|g.into_iter().map(|i|format!("asset-{i:02}.bin")).collect()).collect();
 assert_eq!(actual,expected);

 for g in &report.groups { let bytes=fs::read(&g.paths[0]).unwrap(); for p in &g.paths { assert_eq!(fs::read(p).unwrap(),bytes); } }
 // Add a second path to one otherwise unique physical file.
 fs::hard_link(dir.path().join("asset-00.bin"),dir.path().join("Алиас-unique.bin")).unwrap();
 let report = run(); assert!(report.complete()); assert_eq!(report.groups.len(),4); assert_eq!(report.aliases.len(),1); assert_eq!(report.aliases[0].paths.len(),2);
 // The ordinary zero-length-file case remains admitted.
 fs::write(dir.path().join("empty-a"), []).unwrap(); fs::write(dir.path().join("empty-b"), []).unwrap();
 let report = run(); assert!(report.complete()); assert_eq!(report.groups.len(),5); assert_eq!(report.groups.iter().filter(|g|g.bytes==0 && g.paths.len()==2).count(),1);
 // Equal-size and equal sampled ends do not imply equal content.
 let a=vec![0x61; 32768]; let mut b=a.clone(); b[16384]=0x62;
 fs::write(dir.path().join("same-ends-a"), &a).unwrap(); fs::write(dir.path().join("same-ends-b"), &b).unwrap();
 let report=run(); assert!(report.complete()); assert_eq!(report.groups.len(),5);
 assert!(!report.groups.iter().any(|g|g.paths.iter().any(|p|p.file_name().unwrap().to_string_lossy().starts_with("same-ends-"))));
 println!("asset_corpus=41 expected_exact_groups=4 hardlink_aliases=1 empty_group=1 same_ends_negative=passed final_groups={} cancelled={} issues={} bytes_read={}",report.groups.len(),report.cancelled,report.issues.len(),report.bytes_read);
}
