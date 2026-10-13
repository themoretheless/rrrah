"""Qualify protocol and build the actual one-shot CPU LoFTR worker."""
import pathlib,json,hashlib,shutil,subprocess,os
R=pathlib.Path(__file__).resolve().parents[1];D=R/'docs/research';T=pathlib.Path('/Users/themoretheless/.codex/tmp');W=T/'rrrah-model-worker-20261013';S=W/'source';P=D/'dedup-model-worker-20261013.json';assert not W.exists() and not P.exists();W.mkdir()
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
data={'status':'preparing','producer_pid':os.getpid(),'source':str(S),'gates':[]}
def save():P.write_text(json.dumps(data,indent=2)+'\n')
save();subprocess.run(['cp','-cpR',str(T/'rrrah-worker-supervisor-20261013/source'),str(S)],check=True)
files=['Cargo.toml','src/lib.rs','src/model_loftr.rs','src/model_worker.rs','src/bin/rrrah-loftr-worker.rs']
for f in files:
 path=S/'crates/rrrah-dedup'/f;path.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(R/'crates/rrrah-dedup'/f,path)
data['inputs']={f:sha(S/'crates/rrrah-dedup'/f) for f in files};save();target=T/'rrrah-cache-interpretation-fix-20261012/target'
commands=[('protocol',['cargo','test','-p','rrrah-dedup','--features','decode','--locked','--offline','--lib','model_worker::','--jobs','2','--target-dir',str(target)]),('native_worker_build',['cargo','build','-p','rrrah-dedup','--features','loftr','--locked','--offline','--bin','rrrah-loftr-worker','--jobs','2','--target-dir',str(target)])]
env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');runtime=T/'rrrah-loftr-native-runtime-20261010';env['PATH']=str(runtime/'bin')+':'+env['PATH'];env['DYLD_LIBRARY_PATH']=str(runtime/'lib/python3.12/site-packages/torch/lib')
for name,cmd in commands:
 log=W/(name+'.log');g={'name':name,'command':cmd,'log':str(log),'status':'running'};data['gates'].append(g);data['status']='running';save()
 with log.open('x') as out:
  child=subprocess.Popen(cmd,cwd=S,env=env,stdout=out,stderr=subprocess.STDOUT);g['child_pid']=child.pid;save();code=child.wait()
 g.update(returncode=code,status='passed' if code==0 else 'failed',log_sha256=sha(log));save()
 if code:break
if len(data['gates'])==2 and all(g['returncode']==0 for g in data['gates']):
 binary=W/'rrrah-loftr-worker';shutil.copy2(target/'debug/rrrah-loftr-worker',binary);data.update(binary=str(binary),binary_sha256=sha(binary),status='passed')
else:data['status']='failed'
data['inputs_unchanged']=all(sha(S/'crates/rrrah-dedup'/f)==h for f,h in data['inputs'].items());save();print(data['status'])
