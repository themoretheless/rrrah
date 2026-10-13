"""Run the complete decode test gate on a hash-verified current combined snapshot."""
import pathlib,json,subprocess,hashlib,os
R=pathlib.Path(__file__).resolve().parents[1]
T=pathlib.Path('/Users/themoretheless/.codex/tmp')
S=T/'rrrah-current-combined-validation-v2-20261013/source'
P=R/'docs/research/dedup-current-combined-gate-20261013.json'
L=T/'rrrah-current-combined-validation-v2-20261013/full.log'
M=S/'validation-snapshot.json'
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
m=json.loads(M.read_text())
assert not P.exists() and not L.exists()
assert all(h(S/k)==v for k,v in m['files'].items())
cmd=['cargo','test','-p','rrrah-dedup','--features','decode','--offline','--locked','--lib','--tests','--jobs','2','--target-dir',str(T/'rrrah-cache-interpretation-fix-20261012/target')]
d=dict(status='running',source=str(S),source_manifest=str(M),source_manifest_sha256=h(M),source_files=len(m['files']),producer_pid=os.getpid(),command=cmd,log=str(L),scope='Current combined source snapshot, decode library and all available integration tests; excludes loftr feature and other platforms')
def save(): P.write_text(json.dumps(d,indent=2)+'\n')
save()
with L.open('x') as out:
 child=subprocess.Popen(cmd,cwd=S,stdout=out,stderr=subprocess.STDOUT);d['child_pid']=child.pid;save();code=child.wait()
d.update(status='passed_pending_audit' if code==0 else 'terminal_failure',returncode=code,log_sha256=h(L),source_unchanged=all(h(S/k)==v for k,v in m['files'].items()));save();print(d['status'])
