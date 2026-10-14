#!/usr/bin/env python3
"""Compile actual current root dedup tests with pinned real deps and no decoder feature."""
from pathlib import Path
import hashlib,json,subprocess,os,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior_path=D/'dedup-gradient-unroll4-managed-focused-v2-20261014.json';prior=json.loads(prior_path.read_text());assert prior['status']=='passed_pending_audit'and all(sha(p)==h for p,h in prior['dependency_pins'].items())
F=Path('/private/tmp/rrrah-gradient-lanes16-root-no-features-20261014');F.mkdir(exist_ok=False)
root_crate=R/'crates/rrrah-dedup';source_pins={str(p.relative_to(R)):sha(p)for p in root_crate.rglob('*')if p.is_file()}
original_source=Path(prior['source'])/'crates/rrrah-dedup/src/lib.rs'
command=[str(root_crate/'src/lib.rs')if v==str(original_source)else str(F/'tests')if v==prior['executable']else v for v in prior['command']]
command=[v for v in command if v not in ['--cfg','feature="raster"']]
env=os.environ.copy();env['CARGO_MANIFEST_DIR']=str(root_crate)
O=D/'dedup-gradient-lanes16-root-no-features-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),command=command,source_sha256=source_pins,dependency_pins=prior['dependency_pins'],pins={str(p):sha(p)for p in [Path(__file__),prior_path]},scope='Actual adopted current-root firstpass16MacARM/4other, secondpass4, unit build with all optional feature cfgs disabled, directrustc and real pinned cached dependencies. Actual target-gated firstpass16kernel and all-tail/cancellation/reference/budget tests, plus duration regressions; no animation/realdecode/Cargo/fullworkspace/native/corpus/platform claim.')
def save():
    tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
save()
with (F/'build.log').open('x')as log:
    p=subprocess.Popen(command,cwd=R,env=env,stdout=log,stderr=subprocess.STDOUT);state['child_pid']=p.pid;save();code=p.wait()
state['build_returncode']=code;save();assert code==0,(F/'build.log').read_text()[-5000:]
exe=F/'tests';state['pins'][str(exe)]=sha(exe)
with (F/'inventory.log').open('x')as log:
    p=subprocess.Popen([str(exe),'--list'],stdout=log,stderr=subprocess.STDOUT);state.update(status='listing',child_pid=p.pid);save();assert p.wait()==0
with (F/'tests.log').open('x')as log:
    p=subprocess.Popen([str(exe),'--test-threads','2'],cwd=R,env=env,stdout=log,stderr=subprocess.STDOUT);state.update(status='testing',child_pid=p.pid);save();code=p.wait()
state['test_returncode']=code;save();text=(F/'tests.log').read_text();summary=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',text);assert code==0 and summary
assert int(summary[1])>=2 and int(summary[2])==0
listed=re.findall(r'^(.+): test$',(F/'inventory.log').read_text(),re.M)
executed=re.findall(r'^test (.+) \.\.\. (ok|ignored(?:,.*)?)$',text,re.M)
assert sorted(listed)==sorted(n for n,_ in executed)and len(listed)==int(summary[1])+int(summary[3])
for name in ['large_shared_denominator_reduces_before_overflow','sum_retains_exact_time_and_refuses_unrepresentable_result']:assert f'sequence::duration_sum_tests::{name} ... ok'in text
assert all(sha(R/p)==h for p,h in source_pins.items())and all(sha(p)==h for p,h in state['dependency_pins'].items())
state.update(status='passed_pending_audit',passed=int(summary[1]),failed=0,ignored=int(summary[3]))
state['pins'].update({str(p):sha(p)for p in [F/'build.log',F/'inventory.log',F/'tests.log']});save();print(O)
