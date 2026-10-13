#!/usr/bin/env python3
import json,hashlib,subprocess,os
from pathlib import Path
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');S=T/'rrrah-consensus-source-20261013';D=R/'docs/research'
P=D/'dedup-consensus-complete-decode-20261013.json';L=T/'rrrah-consensus-complete-decode-20261013.log';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins={str(p.relative_to(S)):sha(p) for p in S.rglob('*') if p.is_file() and 'target' not in p.relative_to(S).parts and '.git' not in p.relative_to(S).parts}
for folder in ['crates/rrrah-dedup','crates/rrrah-core','crates/rrrah-decode','crates/rrrah-memory']:
 for p in (R/folder).rglob('*'):
  if p.is_file() and 'target' not in p.relative_to(R).parts: assert pins[str(p.relative_to(R))]==sha(p)
cmd=['cargo','test','-p','rrrah-dedup','--features','decode','--lib','--tests','--offline','--locked','--no-fail-fast','--jobs','2','--target-dir',str(T/'rrrah-cache-interpretation-fix-20261012/target')]
d=dict(status='running',producer_pid=os.getpid(),source=str(S),source_sha256=pins,command=cmd,log=str(L),scope='Complete rrrah-dedup default+decode gate after opt-in guarded two-direction native bank consensus plus prior worker and vendor fixes; fresh current vendor snapshot verified during preparation, four involved local crates matched current root before launch. Does not qualify loftr runtime, other platforms, whole workspace, precision or all formats.')
def save():
 q=P.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(P)
assert not P.exists() and not L.exists();save()
with L.open('x') as f:
 c=subprocess.Popen(cmd,cwd=S,stdout=f,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();code=c.wait()
d.update(status='passed_pending_audit' if code==0 else 'terminal_failure',returncode=code,log_sha256=sha(L),source_unchanged=all(sha(S/p)==h for p,h in pins.items()));save();print(d['status'])
