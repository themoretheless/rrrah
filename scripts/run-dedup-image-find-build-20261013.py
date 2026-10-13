#!/usr/bin/env python3
from pathlib import Path
import subprocess,json,hashlib,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp');F=T/'rrrah-image-find-controls-20261013';P=D/'image-find-build-20261013.json';L=T/'rrrah-image-find-build-20261013.log';assert not P.exists() and not L.exists();m=json.loads((D/'image-find-full-source-20261013.json').read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert all(sha(F/r['path'])==r['sha256'] for r in m['files']);cmd=['cargo','build','--locked','--jobs','2','--target-dir',str(T/'rrrah-image-find-target-20261013')];d=dict(status='running',producer_pid=os.getpid(),command=cmd,source=str(F),log=str(L),compiler=subprocess.run(['rustc','-vV'],cwd=F,capture_output=True,text=True,check=True).stdout)
def save():
 q=P.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(P)
save()
with L.open('x') as log:
 c=subprocess.Popen(cmd,cwd=F,stdout=log,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();code=c.wait()
d.update(status='built_pending_controls' if code==0 else 'terminal_build_failure',returncode=code,log_sha256=sha(L),source_unchanged=all(sha(F/r['path'])==r['sha256'] for r in m['files']));save();print(d['status'])
