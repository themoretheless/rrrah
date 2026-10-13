#!/usr/bin/env python3
from pathlib import Path
import subprocess,json,hashlib,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');P=D/'dedup-image-find-native-crosscheck-plan-20261013.json';p=json.loads(P.read_text());sha=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest();assert all(sha(k)==h for k,h in p['pins'].items());S=Path(p['source']);assert all(sha(S/k)==h for k,h in p['source_sha256'].items());H=Path(p['harness']);O=D/'dedup-image-find-native-crosscheck-20261013.json';assert not O.exists();L=H/'build.log';assert not L.exists();cmd=['cargo','+stable','build','--offline','--locked','--jobs','2','--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')];d=dict(status='building',producer_pid=os.getpid(),plan_sha256=sha(P),command=cmd,pins={str(P):sha(P),str(Path(__file__)):sha(__file__)},scope=p['scope'])
def save():
 q=O.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(O)
save()
with L.open('x') as f:
 c=subprocess.Popen(cmd,cwd=H,stdout=f,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();code=c.wait()
d['build_returncode']=code;d['pins'][str(L)]=sha(L)
if code:d['status']='terminal_build_failure';save();raise SystemExit(code)
exe=H/'probe';subprocess.run(['cp','-cp',str(T/'rrrah-dedup-exact-portfolio-20261011/debug/dedup-image-find-native-crosscheck'),str(exe)],check=True);d['pins'][str(exe)]=sha(exe);run=subprocess.run([str(exe),str(P)],capture_output=True,text=True,timeout=60);case=H/'case.json';case.write_text(json.dumps(dict(command=[str(exe),str(P)],returncode=run.returncode,stdout=run.stdout,stderr=run.stderr),indent=2)+'\n');d['pins'][str(case)]=sha(case);d.update(status='complete_pending_audit' if run.returncode==0 else 'terminal_runtime_failure',returncode=run.returncode,source_unchanged=all(sha(S/k)==h for k,h in p['source_sha256'].items()),inputs_unchanged=all(sha(k)==h for k,h in p['pins'].items()),case=str(case));save();print(d['status'])
