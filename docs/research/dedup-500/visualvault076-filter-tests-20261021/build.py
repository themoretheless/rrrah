from pathlib import Path
import subprocess
root=Path.cwd();cache=root/'.dedup-work/visualvault076-20261021';dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-filters-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
s='use chrono::{Duration,Local,NaiveDate,TimeZone};\n'
for name,p in [('media_file','crates/models/src/media_file.rs'),('filters','crates/models/src/filters.rs')]:s+='#[path="'+str(cache/p)+'"] mod '+name+';\n'
s+='pub use media_file::{MediaFile,FileType};\nuse filters::{FilterSet,RegexTarget};\nstruct App;\nimpl App {\n';original=(cache/'crates/app/src/filters.rs').read_text();s+=original[original.index('    fn parse_date_range('):original.index('    fn delete_selected_filter(')]+'}\n'
s+='''
fn media()->MediaFile {let now=Local::now();MediaFile {path:"test.jpg".into(),name:"test.jpg".into(),extension:"jpg".into(),file_type:FileType::Image,size:1,created:now,modified:now,metadata:None,hash:None}}
#[test] fn control_kilobyte_filter_mixes_unit_factors() {
 let (min,max)=App::parse_size_range(">1KB").unwrap();let mut f=FilterSet::new();f.add_size_range("parsed".into(),min,max);assert_eq!(f.size_ranges[0].min_bytes,Some(1048));let mut file=media();file.size=1024;assert!(!f.matches_file(&file));
}
#[test] fn control_invalid_date_endpoint_becomes_unbounded() {
 let (from,to)=App::parse_date_range("2024-01-01 to nonsense").unwrap();assert!(from.is_some());assert!(to.is_none());
}
#[test] fn control_day_filter_excludes_fractional_final_second() {
 let (from,to)=App::parse_date_range("2024-01-01").unwrap();let mut f=FilterSet::new();f.add_date_range("day".into(),from,to);let mut file=media();file.modified=to.unwrap()+Duration::milliseconds(500);assert_eq!(file.modified.date_naive(),to.unwrap().date_naive());assert!(!f.matches_file(&file));
}
#[test] fn control_nan_size_bound_is_accepted_then_cast_to_zero() {
 let (min,max)=App::parse_size_range("<NaN").unwrap();assert!(max.unwrap().is_nan());let mut f=FilterSet::new();f.add_size_range("nan".into(),min,max);assert_eq!(f.size_ranges[0].max_bytes,Some(0));assert!(!f.matches_file(&media()));
}
'''
(dest/'src/lib.rs').write_text(s)
manifest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-realmodel-20261021/Cargo.toml').read_text().replace('visualvault076-realmodel','visualvault076-filters')+'\nregex="1"\n';(dest/'Cargo.toml').write_text(manifest)
log=root/'docs/research/dedup-500/visualvault076-filter-tests-20261021/run.log'
with log.open('w') as f:
 for pattern in ['filters::tests::','control_']:
  subprocess.run(['cargo','+1.98.1','test','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-detector-target-20261021'),pattern],stdout=f,stderr=subprocess.STDOUT,check=True)
log.write_text(log.read_text().rstrip()+'\n')
