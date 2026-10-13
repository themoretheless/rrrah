#!/usr/bin/env python3
"""Check sequential versus stacked backbone before changing production."""
import hashlib
import json
import os
import re
import shutil
import subprocess
from pathlib import Path

R=Path(__file__).resolve().parents[1]; D=R/'docs/research'; T=Path('/Users/themoretheless/.codex/tmp')
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def main():
    h=T/'rrrah-loftr-sequential-backbone-control-20261012'
    out=D/'dedup-loftr-sequential-backbone-control-20261012.json'
    assert not h.exists() and not out.exists()
    shutil.copytree(R/'vendor/loftr/src',h/'src')
    original={str(p.relative_to(h)):sha(p) for p in h.rglob('*.rs')}
    assert all(sha(R/'vendor/loftr'/n)==v for n,v in original.items())
    old=T/'rrrah-loftr-coarse-cancel-test-harness-20261011'
    for n in ['Cargo.toml','Cargo.lock']: shutil.copy2(old/n,h/n)
    controls=D/'dedup-loftr-sequential-backbone-controls-20261012.rs'
    tests=h/'src/loftr_model/tests.rs'; tests.write_text(tests.read_text()+controls.read_text())
    pins={str(p):sha(p) for p in [controls,Path(__file__),*[p for p in h.rglob('*') if p.is_file()]]}
    r={'status':'building','producer_pid':os.getpid(),'pins':pins,'source':str(h),'original_file_hashes':original,
       'scope':'Unchanged current root native production, authored inference-only random-weight four-shape/batch control. Exact feature tensor equality required before sequential candidate adoption. No real-weight full inference/RSS/corpus/gradient/platform qualification.'}
    def save():
        p=out.with_suffix('.tmp');p.write_text(json.dumps(r,indent=2)+'\n');p.replace(out)
    save();runtime=T/'rrrah-loftr-native-runtime-20261010';env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');env['PATH']=str(runtime/'bin')+':'+env['PATH']
    log=T/'rrrah-loftr-sequential-backbone-build-20261012.jsonl';err=log.with_suffix('.stderr')
    cmd=['cargo','test','--no-run','--message-format=json','--lib','--locked','--offline','--manifest-path',str(h/'Cargo.toml'),'--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')]
    with log.open('w') as o,err.open('w') as e:
        child=subprocess.Popen(cmd,cwd='/tmp',env=env,stdout=o,stderr=e);r['build_pid']=child.pid;save();rc=child.wait()
    r['build_returncode']=rc;pins.update({str(p):sha(p) for p in [log,err]});save()
    if rc: r['status']='build_failed';save();return
    rows=[json.loads(x) for x in log.read_text().splitlines() if x.startswith('{')]
    paths=[x['executable'] for x in rows if x.get('reason')=='compiler-artifact' and x.get('executable') and x['profile']['test']];assert len(paths)==1
    exe=T/'rrrah-loftr-sequential-backbone-control-executable-20261012';assert not exe.exists();shutil.copy2(paths[0],exe);pins[str(exe)]=sha(exe)
    cmd=['/usr/bin/env','DYLD_LIBRARY_PATH='+str(runtime/'lib/python3.12/site-packages/torch/lib'),str(exe),'--exact','loftr_model::tests::sequential_backbone_matches_stacked_features_exactly','--nocapture','--test-threads=1']
    r.update(status='running_control',command=cmd);save();run=subprocess.run(cmd,cwd='/tmp',capture_output=True,text=True)
    log=T/'rrrah-loftr-sequential-backbone-control-20261012.log';log.write_text(run.stdout+run.stderr);pins[str(log)]=sha(log)
    r.update(returncode=run.returncode,log=str(log),status='exact_feature_control_passed_not_adopted' if run.returncode==0 else 'exact_feature_control_failed_not_adopted')
    assert all(sha(p)==v for p,v in pins.items());save();print(r['status'])

if __name__=='__main__': main()
