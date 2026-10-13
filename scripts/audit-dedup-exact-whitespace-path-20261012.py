#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=D/'dedup-exact-output-source-20261012.json';m=json.loads(parent.read_text());key='crates/rrrah-dedup/tests/exact_files.rs';before=T/'rrrah-exact-whitespace-path-before-20261012.rs';controls=D/'dedup-exact-whitespace-path-controls-20261012.rs';assert sha(before)==m['file_hashes'][key];assert (R/key).read_text()==before.read_text()+'\n'+controls.read_text()
 for p,h in m['file_hashes'].items():
  if p.startswith(('crates/rrrah-dedup/src/','vendor/loftr/src/')):assert sha(R/p)==h,p
 log=T/'rrrah-exact-whitespace-path-macos-20261012.log';text=log.read_text();assert '24 passed; 0 failed; 0 ignored' in text;names=re.findall(r'^test (\S+) \.\.\. ok$',text,re.M);assert len(names)==len(set(names))==24 and names.count('whitespace_path_names_preserve_all_authored_copy_partitions')==1
 original=Path(re.search(r'Running tests/exact_files.rs \(([^\n]+)\)',text)[1]);exe=T/'rrrah-exact-whitespace-path-macos-executable-20261012';assert not exe.exists();shutil.copy2(original,exe);listed=subprocess.check_output([str(exe),'--list'],text=True);assert sorted(names)==sorted(re.findall(r'^(.*): test$',listed,re.M));raw=exe.read_bytes();assert raw[:4]==b'\xcf\xfa\xed\xfe' and int.from_bytes(raw[4:8],'little')==0x100000c
 s=T/'rrrah-exact-whitespace-path-qualified-20261012';assert not s.exists();s.mkdir();hashes={}
 for p in m['file_hashes']:
  source=R/p if p==key else Path(m['snapshot'])/p;dest=s/p;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,dest);hashes[p]=sha(dest)
 assert [p for p in hashes if hashes[p]!=m['file_hashes'][p]]==[key]
 mp=D/'dedup-exact-whitespace-path-source-20261012.json';assert not mp.exists();mp.write_text(json.dumps({'snapshot':str(s),'parent_manifest':str(parent),'file_hashes':hashes,'deltas_from_parent':[key]},indent=2)+'\n')
 out=D/'dedup-exact-whitespace-path-verification-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_24_exact_file_tests_with_authored_whitespace_partitions','passed':24,'ignored':0,'tests':names,'pins':{str(p):sha(p) for p in [parent,before,controls,log,exe,mp,Path(__file__)]},'scope':'Current-identical production source unchanged; only a Unix-specific regression appended. Actual macOS ARM64 filesystem:8 same-size distinct families,2 copies each, leading/trailing space/tab/newline/CR names, ordinary and verified scanning, both root orders with repeated root. Full24exact integration tests pass. No Windows path qualification, new Linux test execution, full500/full458/totalRSS or general import/export contract claim.'},indent=2)+'\n');subprocess.run(['git','diff','--check','--',key],cwd=R,check=True);print('Verified24 exact integration tests;8 authored whitespace filename partitions')
if __name__=='__main__':main()
