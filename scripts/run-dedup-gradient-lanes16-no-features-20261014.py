#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,os,subprocess,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-gradient-lanes16-managed-20261014.json';m=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert m['status']=='passed_pending_audit'
S=Path(m['source']);assert all(sha(S/p)==h for p,h in m['source_sha256'].items())and all(sha(p)==h for p,h in m['dependency_pins'].items())
F=Path('/private/tmp/rrrah-gradient-lanes16-no-features-20261014');F.mkdir(exist_ok=False);command=[v for v in m['command']if v not in ['--cfg','feature="raster"']];command[command.index('-o')+1]=str(F/'tests');env=os.environ.copy();env['CARGO_MANIFEST_DIR']=str(S/'crates/rrrah-dedup');O=D/'dedup-gradient-lanes16-no-features-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),command=command,manifest=str(P),manifest_sha256=sha(P),dependency_pins=m['dependency_pins'],pins={str(Path(__file__)):sha(__file__)},scope='Exact isolated actual-layout firstpass16candidate unit library, all optional features off. Directrustc pinned real deps. Full default unit inventory with all-tail scalar/cancellation/reference controls and duration regressions; not Cargo/decode/native/workspace/corpus/platform proof. Working runtime kernel unchanged.')
def save():
    tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
save()
with (F/'build.log').open('x')as log:
    p=subprocess.Popen(command,cwd=S,env=env,stdout=log,stderr=subprocess.STDOUT);state['child_pid']=p.pid;save();code=p.wait()
state['build_returncode']=code;save();assert code==0,(F/'build.log').read_text()[-5000:]
exe=F/'tests';state['pins'][str(exe)]=sha(exe)
with (F/'inventory.log').open('x')as log:
    p=subprocess.Popen([str(exe),'--list'],stdout=log,stderr=subprocess.STDOUT);state.update(status='listing',child_pid=p.pid);save();assert p.wait()==0
with (F/'tests.log').open('x')as log:
    p=subprocess.Popen([str(exe),'--test-threads','2'],cwd=S,env=env,stdout=log,stderr=subprocess.STDOUT);state.update(status='testing',child_pid=p.pid);save();code=p.wait()
state['test_returncode']=code;save();log=(F/'tests.log').read_text();summary=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',log);assert code==0 and summary and (int(summary[1]),int(summary[2]),int(summary[3]))==(45,0,2)
assert all(sha(S/p)==h for p,h in m['source_sha256'].items());state.update(status='passed_pending_audit',passed=45,failed=0,ignored=2);state['pins'].update({str(p):sha(p)for p in [F/'build.log',F/'inventory.log',F/'tests.log']});save();print(O)
