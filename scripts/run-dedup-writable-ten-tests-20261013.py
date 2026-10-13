#!/usr/bin/env python3
import json,hashlib,subprocess,os
from pathlib import Path
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');S=T/'rrrah-writable-ten-tests-20261013'
P=R/'docs/research/dedup-writable-ten-tests-20261013.json';L=T/'rrrah-writable-ten-tests-20261013.log'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
a=json.loads((R/'docs/research/dedup-fingerprint-nonfinite-audit-20261013.json').read_text())
# Inventory the immutable qualification source, including all inherited files.
pins={str(p.relative_to(S)):sha(p) for p in S.rglob('*') if p.is_file() and 'target' not in p.relative_to(S).parts and '.git' not in p.relative_to(S).parts}
names='local_collection local_scan photo_files pixel_search presentation_kind projective_files region_grid_files region_transform scan scan_group_publication'.split()
cmd=['cargo','test','-p','rrrah-dedup','--features','decode','--offline','--locked','--no-fail-fast','--jobs','2','--target-dir',str(T/'rrrah-cache-interpretation-fix-20261012/target')]
for n in names:cmd+=['--test',n]
d=dict(status='running',producer_pid=os.getpid(),source=str(S),source_sha256=pins,targets=names,command=cmd,log=str(L),scope='Ten repaired fixture targets on frozen finite-statistics source plus ten current test files; not full workspace qualification.')
def save():
 q=P.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(P)
assert not P.exists() and not L.exists();save()
with L.open('x') as f:
 c=subprocess.Popen(cmd,cwd=S,stdout=f,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();code=c.wait()
d.update(status='passed_pending_audit' if code==0 else 'terminal_failure',returncode=code,log_sha256=sha(L),source_unchanged=all(sha(S/p)==h for p,h in pins.items()));save();print(d['status'])
