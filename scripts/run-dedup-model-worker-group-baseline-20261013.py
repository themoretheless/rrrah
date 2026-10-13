#!/usr/bin/env python3
import json,subprocess,hashlib,os
from pathlib import Path
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');P=R/'docs/research/dedup-model-worker-group-baseline-20261013.json';d=json.loads(P.read_text());assert d['status']=='prepared';S=Path(d['source']);L=T/'rrrah-model-worker-group-baseline-20261013.log';assert not L.exists();cmd=['cargo','test','-p','rrrah-dedup','--features','decode','--lib','successful_worker_response_does_not_leave_background_process','--offline','--locked','--jobs','2','--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')];d.update(status='running',producer_pid=os.getpid(),command=cmd,log=str(L))
def save():
 q=P.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(P)
save()
with L.open('x') as f:
 c=subprocess.Popen(cmd,cwd=S,stdout=f,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();code=c.wait()
d.update(status='terminal_baseline_execution_pending_audit',returncode=code,log_sha256=hashlib.sha256(L.read_bytes()).hexdigest(),source_unchanged=all(hashlib.sha256((S/k).read_bytes()).hexdigest()==h for k,h in d['source_sha256'].items()));save();print(code)
