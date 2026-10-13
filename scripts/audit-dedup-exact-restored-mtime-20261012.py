#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-exact-restored-mtime-source-retry2-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==v for k,v in m['file_hashes'].items())
 for k,v in m['file_hashes'].items():
  if k.startswith(('src/','tests/')):assert sha(R/'crates/rrrah-dedup'/k)==v,k
 log=T/'rrrah-exact-restored-mtime-tests-retry2-20261012.log';text=log.read_text();assert re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',text)==[('32','2'),('19','0')]
 for n in ['restored_modified_time_does_not_hide_same_length_snapshot_edit','restored_modified_time_does_not_hide_same_inode_edit_during_scan']:assert f'test {n} ... ok' in text
 out=D/'dedup-exact-restored-mtime-verification-20261012.json';saved=T/'rrrah-exact-restored-mtime-executables-20261012';assert not out.exists()and not saved.exists();saved.mkdir();pins={str(p):sha(p)for p in [mp,log,Path(__file__),s/'Cargo.lock']};suites=[]
 for lib,test,binary in re.findall(r'Running (?:unittests src/(lib)\.rs|tests/(exact_files)\.rs) \(([^)]+)\)',text):
  name=lib or test;binary=Path(binary);listed=subprocess.check_output([str(binary),'--list'],text=True);ignored=subprocess.check_output([str(binary),'--list','--ignored'],text=True);all_names=set(re.findall(r'^(.*): test$',listed,re.M));ignore=set(re.findall(r'^(.*): test$',ignored,re.M));assert (len(all_names-ignore),len(ignore))==((32,2)if name=='lib'else(19,0));assert all(re.search(r'^test '+re.escape(n)+r' \.\.\. ok$',text,re.M)for n in all_names-ignore)
  dest=saved/name;shutil.copy2(binary,dest);assert sha(dest)==sha(binary);pins[str(dest)]=sha(dest);suites.append({'suite':name,'passed':len(all_names-ignore),'ignored':sorted(ignore)})
 assert len(suites)==2
 out.write_text(json.dumps({'status':'verified_51_default_feature_tests_and_restored_mtime_guards','passed':51,'suites':suites,'pins':pins,'scope':'Current-identical production source and test files; default standalone build on macOS ARM64. Same-length restored-mtime snapshot edit refused and fresh retry succeeds. Unix same-inode/size/mtime mutation during scan rejected via changed metadata, with deterministic distinct ctime tick and healthy fresh retry. No Windows execution, all-feature/full-workspace/format/500 qualification. First preparation missing build_context failed before tests; retry source/log retained separately.'},indent=2)+'\n');print('Verified51default-feature passes, two new restored-mtime regressions, two manual controls ignored')
if __name__=='__main__':main()
