"""Qualify compact negative-search diagnostics on a separate source snapshot."""
import pathlib,subprocess,shutil,json,hashlib,os
R=pathlib.Path(__file__).resolve().parents[1];T=pathlib.Path('/Users/themoretheless/.codex/tmp');W=T/'rrrah-model-miss-diagnostics-20261012';S=W/'source';B=T/'rrrah-cache-interpretation-fix-20261012/target';P=R/'docs/research/dedup-model-miss-diagnostics-20261012.json'
assert not W.exists() and not P.exists();W.mkdir()
data={'status':'preparing','producer_pid':os.getpid(),'source':str(S),'target':str(B),'gates':[]}
def save():P.write_text(json.dumps(data,indent=2)+'\n')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
save();subprocess.run(['cp','-cpR',str(T/'rrrah-cache-interpretation-fix-20261012/source'),str(S)],check=True)
files=['src/model_banks.rs','src/search_portfolio.rs','tests/model_compare.rs','tests/portfolio_collection.rs']
for f in files:shutil.copy2(R/'crates/rrrah-dedup'/f,S/'crates/rrrah-dedup'/f)
data['inputs']={f:sha(S/'crates/rrrah-dedup'/f) for f in files};save()
for test,pattern in [('model_compare',None),('search_portfolio','native_bank'),('portfolio_collection','native_bank')]:
 cmd=['cargo','test','-p','rrrah-dedup','--features','decode','--locked','--offline','--test',test,'--jobs','2','--target-dir',str(B)]
 if pattern:cmd.append(pattern)
 log=W/(test+'.log');gate={'test':test,'filter':pattern,'command':cmd,'log':str(log),'status':'running'};data['gates'].append(gate);data['status']='running';save()
 with log.open('x') as out:
  child=subprocess.Popen(cmd,cwd=S,stdout=out,stderr=subprocess.STDOUT);gate['child_pid']=child.pid;save();code=child.wait()
 gate.update(returncode=code,status='passed' if code==0 else 'failed',log_sha256=sha(log));save()
 if code:break
data.update(status='passed' if len(data['gates'])==3 and all(g['returncode']==0 for g in data['gates']) else 'failed',inputs_unchanged=all(sha(S/'crates/rrrah-dedup'/f)==h for f,h in data['inputs'].items()));save();print(data['status'])
