"""Run targets unexecuted by the combined gate after container harness qualification."""
import hashlib,json,os,pathlib,re,subprocess
R=pathlib.Path(__file__).resolve().parents[1]
T=pathlib.Path('/Users/themoretheless/.codex/tmp')
S=T/'rrrah-container-writable-tests-20261013'
P=R/'docs/research/dedup-current-combined-remaining-20261013.json'
L=T/'rrrah-current-combined-remaining-20261013.log'
h=lambda p:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
assert not P.exists() and not L.exists()
control=(T/'rrrah-container-writable-tests-20261013.log').read_text()
assert re.search(r'test result: ok\. 14 passed; 0 failed; 0 ignored;',control)
old=(T/'rrrah-current-combined-validation-v2-20261013/full.log').read_text()
ran=set(re.findall(r'Running tests/(\w+)\.rs',old))
names=sorted({p.stem for p in (S/'crates/rrrah-dedup/tests').glob('*.rs')}-ran)
assert len(names)==103
m=json.loads((S/'validation-snapshot.json').read_text())
changed='crates/rrrah-dedup/tests/container.rs'
assert all(h(S/p)==v for p,v in m['files'].items() if p!=changed)
pins={p:h(S/p) for p in m['files']}
cmd=['cargo','test','-p','rrrah-dedup','--features','decode','--offline','--locked','--jobs','2','--target-dir',str(T/'rrrah-cache-interpretation-fix-20261012/target')]
for name in names:cmd.extend(['--test',name])
d=dict(status='running',producer_pid=os.getpid(),source=str(S),source_sha256=pins,targets=names,command=cmd,log=str(L),scope='103 integration targets unexecuted by failed combined gate; source differs only in mutable container fixture setup. Excludes subsequent mesh_refine contrast and visual/local publication fixes, loftr feature and other platforms.')
def save():P.write_text(json.dumps(d,indent=2)+'\n')
save()
with L.open('x') as out:
 child=subprocess.Popen(cmd,cwd=S,stdout=out,stderr=subprocess.STDOUT);d['child_pid']=child.pid;save();code=child.wait()
d.update(status='passed_pending_audit' if code==0 else 'terminal_failure',returncode=code,log_sha256=h(L),source_unchanged=all(h(S/p)==v for p,v in pins.items()));save();print(d['status'])
