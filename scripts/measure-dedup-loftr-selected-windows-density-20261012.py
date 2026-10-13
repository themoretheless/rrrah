#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 audit=D/'dedup-loftr-selected-windows-stage-audit-20261012.json';a=json.loads(audit.read_text());assert all(sha(p)==h for p,h in a['pins'].items())
 old=T/'rrrah-loftr-selected-windows-test-harness-v2-20261012';h=T/'rrrah-loftr-selected-windows-density-harness-20261012';assert not h.exists();shutil.copytree(old,h)
 p=h/'src/fine_preprocess/tests.rs';code=p.read_text();assert code.count('let side=320_i64;let count=256_i64;')==1;code=code.replace('let side=320_i64;let count=256_i64;','let side=320_i64;let count=std::env::var("RRRAH_SELECTED_WINDOW_COUNT").unwrap().parse::<i64>().unwrap();assert!((1..=12800).contains(&count));').replace('Tensor::arange(count,(Kind::Int64,Device::Cpu))*23','(Tensor::arange(count,(Kind::Int64,Device::Cpu))*23).remainder(6400)').replace('Tensor::arange(count,(Kind::Int64,Device::Cpu))*19','(Tensor::arange(count,(Kind::Int64,Device::Cpu))*19).remainder(6400)');p.write_text(code)
 pins={str(p):sha(p) for p in [audit,Path(__file__),*[x for x in h.rglob('*') if x.is_file()]]};out=D/'dedup-loftr-selected-windows-density-20261012.json';assert not out.exists();r={'status':'building','producer_pid':os.getpid(),'pins':pins,'results':[],'scope':'Same v2 candidate production, only ignored benchmark test accepts fixed counts and repeats indices modulo6400. Fresh-process stage controls at256,6400,12800windows from6400positions. Not full-model or allcase memory bound.'}
 def save():
  tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
 env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');runtime=T/'rrrah-loftr-native-runtime-20261010';env['PATH']=str(runtime/'bin')+':'+env['PATH'];cmd=['cargo','test','--no-run','--message-format=json','--manifest-path',str(h/'Cargo.toml'),'--lib','--locked','--offline','--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')];save();log=T/'rrrah-loftr-selected-windows-density-build-20261012.jsonl';err=log.with_suffix('.stderr')
 with log.open('w') as o,err.open('w') as e:
  child=subprocess.Popen(cmd,cwd='/tmp',env=env,stdout=o,stderr=e);r['build_pid']=child.pid;save();rc=child.wait()
 pins[str(log)]=sha(log);pins[str(err)]=sha(err);r['build_returncode']=rc;save();assert rc==0
 artifacts=[json.loads(line) for line in log.read_text().splitlines() if line.startswith('{')];paths=[x['executable'] for x in artifacts if x.get('reason')=='compiler-artifact' and x.get('executable') and x['profile']['test']];assert len(paths)==1
 exe=T/'rrrah-loftr-selected-windows-density-executable-20261012';assert not exe.exists();shutil.copy2(paths[0],exe);pins[str(exe)]=sha(exe);r['status']='running';save()
 for count in [256,6400,12800]:
  for mode in ['dense','selected']:
   cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(runtime/'lib/python3.12/site-packages/torch/lib'),'RRRAH_SELECTED_WINDOW_CONTROL='+mode,'RRRAH_SELECTED_WINDOW_COUNT='+str(count),str(exe),'--exact','fine_preprocess::tests::selected_windows_memory_control','--ignored','--nocapture','--test-threads=1'];start=time.monotonic();run=subprocess.run(cmd,cwd='/tmp',capture_output=True,text=True);log=T/f'rrrah-loftr-selected-windows-density-{count}-{mode}-20261012.log';log.write_text(run.stdout+run.stderr);pins[str(log)]=sha(log);row={'count':count,'mode':mode,'command':cmd,'returncode':run.returncode,'elapsed_seconds':time.monotonic()-start,'stdout':run.stdout,'stderr':run.stderr,'log':str(log)};r['results'].append(row);save();assert run.returncode==0
   row.update(hash=re.search(r'hash=([0-9a-f]{16})',run.stdout)[1],rss_bytes=int(re.search(r'(\d+)\s+maximum resident set size',run.stderr)[1]));save();print(count,mode,row['rss_bytes'],row['hash'],flush=True)
  assert r['results'][-1]['hash']==r['results'][-2]['hash']
 r['status']='complete_density_controls_not_adopted';save()
if __name__=='__main__':main()
