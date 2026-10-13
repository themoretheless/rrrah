#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-exact-root-admission-source-retry2-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==v for k,v in m['file_hashes'].items())
 for k,v in m['file_hashes'].items():
  if k.startswith(('src/','tests/')):assert sha(R/'crates/rrrah-dedup'/k)==v,k
 baseline=json.loads((D/'dedup-exact-restored-mtime-source-retry2-20261012.json').read_text());old=(Path(baseline['snapshot'])/'src/exact.rs').read_text();new=(s/'src/exact.rs').read_text();a='fn collect(';b='    let mut directories = BTreeSet::new();';assert old[:old.index(a)]==new[:new.index(a)];assert old[old.index(b):]==new[new.index(b):]
 log=T/'rrrah-exact-root-admission-tests-retry2-20261012.log';text=log.read_text();assert re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',text)==[('32','2'),('22','0')]
 for n in ['restored_modified_time_does_not_hide_same_length_snapshot_edit','restored_modified_time_does_not_hide_same_inode_edit_during_scan','excess_root_admission_refuses_before_missing_source_io','root_admission_cancels_before_io_at_each_preparation_checkpoint','duplicate_root_admission_retains_boundary_and_fresh_retry']:assert f'test {n} ... ok' in text
 out=D/'dedup-exact-root-admission-verification-20261012.json';saved=T/'rrrah-exact-root-admission-executables-20261012';assert not out.exists()and not saved.exists();saved.mkdir();pins={str(p):sha(p)for p in [mp,log,Path(__file__),s/'Cargo.lock']};suites=[]
 for lib,test,binary in re.findall(r'Running (?:unittests src/(lib)\.rs|tests/(exact_files)\.rs) \(([^)]+)\)',text):
  name=lib or test;binary=Path(binary);listed=subprocess.check_output([str(binary),'--list'],text=True);ignored=subprocess.check_output([str(binary),'--list','--ignored'],text=True);all_names=set(re.findall(r'^(.*): test$',listed,re.M));ignore=set(re.findall(r'^(.*): test$',ignored,re.M));assert (len(all_names-ignore),len(ignore))==((32,2)if name=='lib'else(22,0));assert all(re.search(r'^test '+re.escape(n)+r' \.\.\. ok$',text,re.M)for n in all_names-ignore)
  dest=saved/name;shutil.copy2(binary,dest);assert sha(dest)==sha(binary);pins[str(dest)]=sha(dest);suites.append({'suite':name,'passed':len(all_names-ignore),'ignored':sorted(ignore)})
 assert len(suites)==2
 out.write_text(json.dumps({'status':'verified_54_default_tests_with_bounded_initial_root_admission','passed':54,'suites':suites,'pins':pins,'scope':'Current-identical adopted production source and tests; macOS ARM64 default32units+22exactintegration passes. Bounded borrowed unique-root preparation; excess roots refuse before filesystem IO,64one-shot cancellation checkpoints during preparation,duplicate1000roots fit limit1,and restored retry succeeds. Existing sample/FIFO mutation fixtures preserve original phases after new root callbacks; no assertions removed. Same-length restored-mtime snapshot and Unix during-scan guards retained. No total-byte/process cap, Windows/currentLinux/allfeature/fullworkspace/format/500 claim. Firstcandidate FIFO phase fixture failed and is preserved separately.'},indent=2)+'\n');print('Verified54default-feature passes, bounded root admission and restored-mtime regressions, two manual controls ignored')
if __name__=='__main__':main()
