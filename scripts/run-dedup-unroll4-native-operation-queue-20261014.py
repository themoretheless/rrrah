#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import os
import resource
import signal
import subprocess
import sys
import time

R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-unroll4-native-operation-plan-20261014.json'
plan=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert all(sha(p)==h for p,h in plan['pins'].items())
for version in plan['versions'].values():
    M=Path(version['source_manifest']);assert sha(M)==version['source_manifest_sha256'];m=json.loads(M.read_text());S=Path(m['source']);assert all(sha(S/p)==h for p,h in m['source_sha256'].items());H=Path(version['harness']);assert all(sha(H/p)==h for p,h in version['harness_pins'].items())
O=D/'dedup-unroll4-native-operation-20261014.json';assert not O.exists();F=Path('/private/tmp/rrrah-unroll4-native-operation-20261014')
state=dict(status='waiting_for_existing_native_gate',producer_pid=os.getpid(),plan=str(P),plan_sha256=sha(P),producer_sha256=sha(__file__),builds=[],results=[],scope=plan['scope'])

def save():
    temporary=O.with_suffix('.tmp');temporary.write_text(json.dumps(state,indent=2)+'\n');temporary.replace(O)

def interrupted(signum,frame):raise KeyboardInterrupt(f'signal {signum}')

signal.signal(signal.SIGTERM,interrupted);signal.signal(signal.SIGINT,interrupted)
save();child=None
try:
    gate=D/'dedup-native-collection-focused-20261014.json'
    while True:
        report=json.loads(gate.read_text())
        if report['status'] in ['building','testing']:
            time.sleep(10);continue
        assert report['status']=='passed_pending_audit' and report['source_unchanged'],report
        break
    audit=R/plan['required_first_audit']
    if not audit.exists():subprocess.run([sys.executable,str(R/'scripts/audit-dedup-native-collection-focused-20261014.py')],cwd=R,check=True)
    audited=json.loads(audit.read_text());assert audited['status']=='verified_frozen_native_collection_adapter_two_integration_targets'
    state['first_audit_sha256']=sha(audit)
    runtime=Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010')
    env=os.environ.copy();env.update(CARGO_HOME='/private/tmp/rrrah-runner-full-cargo-home-20261014',LIBTORCH_USE_PYTORCH='1',PATH=str(runtime/'bin')+os.pathsep+env['PATH'],DYLD_LIBRARY_PATH=str(runtime/'lib/python3.12/site-packages/torch/lib'))
    for name,version in plan['versions'].items():
        folder=F/name;H=Path(version['harness']);command=['cargo','+stable','build','--offline','--locked','--bin','native_bank_current_all_positives','--jobs','2','--target-dir',plan['target']]
        state.update(status='building',active_version=name);save()
        with (folder/'build.log').open('x') as log:
            child=subprocess.Popen(command,cwd=H,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);state['child_pid']=child.pid;save();code=child.wait();child=None
        assert code==0,(name,code)
        binary=folder/'probe';subprocess.run(['cp','-cp',str(Path(plan['target'])/'debug/native_bank_current_all_positives'),str(binary)],check=True)
        state['builds'].append(dict(version=name,executable=str(binary),executable_sha256=sha(binary),log=str(folder/'build.log'),log_sha256=sha(folder/'build.log')));save()
    outputs={}
    for case in plan['cases']:
        for pair in range(3):
            for version in (['baseline','candidate'] if pair%2==0 else ['candidate','baseline']):
                assert all(sha(p)==plan['pins'][p] for p in case['arguments'][:3])
                binary=F/version/'probe';command=['/usr/bin/time','-l',str(binary),*case['arguments']]
                before=resource.getrusage(resource.RUSAGE_CHILDREN);started=time.monotonic();child=subprocess.Popen(command,cwd=R,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
                state.update(status='running',child_pid=child.pid,active_case=case['label'],active_pair=pair,active_version=version);save();stdout,stderr=child.communicate();code=child.returncode;child=None;after=resource.getrusage(resource.RUSAGE_CHILDREN)
                raw=F/(case['label']+'-'+str(pair)+'-'+version+'.json');raw.write_text(json.dumps(dict(command=command,returncode=code,stdout=stdout,stderr=stderr),indent=2)+'\n');assert code==0,stderr
                value=json.loads(stdout);assert value['managed_used']==0 and value['support'] is True and value['forward_support'] is True and value['reverse_support'] is True
                if case['label'] in outputs:assert outputs[case['label']]==value
                else:outputs[case['label']]=value
                state['results'].append(dict(case=case['label'],pair=pair,version=version,report=str(raw),report_sha256=sha(raw),cpu_seconds=after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime,wall_seconds=time.monotonic()-started,managed_peak=value['managed_peak']));save()
    assert all(sha(p)==h for p,h in plan['pins'].items())
    for version in plan['versions'].values():
        m=json.loads(Path(version['source_manifest']).read_text());assert all(sha(Path(m['source'])/p)==h for p,h in m['source_sha256'].items())
    state['status']='complete_pending_audit';save()
except BaseException as error:
    state.update(status='stopped_on_failure',error=repr(error));save();raise
finally:
    if child is not None and child.poll() is None:
        os.killpg(child.pid,signal.SIGKILL);child.wait()
