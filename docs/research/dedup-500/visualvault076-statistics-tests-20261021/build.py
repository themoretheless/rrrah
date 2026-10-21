from pathlib import Path
import subprocess
root=Path.cwd();cache=root/'.dedup-work/visualvault076-20261021';old=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-realmodel-20261021');dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-statistics-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
s=(old/'src/lib.rs').read_text()+'\n#[path="'+str(cache/'crates/models/src/statistics.rs')+'"] mod statistics;\n'
s+='''
#[cfg(test)] mod controls {
use super::*;use statistics::Statistics;
fn fixture()->(Vec<Arc<MediaFile>>,DuplicateStats) {let files=vec![media("a.jpg".into(),4),media("b.jpg".into(),4)];let mut d=DuplicateStats::new();d.groups.push(DuplicateGroup::new(files.clone(),4));(files,d)}
#[test] fn control_repeated_scan_accumulates_duplicate_totals() {
 let(files,d)=fixture();let mut s=Statistics::new();s.update_from_scan_results(&files,&d);assert_eq!((s.duplicate_count,s.duplicate_size),(1,4));s.update_from_scan_results(&files,&d);assert_eq!((s.total_files,s.total_size),(2,8));assert_eq!((s.duplicate_count,s.duplicate_size),(2,8));
}
#[test] fn control_empty_scan_retains_duplicate_totals() {
 let(files,d)=fixture();let mut s=Statistics::new();s.update_from_scan_results(&files,&d);s.update_from_scan_results(&[],&DuplicateStats::new());assert_eq!((s.total_files,s.total_size),(0,0));assert_eq!((s.duplicate_count,s.duplicate_size),(1,4));
}
#[test] fn control_switching_update_methods_retains_incompatible_fields() {
 let(files,d)=fixture();let mut s=Statistics::new();s.update_from_files(&files);assert_eq!(s.largest_files.len(),2);s.update_from_scan_results(&[],&DuplicateStats::new());assert_eq!(s.total_files,0);assert_eq!(s.largest_files.len(),2);assert_eq!(s.files_by_extension.get("jpg"),Some(&2));s.update_from_scan_results(&files,&d);s.update_from_files(&[]);assert_eq!(s.total_files,0);assert_eq!(s.file_types.get(&FileType::Image),Some(&2));assert_eq!(s.duplicate_count,1);
}
}
'''
(dest/'src/lib.rs').write_text(s);(dest/'Cargo.toml').write_text((old/'Cargo.toml').read_text().replace('visualvault076-realmodel','visualvault076-statistics'))
log=root/'docs/research/dedup-500/visualvault076-statistics-tests-20261021/run.log'
with log.open('w') as f:
 for pattern in ['statistics::tests::','controls::']:
  subprocess.run(['cargo','+1.98.1','test','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-detector-target-20261021'),pattern],stdout=f,stderr=subprocess.STDOUT,check=True)
log.write_text(log.read_text().rstrip()+'\n')
