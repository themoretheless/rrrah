#!/usr/bin/env python3
"""Qualify sequential backbone against the complete current native unit source."""
import hashlib,json,os,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    audit=D/'dedup-loftr-sequential-backbone-complete-audit-20261012.json';a=json.loads(audit.read_text());assert a['entire_evidence_identical'] and a['rss_delta_vs_original_bytes']<0 and a['rss_delta_vs_instrumented_bytes']<0
    h=T/'rrrah-loftr-sequential-current-units-20261012';out=D/'dedup-loftr-sequential-current-units-20261012.json';assert not h.exists() and not out.exists()
    native=R/'vendor/loftr/src';before={str(p.relative_to(native)):sha(p) for p in native.rglob('*') if p.is_file()};shutil.copytree(native,h/'src')
    old=T/'rrrah-loftr-coarse-cancel-test-harness-20261011'
    for n in ['Cargo.toml','Cargo.lock']:shutil.copy2(old/n,h/n)
    p=h/'src/loftr_model.rs';candidate=T/'rrrah-loftr-sequential-backbone-source-20261012/vendor/loftr/src/loftr_model.rs'
    runtime=json.loads((D/'dedup-loftr-sequential-backbone-runtime-20261012.json').read_text());code=p.read_text();assert code.count(runtime['original_call'])==1;code=code.replace(runtime['original_call'],'self.backbone_features(&image0, &image1, cancel)?')
    for v in [runtime['removed_stacked_branch'],runtime['removed_arguments']]:assert code.count(v)==1;code=code.replace(v,'')
    assert code==candidate.read_text();p.write_text(code)
    test=h/'src/loftr_model/tests.rs';test.write_text(test.read_text()+runtime['unit_control'])
    r=dict(status='building',source=str(h),root_native_before=before,production_delta=['loftr_model.rs'],test_delta=['loftr_model/tests.rs'],pins={str(p):sha(p) for p in h.rglob('*') if p.is_file()},scope='Complete current native unit source plus independent original stacked feature oracle. Does not qualify full current decoder/caller stack, gradients, full corpus/platforms or hard process memory cap.')
    r['pins'][str(audit)]=sha(audit)
    def save():out.write_text(json.dumps(r,indent=2)+'\n')
    save();env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');runtime_dir=T/'rrrah-loftr-native-runtime-20261010';env['PATH']=str(runtime_dir/'bin')+':'+env['PATH'];target=T/'rrrah-dedup-exact-portfolio-20261011';log=T/'rrrah-loftr-sequential-current-build-20261012.log'
    with log.open('w') as f:run=subprocess.run(['cargo','test','--lib','--no-run','--locked','--offline','--manifest-path',str(h/'Cargo.toml'),'--target-dir',str(target)],cwd='/tmp',env=env,stdout=f,stderr=subprocess.STDOUT)
    r['pins'][str(log)]=sha(log);r['build_returncode']=run.returncode;save()
    if run.returncode:r['status']='build_failed';save();return
    exe=Path(re.search(r'Executable unittests .*\(([^)]+)\)',log.read_text())[1]);saved=T/'rrrah-loftr-sequential-current-unit-executable-20261012';assert not saved.exists();shutil.copy2(exe,saved);r['pins'][str(saved)]=sha(saved)
    run=subprocess.run(['/usr/bin/env','DYLD_LIBRARY_PATH='+str(runtime_dir/'lib/python3.12/site-packages/torch/lib'),str(saved),'--test-threads=1'],cwd='/tmp',capture_output=True,text=True);log=T/'rrrah-loftr-sequential-current-units-20261012.log';log.write_text(run.stdout+run.stderr);r['pins'][str(log)]=sha(log);r['returncode']=run.returncode
    result=re.search(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out',run.stdout);r['passed']=int(result[1]) if result else None
    assert all(sha(native/n)==v for n,v in before.items());assert all(sha(p)==v for p,v in r['pins'].items());r['status']='current_native_units_passed_not_adopted' if run.returncode==0 and result else 'unit_failed';save();print(r['status'],r['passed'])
if __name__=='__main__':main()
