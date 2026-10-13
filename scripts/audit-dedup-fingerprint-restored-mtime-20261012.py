#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-fingerprint-restored-mtime-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);parent=Path(m['parent_manifest']);assert sha(parent)==m['parent_sha256'];p=json.loads(parent.read_text());assert all(sha(s/k)==v for k,v in m['file_hashes'].items());assert [k for k,v in m['file_hashes'].items()if p['file_hashes'][k]!=v]==m['deltas']==['crates/rrrah-dedup/tests/decode.rs']
 for k,v in m['file_hashes'].items():
  if k.startswith('crates/rrrah-dedup/src/')or k=='crates/rrrah-dedup/tests/decode.rs':assert sha(R/k)==v,k
 log=T/'rrrah-fingerprint-restored-mtime-tests-20261012.log';text=log.read_text();assert '15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'in text;assert 'test memory_and_persisted_fingerprint_cache_revalidate_same_size_restored_mtime ... ok'in text;b=Path(re.search(r'Running tests/decode.rs \(([^)]+)\)',text)[1]);dest=T/'rrrah-fingerprint-restored-mtime-executable-20261012';out=D/'dedup-fingerprint-restored-mtime-verification-20261012.json';assert not out.exists()and not dest.exists();shutil.copy2(b,dest);assert sha(b)==sha(dest);ls=subprocess.check_output([str(dest),'--list'],text=True);names=set(re.findall(r'^(.*): test$',ls,re.M));assert len(names)==15 and names==set(re.findall(r'^test (\S+) \.\.\. ok$',text,re.M))
 out.write_text(json.dumps({'status':'verified_15_current_decode_cache_tests_with_restored_mtime_regression','passed':15,'ignored':0,'tests':sorted(names),'pins':{str(x):sha(x)for x in [mp,parent,log,dest,Path(__file__)]},'scope':'Current production unchanged, one added regression across two same-size bitmap edits with exactrestoredmtime and distinctsubmillisecondmtime. Warm memory and saved/reloaded cache both miss after edit and equal fresh fingerprint; new healthy repeat hits and managed usage returns0. All14existingdecode tests also pass. macOS ARM64 on compatible frozen dependencies, not Windows/currentLinux/allformats/wholeprocesscap/500qualification.'},indent=2)+'\n');print('Verified15currentdecode/cachetests, including memory and persisted restoredmtime/submillisecond invalidation')
if __name__=='__main__':main()
