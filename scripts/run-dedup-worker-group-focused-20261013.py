#!/usr/bin/env python3
import json,hashlib,subprocess,os
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');S=T/'rrrah-worker-group-source-20261013';P=D/'dedup-worker-group-focused-20261013.json';L=T/'rrrah-worker-group-focused-20261013.log';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={str(p.relative_to(S)):sha(p) for p in S.rglob('*') if p.is_file() and '.git' not in p.relative_to(S).parts and 'target' not in p.relative_to(S).parts};assert not P.exists() and not L.exists();cmd=['cargo','test','-p','rrrah-dedup','--lib','--test','worker','--test','worker_group','--offline','--locked','--no-fail-fast','--jobs','2','--target-dir',str(T/'rrrah-worker-group-target-20261013')];d=dict(status='running',producer_pid=os.getpid(),source=str(S),source_sha256=pins,command=cmd,log=str(L),scope='Frozen default-feature library plus existing single-worker and new process-group tests on macOSARM64; no decode/LoFTR/fullworkspace/Linux/Windows/RSS/sandbox qualification. Separate full default+decode gate predates this worker change.')
def save():
 q=P.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(P)
save()
with L.open('x') as f:
 c=subprocess.Popen(cmd,cwd=S,stdout=f,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();code=c.wait()
d.update(status='passed_pending_audit' if code==0 else 'terminal_failure',returncode=code,log_sha256=sha(L),source_unchanged=all(sha(S/k)==v for k,v in pins.items()));save();print(d['status'])
