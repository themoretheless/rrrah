#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'fclones-hasher-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==v for k,v in m['file_hashes'].items());assert not subprocess.check_output(['git','-C',str(s),'status','--porcelain'],text=True).strip()
 log=T/'rrrah-fclones-full-tests-20261012.log';text=log.read_text();assert re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',text)==[('166','1'),('4','0'),('2','0')]
 saved=T/'rrrah-fclones-upstream-executables-20261012';out=D/'fclones-upstream-verification-20261012.json';assert not out.exists()and not saved.exists();saved.mkdir();pins={str(p):sha(p) for p in [mp,log,Path(__file__),s/'Cargo.lock']};suites=[]
 for name,binary in re.findall(r'Running unittests src/(lib|main)\.rs \(([^)]+)\)',text):
  binary=Path(binary);listed=subprocess.check_output([str(binary),'--list'],text=True);ignored=subprocess.check_output([str(binary),'--list','--ignored'],text=True);all_names=set(re.findall(r'^(.*): test$',listed,re.M));ignore=set(re.findall(r'^(.*): test$',ignored,re.M));assert (len(all_names-ignore),len(ignore))==((166,1)if name=='lib'else(4,0));assert all(re.search(r'^test '+re.escape(n)+r' \.\.\. ok$',text,re.M)for n in all_names-ignore);assert all(re.search(r'^test '+re.escape(n)+r' \.\.\. ignored',text,re.M)for n in ignore)
  dest=saved/name;shutil.copy2(binary,dest);assert sha(dest)==sha(binary);pins[str(dest)]=sha(dest);suites.append({'suite':name,'passed':len(all_names-ignore),'ignored':sorted(ignore)})
 assert len(suites)==2 and len(re.findall(r'^test fclones/src/.* \.\.\. ok$',text,re.M))==2
 sem=T/'rrrah-fclones-semaphore-tests-20261012';sl=T/'rrrah-fclones-semaphore-tests-20261012.log';assert '7 passed; 0 failed; 0 ignored' in sl.read_text();pins.update({str(p):sha(p)for p in [sem,sl]});assert 'hasher::test::test_file_hash_blake3' in text
 out.write_text(json.dumps({'status':'verified_170_original_unit_tests_and_two_docs_macos_arm64','source_commit':m['commit'],'suites':suites,'doc_tests_passed':2,'standalone_original_semaphore_tests_passed':7,'fully_read_source_files':['LICENSE','fclones/Cargo.toml','fclones/src/hasher.rs','fclones/src/semaphore.rs','fclones/src/rlimit.rs','fclones/src/device.rs'],'compiler':subprocess.check_output(['rustc','-vV'],text=True),'pins':pins,'scope':'Unmodified pinned fclones default-feature whole upstream cargo test, locked dependencies on current macOS ARM64. Linux-only physical-device test ignored. Standalone original semaphore source also ran seven tests. Tree is pinned but full42-file read remains incomplete; no rrrah production adoption, full500, other platforms or total process cap qualification.'},indent=2)+'\n');print('Verified original170units+2docs; one Linux-only ignored test; full tree reading still incomplete')
if __name__=='__main__':main()
