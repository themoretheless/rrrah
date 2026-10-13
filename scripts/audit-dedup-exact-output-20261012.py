#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=D/'dedup-exact-interrupted-source-20261012.json';m=json.loads(parent.read_text());key='crates/rrrah-dedup/src/exact.rs';before=T/'rrrah-exact-output-before-20261012.rs';assert sha(before)==m['file_hashes'][key]
 for p,h in m['file_hashes'].items():
  if p.startswith(('crates/rrrah-dedup/src/','vendor/loftr/src/')) and p!=key:assert sha(R/p)==h,p
 redp=D/'dedup-exact-output-red-20261012.json';red=json.loads(redp.read_text());assert all(sha(p)==h for p,h in red['pins'].items())
 log=T/'rrrah-exact-output-green-20261012.log';text=log.read_text();assert '39 passed; 0 failed; 2 ignored' in text and '23 passed; 0 failed; 0 ignored' in text
 passed=re.findall(r'^test (\S+) \.\.\. ok$',text,re.M);ignored=re.findall(r'^test (\S+) \.\.\. ignored(?:,.*)?$',text,re.M);assert len(passed)==62 and len(ignored)==2 and len(set(passed))==62
 binaries=re.findall(r'^\s*Running .* \(([^\n]+)\)$',text,re.M);assert len(binaries)==2;dest=T/'rrrah-exact-output-executables-20261012';assert not dest.exists();dest.mkdir();pins={str(p):sha(p) for p in [parent,redp,before,log,Path(__file__)]};listed=[]
 for i,b in enumerate(binaries):
  exe=dest/str(i);shutil.copy2(b,exe);pins[str(exe)]=sha(exe);inventory=subprocess.check_output([str(exe),'--list'],text=True);listed+=re.findall(r'^(.*): test$',inventory,re.M)
 assert sorted(listed)==sorted(passed+ignored)
 snapshot=T/'rrrah-exact-output-qualified-20261012';assert not snapshot.exists();snapshot.mkdir();hashes={}
 for p in m['file_hashes']:
  source=R/p if p.startswith(('crates/rrrah-dedup/src/','vendor/loftr/src/')) else Path(m['snapshot'])/p
  target=snapshot/p;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,target);hashes[p]=sha(target)
 assert [p for p in hashes if hashes[p]!=m['file_hashes'][p]]==[key]
 sourcep=D/'dedup-exact-output-source-20261012.json';assert not sourcep.exists();sourcep.write_text(json.dumps({'snapshot':str(snapshot),'parent_manifest':str(parent),'file_hashes':hashes,'deltas_from_parent':[key]},indent=2)+'\n');pins[str(sourcep)]=sha(sourcep)
 # All original production/tests remain identical after removing formatting whitespace,
 # except the two read helpers and explicitly replaced call sites.
 code=(R/key).read_text();assert '.read_exact(' not in code and '.write_all(' not in code;assert code.count('fn write_all_cancelled(')==1 and code.count('write_all_cancelled(&mut output,')==1;assert red['failed']==2 and red['passed']==2
 subprocess.run(['git','diff','--check','--',key],cwd=R,check=True)
 out=D/'dedup-exact-output-verification-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'adopted_cancel_checks_between_output_write_attempts','passed':62,'ignored':2,'tests':passed,'ignored_tests':ignored,'pins':pins,'scope':'Actual current-root macOS ARM64 default library and exact-file integration suites. Public copy_range now checks cancellation before every write attempt, including Interrupted retries and partial writes. Two original public API failures preserved; four writer controls cover cancellation, exact content, WriteZero and BrokenPipe. Prior read cancellation regressions pass. No physical EINTR signal test or kernel/single writer-call preemption, Windows/Linux execution, all formats/full500/full458 or total resource bound claim.'},indent=2)+'\n');print('Verified62passes,2manualignored,onlyexact.rs source delta')
if __name__=='__main__':main()
