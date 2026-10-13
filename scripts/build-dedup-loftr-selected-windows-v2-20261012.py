#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-loftr-selected-windows-source-v2-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==h for k,h in m['file_hashes'].items())
 h=T/'rrrah-loftr-selected-windows-test-harness-v2-20261012';assert not h.exists();h.mkdir();shutil.copytree(s/'vendor/loftr/src',h/'src')
 old=T/'rrrah-loftr-coarse-cancel-test-harness-20261011'
 for f in ['Cargo.toml','Cargo.lock']:shutil.copy2(old/f,h/f)
 pins={str(p):sha(p) for p in [mp,Path(__file__),h/'Cargo.toml',h/'Cargo.lock',*[x for x in (h/'src').rglob('*') if x.is_file()]]}
 out=D/'dedup-loftr-selected-windows-v2-tests-20261012.json';assert not out.exists();result={'status':'building_and_testing','producer_pid':os.getpid(),'pins':pins}
 def save():
  temp=out.with_suffix('.tmp');temp.write_text(json.dumps(result,indent=2)+'\n');temp.replace(out)
 env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');runtime=T/'rrrah-loftr-native-runtime-20261010';env['PATH']=str(runtime/'bin')+':'+env['PATH'];env['DYLD_LIBRARY_PATH']=str(runtime/'lib/python3.12/site-packages/torch/lib')
 cmd=['cargo','test','--manifest-path',str(h/'Cargo.toml'),'--lib','--locked','--offline','--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011'),'--','--test-threads=1']
 result['command']=cmd;save();log=T/'rrrah-loftr-selected-windows-v2-tests-20261012.log'
 with log.open('w') as f:
  child=subprocess.Popen(cmd,cwd='/tmp',env=env,stdout=f,stderr=subprocess.STDOUT);result['child_pid']=child.pid;save();rc=child.wait()
 result.update(status='tests_passed_not_adopted' if rc==0 else 'failed',returncode=rc,log=str(log));pins[str(log)]=sha(log);save();assert rc==0;print('All candidate native unit tests passed')
if __name__=='__main__':main()
