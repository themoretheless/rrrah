#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from collections import Counter
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');BASE=T/'rrrah-dedup-linux-native'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 prep=D/'dedup-exact-lossy-path-linux-prepared-20261012.json';p=json.loads(prep.read_text());assert p['user']=='65534:65534' and all(sha(k)==h for k,h in p['pins'].items())
 mp=D/'dedup-exact-lossy-path-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==h for k,h in m['file_hashes'].items());assert m['deltas_from_parent']==['crates/rrrah-dedup/tests/exact_files.rs']
 for k,h in m['file_hashes'].items():
  if k.startswith('crates/rrrah-dedup/src/') or k=='crates/rrrah-dedup/tests/exact_files.rs':assert sha(R/k)==h,k
 log=T/'rrrah-exact-lossy-path-linux-20261012.log';text=log.read_text();assert 'test result: FAILED' not in text
 matches=re.findall(r'Running (?:unittests src/lib\.rs|tests/([^\s]+)\.rs) \(([^)]+)\)',text);assert len(matches)==2 and {n or 'lib' for n,b in matches}=={'lib','exact_files'}
 out=D/'dedup-exact-lossy-path-linux-verification-20261012.json';saved=T/'rrrah-exact-lossy-path-linux-executables-20261012';assert not out.exists() and not saved.exists();saved.mkdir()
 pins={str(x):sha(x) for x in [prep,mp,log,Path(__file__)]};passed=Counter();ignored=Counter();suites=[]
 cmd=['docker','run','--rm','--user','65534:65534','-v',str(BASE)+':/validation:ro','-v',p['sysroot_mount']['Source']+':/validation/sysroot:ro','-e','LD_LIBRARY_PATH=/validation/sysroot/usr/lib/aarch64-linux-gnu:/validation/sysroot/usr/lib/llvm-18/lib',p['image_id']]
 for name,binary in matches:
  name=name or 'lib';original=BASE/Path(binary).relative_to('/validation');raw=original.read_bytes();assert raw[:4]==b'\x7fELF' and raw[4]==2 and int.from_bytes(raw[18:20],'little')==183
  listed=subprocess.check_output(cmd+[binary,'--list'],text=True);ig=subprocess.check_output(cmd+[binary,'--list','--ignored'],text=True);names=re.findall(r'^(.*): test$',listed,re.M);ignames=re.findall(r'^(.*): test$',ig,re.M);assert len(names)==len(set(names));passed.update(x for x in names if x not in ignames);ignored.update(ignames)
  dest=saved/name;shutil.copy2(original,dest);assert sha(dest)==sha(original);pins[str(dest)]=sha(dest);suites.append({'suite':name,'passed':len(names)-len(ignames),'ignored':ignames,'binary':str(dest)})
 assert passed==Counter(re.findall(r'^test (\S+) \.\.\. ok$',text,re.M));assert ignored==Counter(re.findall(r'^test (\S+) \.\.\. ignored(?:,.*)?$',text,re.M));assert passed['lossy_equal_non_utf8_names_preserve_distinct_copy_partitions']==1
 assert sum(passed.values())==57 and sum(ignored.values())==2
 version=subprocess.check_output(cmd+['/validation/toolchain/bin/rustc','-vV'],text=True);assert 'host: aarch64-unknown-linux-gnu' in version
 out.write_text(json.dumps({'status':'verified_57_current_source_linux_exact_tests','passed':57,'ignored':2,'suites':suites,'compiler':version,'image_id':p['image_id'],'pins':pins,'scope':'Current-identical production, default features disabled, Linux ARM64 nonroot execution. Two invalid UTF8 filenames have identical lossy text but distinct bytes and retain two correct copy families in both overlapping root orders. Includes prior block-order regression. No macOS filesystem invalid-byte support, Windows, all formats/full500 or visual corpus qualification.'},indent=2)+'\n');print('Verified57 Linux exact/default tests; two ignored manual controls')
if __name__=='__main__':main()
