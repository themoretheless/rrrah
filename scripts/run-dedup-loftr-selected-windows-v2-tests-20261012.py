#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 bp=D/'dedup-loftr-selected-windows-v2-tests-20261012.json';b=json.loads(bp.read_text());assert all(sha(p)==h for p,h in b['pins'].items())
 text=Path(b['log']).read_text();match=re.search(r'Running unittests src/lib.rs \(([^)]+)\)',text);assert match and 'Finished `test`' in text
 original=Path(match[1]);exe=T/'rrrah-loftr-selected-windows-v2-test-executable-20261012';assert not exe.exists();shutil.copy2(original,exe);assert sha(exe)==sha(original)
 pins=dict(b['pins']);pins.update({str(p):sha(p) for p in [bp,exe,Path(__file__)]})
 out=D/'dedup-loftr-selected-windows-v2-direct-tests-20261012.json';assert not out.exists();result={'status':'running','producer_pid':os.getpid(),'pins':pins,'cargo_runner_failure':'Test binary compiled; Cargo dynamic library search omitted explicit Torch directory. Earlier failed launch log retained; no test assertion ran.'}
 def save():
  tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(result,indent=2)+'\n');tmp.replace(out)
 cmd=['/usr/bin/env','DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),str(exe),'--test-threads=1'];result['command']=cmd;save();log=T/'rrrah-loftr-selected-windows-v2-direct-tests-20261012.log'
 with log.open('w') as f:
  child=subprocess.Popen(cmd,cwd='/tmp',stdout=f,stderr=subprocess.STDOUT);result['child_pid']=child.pid;save();rc=child.wait()
 pins[str(log)]=sha(log);result.update(status='passed_not_adopted' if rc==0 else 'failed',returncode=rc,log=str(log));save();assert rc==0;print('Direct candidate unit suite passed')
if __name__=='__main__':main()
