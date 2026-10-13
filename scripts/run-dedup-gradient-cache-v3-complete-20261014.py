#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,os,sys
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');mode='complete-decode';P=D/'dedup-gradient-cache-v3-source-20261013.json';p=json.loads(P.read_text());S=Path(p['source']);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert all(sha(S/k)==h for k,h in p['source_sha256'].items());O=D/f'dedup-gradient-cache-v3-{mode}-20261013.json';L=T/f'rrrah-gradient-cache-v3-{mode}-20261013.log';assert not O.exists() and not L.exists();cmd=['cargo','+stable','test','-p','rrrah-dedup','--lib','--tests','--features','decode'];cmd+=['--offline','--locked','--no-fail-fast','--jobs','2','--target-dir',str(T/'rrrah-cache-interpretation-fix-20261012/target' if mode=='complete-decode' else T/'rrrah-worker-group-target-20261013')];d=dict(status='running',mode=mode,producer_pid=os.getpid(),source=str(S),source_manifest=str(P),source_manifest_sha256=sha(P),producer_sha256=sha(__file__),command=cmd,log=str(L),compiler=subprocess.run(['rustc','+stable','-vV'],cwd=S,capture_output=True,text=True,check=True).stdout,scope='Execute from fresh frozen current selected source after formatting drift and isolated16MiBcache-child refinement. Full declared library and integration tests with default+decode on macOS ARM64. Not allfeatures/platform/corpus or whole-operation speedup proof.')
def save():
 q=O.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(O)
save()
with L.open('x') as f:
 c=subprocess.Popen(cmd,cwd=S,stdout=f,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();code=c.wait()
d.update(status='passed_pending_audit' if code==0 else 'terminal_test_failure',returncode=code,log_sha256=sha(L),source_unchanged=all(sha(S/k)==h for k,h in p['source_sha256'].items()));save();print(mode,d['status'])
