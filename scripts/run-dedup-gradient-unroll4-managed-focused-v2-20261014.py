#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess

R=Path(__file__).resolve().parents[1]
P=R/'docs/research/dedup-gradient-unroll4-v7-source-20261014.json'
m=json.loads(P.read_text());S=Path(m['source'])
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert all(sha(S/name)==pin for name,pin in m['source_sha256'].items())
F=Path('/private/tmp/rrrah-gradient-unroll4-managed-focused-v2-20261014');F.mkdir(exist_ok=False)
deps=Path('/Users/themoretheless/.codex/tmp/rrrah-worker-group-target-20261013/debug/deps')
command=['rustc','+stable','--edition=2024','--crate-name','rrrah_dedup','--test',str(S/'crates/rrrah-dedup/src/lib.rs'),'--cfg','feature="raster"','-C','opt-level=1','-L','dependency='+str(deps),'-o',str(F/'gradient-tests')]
dependency_pins={}
for crate in ['rrrah_core','blake3','thiserror','tempfile','libc','rustix','libproc','errno']:
    candidates=list(deps.glob('lib'+crate+'-*.rlib'));assert candidates,crate
    if crate=='rustix':
        qualified=[]
        for artifact in candidates:
            tag=artifact.stem.removeprefix('lib')
            fingerprint=deps.parent/'.fingerprint'/tag/'lib-rustix.json'
            dependency_file=deps/(tag+'.d')
            if fingerprint.exists() and dependency_file.exists() and 'rustix-1.1.4' in dependency_file.read_text()[:12000] and 'process' in json.loads(fingerprint.read_text())['features']:
                qualified.append(artifact)
        assert qualified, 'Need declared rustix1.1.4 with process feature'
        candidates=qualified
    artifact=max(candidates,key=lambda p:p.stat().st_mtime_ns)
    command+=['--extern',crate+'='+str(artifact)];dependency_pins[str(artifact)]=sha(artifact)
env=os.environ.copy();env['CARGO_MANIFEST_DIR']=str(S/'crates/rrrah-dedup')
O=R/'docs/research/dedup-gradient-unroll4-managed-focused-v2-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),source=str(S),manifest=str(P),manifest_sha256=sha(P),producer_sha256=sha(__file__),command=command,dependency_pins=dependency_pins,scope='Complete frozen rrrah-dedup source compiled directly with raster cfg and real cached rrrah-core/other dependencies. Execute gradient module tests only, including managed cache/runner and new scalar-bit test. Cargo build-script/default/full decoder/workspace qualification excluded. Dependency binaries pinned; not a claim they were rebuilt from this snapshot.')

def save():
    temporary=O.with_suffix('.tmp');temporary.write_text(json.dumps(state,indent=2)+'\n');temporary.replace(O)

save()
with (F/'build.log').open('x') as log:
    child=subprocess.Popen(command,cwd=S,env=env,stdout=log,stderr=subprocess.STDOUT);state['child_pid']=child.pid;save();code=child.wait()
state['build_returncode']=code
if code==0:
    state.update(status='testing',executable=str(F/'gradient-tests'),executable_sha256=sha(F/'gradient-tests'));save()
    with (F/'tests.log').open('x') as log:
        child=subprocess.Popen([str(F/'gradient-tests'),'gradient::','--test-threads','2'],cwd=S,env=env,stdout=log,stderr=subprocess.STDOUT);state['child_pid']=child.pid;save();code=child.wait()
    state['test_returncode']=code
    log=(F/'tests.log').read_text()
    if code==0:
        assert 'unrolled_distance_blocks_preserve_scalar_bits_and_partial_lanes ... ok' in log
        summary=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',log);assert summary
        state.update(passed=int(summary[1]),failed=int(summary[2]),ignored=int(summary[3]))
state['source_unchanged']=all(sha(S/name)==pin for name,pin in m['source_sha256'].items())
logs=[F/'build.log']
if (F/'tests.log').exists():
    logs.append(F/'tests.log')
state['pins']={str(path):sha(path) for path in logs}
state['status']='passed_pending_audit' if state.get('test_returncode')==0 and state['source_unchanged'] else 'build_or_test_failure'
save();print(state['status'])
