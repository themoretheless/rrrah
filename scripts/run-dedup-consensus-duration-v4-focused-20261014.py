#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-consensus-duration-v4-source-20261014.json';m=json.loads(P.read_text());S=Path(m['source']);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert all(sha(S/k)==h for k,h in m['source_sha256'].items());O=D/'dedup-consensus-duration-v4-focused-20261014.json';L=Path('/private/tmp/rrrah-consensus-duration-v4-focused-20261014.log');assert not O.exists() and not L.exists();cmd=['cargo','+stable','test','-p','rrrah-dedup','--features','decode','--lib','--test','search_portfolio','--test','portfolio_collection','--test','animated','--test','sequence','--offline','--locked','--no-fail-fast','--jobs','2','--target-dir','/private/tmp/rrrah-consensus-collection-target-20261014'];env=os.environ.copy();env['CARGO_HOME']='/private/tmp/rrrah-runner-full-cargo-home-20261014';d=dict(status='running',producer_pid=os.getpid(),source=str(S),source_manifest=str(P),source_manifest_sha256=sha(P),producer_sha256=sha(__file__),command=cmd,log=str(L),scope='Frozen final collection v3 plus duration arithmetic fix and real animation timeline-key regression; library, pair, collection, animation and sequence targets. Other sources inherited frozen v3 capture, not current workspace parity. Fake proposal providers are not actual learned inference.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
assert (D/'dedup-consensus-collection-v3-focused-audit-20261014.json').is_file(), 'Preserve final collection v3 binaries before target reuse'
save()
with L.open('x') as f:
 c=subprocess.Popen(cmd,cwd=S,env=env,stdout=f,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();rc=c.wait()
d.update(returncode=rc,status='passed_pending_audit' if not rc else 'test_or_compile_failure',log_sha256=sha(L),source_unchanged=all(sha(S/k)==h for k,h in m['source_sha256'].items()));save();print(d['status'])
