#!/usr/bin/env python3
import json,hashlib,subprocess,os
from pathlib import Path
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');S=T/'rrrah-visual-publication-writable-source-20261013';D=R/'docs/research';P=D/'dedup-visual-publication-writable-20261013.json';L=T/'rrrah-visual-publication-writable-20261013.log';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins={str(p.relative_to(S)):sha(p) for p in S.rglob('*') if p.is_file() and 'target' not in p.relative_to(S).parts and '.git' not in p.relative_to(S).parts}
names=['visual_publication'];cmd=['cargo','test','-p','rrrah-dedup','--features','decode','--offline','--locked','--jobs','2','--target-dir',str(T/'rrrah-cache-interpretation-fix-20261012/target')]
for n in names:cmd+=['--test',n]
d=dict(status='running',producer_pid=os.getpid(),source=str(S),source_sha256=pins,targets=names,command=cmd,log=str(L),scope='Focused byte-copy fixture repair only; frozen full-gate baseline plus visual_publication test edit, no production acceptance change.')
def save():
 q=P.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(P)
assert not P.exists() and not L.exists();save()
with L.open('x') as f:
 c=subprocess.Popen(cmd,cwd=S,stdout=f,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();code=c.wait()
d.update(status='passed_pending_audit' if code==0 else 'terminal_failure',returncode=code,log_sha256=sha(L),source_unchanged=all(sha(S/p)==h for p,h in pins.items()));save();print(d['status'])
