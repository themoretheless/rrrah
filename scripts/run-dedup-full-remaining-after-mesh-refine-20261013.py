"""Qualify the 53 previously unexecuted targets on the frozen contract-fixed snapshot."""
import pathlib,json,subprocess,hashlib,os,re
R=pathlib.Path(__file__).resolve().parents[1]
T=pathlib.Path('/Users/themoretheless/.codex/tmp')
S=T/'rrrah-mesh-refine-error-contract-20261013'
P=R/'docs/research/dedup-full-remaining-after-mesh-refine-20261013.json'
L=T/'rrrah-full-remaining-after-mesh-refine-20261013.log'
old=(T/'rrrah-mesh-rank-error-contract-20261013/full.log').read_text()
ran=set(re.findall(r'Running tests/(\w+)\.rs',old))
names=sorted({p.stem for p in (S/'crates/rrrah-dedup/tests').glob('*.rs')}-ran)
assert len(names)==53 and not P.exists() and not L.exists()
cmd=['cargo','test','-p','rrrah-dedup','--features','decode','--offline','--locked','--jobs','2','--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')]
for name in names: cmd.extend(['--test',name])
d=dict(status='running',source=str(S),producer_pid=os.getpid(),command=cmd,targets=names,log=str(L),scope='53 targets not run by preceding failed gate; frozen source with two corrected alpha error expectations, excludes later cache/model/scan fixes')
def save(): P.write_text(json.dumps(d,indent=2)+'\n')
save()
with L.open('x') as out:
 child=subprocess.Popen(cmd,cwd=S,stdout=out,stderr=subprocess.STDOUT);d['child_pid']=child.pid;save();code=child.wait()
d.update(status='passed_pending_audit' if code==0 else 'terminal_failure',returncode=code,log_sha256=hashlib.sha256(L.read_bytes()).hexdigest());save();print(d['status'])
