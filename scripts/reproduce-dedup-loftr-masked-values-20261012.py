#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-loftr-selected-windows-current-baseline-20261012.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items());h=T/'rrrah-loftr-masked-value-red-harness-20261012';assert not h.exists();h.mkdir();shutil.copytree(Path(m['snapshot'])/'vendor/loftr/src',h/'src')
 for p in (h/'src').rglob('*.rs'):assert sha(p)==sha(R/'vendor/loftr/src'/p.relative_to(h/'src'))
 old=T/'rrrah-loftr-coarse-cancel-test-harness-20261011'
 for f in ['Cargo.toml','Cargo.lock']:shutil.copy2(old/f,h/f)
 controls=D/'dedup-loftr-masked-value-controls-20261012.rs';tests=h/'src/linear_attention/tests.rs';tests.write_text(tests.read_text()+controls.read_text())
 pins={str(p):sha(p) for p in [mp,controls,Path(__file__),*[p for p in h.rglob('*') if p.is_file()]]};out=D/'dedup-loftr-masked-values-red-20261012.json';assert not out.exists();r={'status':'building_reproducer','producer_pid':os.getpid(),'pins':pins}
 def save():
  tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
 save();runtime=T/'rrrah-loftr-native-runtime-20261010';env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');env['PATH']=str(runtime/'bin')+':'+env['PATH'];log=T/'rrrah-loftr-masked-value-red-build-20261012.jsonl';err=log.with_suffix('.stderr');cmd=['cargo','test','--no-run','--message-format=json','--lib','--locked','--offline','--manifest-path',str(h/'Cargo.toml'),'--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')]
 with log.open('w') as o,err.open('w') as e:
  child=subprocess.Popen(cmd,cwd='/tmp',env=env,stdout=o,stderr=e);r['build_pid']=child.pid;save();rc=child.wait()
 pins[str(log)]=sha(log);pins[str(err)]=sha(err);r['build_returncode']=rc;save();assert rc==0
 artifacts=[json.loads(x) for x in log.read_text().splitlines() if x.startswith('{')];paths=[x['executable'] for x in artifacts if x.get('reason')=='compiler-artifact' and x.get('executable') and x['profile']['test']];assert len(paths)==1
 exe=T/'rrrah-loftr-masked-value-red-executable-20261012';assert not exe.exists();shutil.copy2(paths[0],exe);pins[str(exe)]=sha(exe);cmd=['/usr/bin/env','DYLD_LIBRARY_PATH='+str(runtime/'lib/python3.12/site-packages/torch/lib'),str(exe),'linear_attention::tests::','--test-threads=1'];r.update(status='executing_reproducer',command=cmd);save();run=subprocess.run(cmd,cwd='/tmp',capture_output=True,text=True);log=T/'rrrah-loftr-masked-value-red-tests-20261012.log';log.write_text(run.stdout+run.stderr);pins[str(log)]=sha(log);failed=re.findall(r'^test (\S+) \.\.\. FAILED$',run.stdout,re.M);r.update(returncode=run.returncode,failed=failed,log=str(log),status='reproduced_three_current_source_failures' if run.returncode and len(failed)==3 else 'unexpected_outcome');save();assert run.returncode!=0 and len(failed)==3;print('Reproduced three current-source masked-value failures')
if __name__=='__main__':main()
