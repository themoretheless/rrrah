#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');Q=D/'dedup-consensus-native-plan-20261013.json';q=json.loads(Q.read_text());old=Path(q['harness']);S=T/'rrrah-gradient-runner-source-20261014';H=T/'rrrah-runner-native-harness-20261014';H.mkdir(exist_ok=False);(H/'src/bin').mkdir(parents=True)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert all(sha(old/k)==h for k,h in q['harness_sha256'].items())
for rel in ['Cargo.toml','Cargo.lock','src/native_policy.rs','src/bin/native_bank_current_all_positives.rs']:
 src=old/rel;dst=H/rel
 if rel=='Cargo.toml':dst.write_text(src.read_text().replace(q['source'],str(S)))
 else:dst.write_bytes(src.read_bytes())
P=D/'dedup-gradient-runner-source-20261014.json';pins=json.loads(P.read_text())['source_sha256'];assert all(sha(S/k)==h for k,h in pins.items())
F=T/'rrrah-runner-native-probe-20261014';F.mkdir(exist_ok=False);L=F/'build.log';O=D/'dedup-runner-native-probe-20261014.json';assert not O.exists()
cmd=['cargo','+stable','build','--offline','--locked','--bin','native_bank_current_all_positives','--jobs','2','--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')]
env=os.environ.copy();env['LIBTORCH_USE_PYTORCH']='1';env['PATH']=str(T/'rrrah-loftr-native-runtime-20261010/bin')+os.pathsep+env['PATH'];env['DYLD_LIBRARY_PATH']=str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib')
d=dict(status='building',pid=os.getpid(),source=str(S),harness=str(H),source_manifest=str(P),pins={str(p):sha(p) for p in [P,Q,Path(__file__),*[H/r for r in ['Cargo.toml','Cargo.lock','src/native_policy.rs','src/bin/native_bank_current_all_positives.rs']]]},command=cmd,log=str(L),scope='Actual native atomic consensus probe linked to frozen managed linear-runner source; exact retained policy harness. Build only until execution reports exist. Not whole-operation speedup evidence.')
def save():
 tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(d,indent=2)+'\n');tmp.replace(O)
save()
with L.open('x') as log:
 c=subprocess.Popen(cmd,cwd=H,env=env,stdout=log,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();rc=c.wait()
d.update(returncode=rc,status='build_failed' if rc else 'built_pending_execution',log_sha256=sha(L),source_unchanged=all(sha(S/k)==h for k,h in pins.items()))
if not rc:
 exe=F/'probe';subprocess.run(['cp','-cp',str(T/'rrrah-dedup-exact-portfolio-20261011/debug/native_bank_current_all_positives'),str(exe)],check=True);d['executable']=str(exe);d['executable_sha256']=sha(exe)
save();print(d['status'])
