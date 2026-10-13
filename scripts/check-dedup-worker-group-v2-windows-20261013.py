#!/usr/bin/env python3
import json,hashlib,subprocess,os
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');A=D/'dedup-worker-group-v2-source-manifest-20261013.json';a=json.loads(A.read_text());S=Path(a['source']);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert all(sha(S/k)==h for k,h in a['source_sha256'].items());P=D/'dedup-worker-group-v2-windows-check-20261013.json';L=T/'rrrah-worker-group-v2-windows-check-20261013.log';assert not P.exists() and not L.exists();cmd=['cargo','check','-p','rrrah-dedup','--lib','--test','worker','--test','worker_group','--target','x86_64-pc-windows-gnu','--offline','--locked','--jobs','2','--target-dir',str(T/'rrrah-worker-group-target-20261013')];d=dict(status='running',producer_pid=os.getpid(),source=str(S),source_manifest=str(A),source_manifest_sha256=sha(A),command=cmd,log=str(L),scope='Cross-check default-feature library/single-worker/group-target cfg for Windows GNU. Process-group API/tests are excluded; this is compile checking only, no Windows execution, linking, decode or LoFTR qualification.')
def save():
 q=P.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(P)
save()
with L.open('x') as f:
 c=subprocess.Popen(cmd,cwd=S,stdout=f,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();code=c.wait()
d.update(status='checked_pending_audit' if code==0 else 'terminal_failure',returncode=code,log_sha256=sha(L),source_unchanged=all(sha(S/k)==h for k,h in a['source_sha256'].items()));save();print(d['status'])
