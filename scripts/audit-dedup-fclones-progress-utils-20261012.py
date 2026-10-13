#!/usr/bin/env python3
import hashlib,json,re,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'fclones-hasher-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==h for k,h in m['file_hashes'].items())
 up=D/'fclones-upstream-verification-20261012.json';u=json.loads(up.read_text());exe=T/'rrrah-fclones-upstream-executables-20261012/lib';assert sha(exe)==u['pins'][str(exe)]
 names=[];logs=[]
 for mod,n in [('phase',1),('progress',3),('util',4)]:
  log=T/f'rrrah-fclones-{mod}-tests-20261012.log';assert not log.exists();run=subprocess.run([str(exe),mod+'::test::','--nocapture'],capture_output=True,text=True,cwd='/tmp');log.write_text(run.stdout+run.stderr);assert run.returncode==0 and f'{n} passed; 0 failed; 0 ignored' in run.stdout
  rows=re.findall(r'^test (\S+) \.\.\. ok$',run.stdout,re.M);assert len(rows)==n and all(x.startswith(mod+'::test::') for x in rows);names+=rows;logs.append(log)
 files={'fclones/src/phase.rs':62,'fclones/src/progress.rs':268,'fclones/src/util.rs':208};assert all(len((s/p).read_text().splitlines())==n for p,n in files.items())
 out=D/'fclones-progress-utils-review-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_three_complete_files_and_eight_original_tests','commit':m['commit'],'fully_read_additional_files':files,'passed':8,'ignored':0,'tests':names,'pins':{str(p):sha(p) for p in [mp,up,exe,Path(__file__),*logs,*[s/p for p in files]]},'observations':['Phase reporting requires the requested phase to exist; missing phase unwraps.','Progress uses relaxed atomic counters and a fixed50-column display; this is reporting, not work admission or memory budgeting.','try_sort_by_key maps failed keys to None and sorts Option ascending; failures sort before successful Some keys, contrary to the comments. This is a source observation; no new runtime counterexample executed.','IteratorWrapper serialization consumes its iterator through Cell.take().unwrap(); repeated serialization is not supported.'],'scope':'Three full source files and eight existing macOS ARM64 cases. No implementation adopted. No total resource cap, Windows, full42tree or full500 completion claim.'},indent=2)+'\n');print('Verified eight original tests and three complete files')
if __name__=='__main__':main()
