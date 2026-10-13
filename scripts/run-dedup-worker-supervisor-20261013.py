"""Execute real subprocess lifecycle tests on a separate frozen source copy."""
import pathlib,subprocess,shutil,json,hashlib,os
R=pathlib.Path(__file__).resolve().parents[1];T=pathlib.Path('/Users/themoretheless/.codex/tmp');W=T/'rrrah-worker-supervisor-20261013';S=W/'source';P=R/'docs/research/dedup-worker-supervisor-20261013.json';assert not W.exists() and not P.exists();W.mkdir()
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
data={'status':'preparing','producer_pid':os.getpid(),'source':str(S)}
def save():P.write_text(json.dumps(data,indent=2)+'\n')
save();subprocess.run(['cp','-cpR',str(T/'rrrah-model-miss-diagnostics-20261012/source'),str(S)],check=True)
files=['src/lib.rs','src/worker.rs','tests/worker.rs']
for f in files:shutil.copy2(R/'crates/rrrah-dedup'/f,S/'crates/rrrah-dedup'/f)
data['inputs']={f:sha(S/'crates/rrrah-dedup'/f) for f in files};cmd=['cargo','test','-p','rrrah-dedup','--no-default-features','--locked','--offline','--test','worker','--jobs','2','--target-dir',str(T/'rrrah-cache-interpretation-fix-20261012/target')];log=W/'test.log';data.update(status='running',command=cmd,log=str(log));save()
with log.open('x') as out:
 child=subprocess.Popen(cmd,cwd=S,stdout=out,stderr=subprocess.STDOUT);data['child_pid']=child.pid;save();code=child.wait()
data.update(status='passed' if code==0 else 'failed',returncode=code,log_sha256=sha(log),inputs_unchanged=all(sha(S/'crates/rrrah-dedup'/f)==h for f,h in data['inputs'].items()));save();print(data['status'])
