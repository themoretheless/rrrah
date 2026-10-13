#!/usr/bin/env python3
"""Validate an isolated sequential backbone on the fixed complete negative case."""
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    mp=D/'dedup-disc21-heldout-negative-source-20261012.json';m=json.loads(mp.read_text());base=Path(m['snapshot'])
    control=D/'dedup-loftr-sequential-backbone-control-20261012.json';c=json.loads(control.read_text());assert c['status']=='exact_feature_control_passed_not_adopted' and all(sha(p)==h for p,h in c['pins'].items())
    profile=json.loads((D/'dedup-loftr-negative-rss-profile-20261012.json').read_text());assert profile['status']=='complete_identical_evidence_stage_rss_diagnostic','Wait for original diagnostic completion; do not run simultaneously.'
    source=T/'rrrah-loftr-sequential-backbone-source-20261012';h=T/'rrrah-loftr-sequential-backbone-harness-20261012';units=T/'rrrah-loftr-sequential-backbone-unit-harness-20261012';out=D/'dedup-loftr-sequential-backbone-runtime-20261012.json'
    assert not any(p.exists() for p in [source,h,units,out])
    for n,v in m['file_hashes'].items():
        assert sha(base/n)==v;q=source/n;q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(base/n,q)
    p=source/'vendor/loftr/src/loftr_model.rs';code=p.read_text();assert sha(p)==sha(R/'vendor/loftr/src/loftr_model.rs')
    assert sha(source/'vendor/loftr/src/backbone.rs')==sha(R/'vendor/loftr/src/backbone.rs')
    old_call='self.backbone_features(&image0, &image1, batch_size, hw0_i, hw1_i, cancel)?'
    assert code.count(old_call)==1;code=code.replace(old_call,'self.backbone_features(&image0, &image1, cancel)?')
    a=code.index('    fn backbone_features(');b=code.index('\n    fn coarse_features(',a);body=code[a:b]
    start=body.index('        if hw0_i == hw1_i {');end=body.index('        let (coarse0, fine0) =',start);removed_branch=body[start:end];body=body[:start]+body[end:]
    removed_args='        batch_size: i64,\n        hw0_i: (i64, i64),\n        hw1_i: (i64, i64),\n';assert body.count(removed_args)==1;body=body.replace(removed_args,'');code=code[:a]+body+code[b:];p.write_text(code)
    hashes={n:sha(source/n) for n in m['file_hashes']};assert [n for n in hashes if hashes[n]!=m['file_hashes'][n]]==['vendor/loftr/src/loftr_model.rs']
    old=T/'rrrah-disc21-heldout-negative-harness-20261012';h.mkdir();(h/'Cargo.toml').write_text((old/'Cargo.toml').read_text().replace(str(base),str(source)));shutil.copy2(old/'Cargo.lock',h/'Cargo.lock');probe=h/'src/bin/disc21_heldout_negative_collection.rs';probe.parent.mkdir(parents=True);shutil.copy2(old/'src/bin/disc21_heldout_negative_collection.rs',probe)
    shutil.copytree(source/'vendor/loftr/src',units/'src');old_units=T/'rrrah-loftr-coarse-cancel-test-harness-20261011'
    for n in ['Cargo.toml','Cargo.lock']:shutil.copy2(old_units/n,units/n)
    text=(D/'dedup-loftr-sequential-backbone-controls-20261012.rs').read_text();begin=text.index('        let stacked = ');end=text.index('        let (coarse0, fine0)',begin)
    oracle='''        let joined = Tensor::cat(&[left.shallow_clone(), right.shallow_clone()], 0);
        let (oracle_coarse, oracle_fine) = model.backbone.forward_cancellable(&joined, false, &|| false)?;
        let oc = oracle_coarse.split(batch, 0); let of = oracle_fine.split(batch, 0);
        let stacked = BackboneFeatures { coarse0: oc[0].shallow_clone(), coarse1: oc[1].shallow_clone(), fine0: of[0].shallow_clone(), fine1: of[1].shallow_clone() };
'''
    text=text[:begin]+oracle+text[end:]
    seq_start=text.index('        let (coarse0, fine0)');seq_end=text.index('        for (label',seq_start)
    text=text[:seq_start]+'''        let sequential = model.backbone_features(&left, &right, &|| false)?;
        let BackboneFeatures {coarse0, coarse1, fine0, fine1} = sequential;
'''+text[seq_end:]
    tests=units/'src/loftr_model/tests.rs';tests.write_text(tests.read_text()+text)
    pins={str(p):sha(p) for p in [mp,control,Path(__file__),probe,h/'Cargo.toml',h/'Cargo.lock']};pins.update({str(source/n):v for n,v in hashes.items()});pins.update({str(p):sha(p) for p in units.rglob('*') if p.is_file()})
    r={'status':'building_units','producer_pid':os.getpid(),'pins':pins,'source':str(source),'source_hashes':hashes,'removed_stacked_branch':removed_branch,'removed_arguments':removed_args,'original_call':old_call,'unit_control':text,'scope':'Isolated sequential backbone candidate; one production method/call change over frozen negative source. Full native units and exact stacked feature oracle, then unchanged complete Q00992/R043984 caller policy/evidence/RSS. Not root adoption, current entire-stack qualification, full corpus or hard process-memory bound.'}
    def save():q=out.with_suffix('.tmp');q.write_text(json.dumps(r,indent=2)+'\n');q.replace(out)
    save();runtime=T/'rrrah-loftr-native-runtime-20261010';env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');env['PATH']=str(runtime/'bin')+':'+env['PATH'];target=T/'rrrah-dedup-exact-portfolio-20261011'
    def build(cmd,log):
        with log.open('w') as stream:
            child=subprocess.Popen(cmd,cwd='/tmp',env=env,stdout=stream,stderr=subprocess.STDOUT);r['build_pid']=child.pid;save();rc=child.wait()
        pins[str(log)]=sha(log);r['build_returncode']=rc;save();return rc
    log=T/'rrrah-loftr-sequential-backbone-units-build-20261012.log';cmd=['cargo','test','--lib','--no-run','--locked','--offline','--manifest-path',str(units/'Cargo.toml'),'--target-dir',str(target)]
    if build(cmd,log):r['status']='unit_build_failed';save();return
    exe=Path(re.search(r'Executable unittests .*\(([^)]+)\)',log.read_text())[1]);saved=T/'rrrah-loftr-sequential-backbone-unit-executable-20261012';assert not saved.exists();shutil.copy2(exe,saved);pins[str(saved)]=sha(saved)
    cmd=['/usr/bin/env','DYLD_LIBRARY_PATH='+str(runtime/'lib/python3.12/site-packages/torch/lib'),str(saved),'--test-threads=1'];run=subprocess.run(cmd,cwd='/tmp',capture_output=True,text=True);log=T/'rrrah-loftr-sequential-backbone-units-20261012.log';log.write_text(run.stdout+run.stderr);pins[str(log)]=sha(log);r.update(unit_returncode=run.returncode,unit_log=str(log));save()
    if run.returncode:r['status']='unit_or_exact_feature_failure';save();return
    r['status']='building_complete_probe';save();log=T/'rrrah-loftr-sequential-backbone-probe-build-20261012.log';cmd=['cargo','build','--manifest-path',str(h/'Cargo.toml'),'--bin','disc21_heldout_negative_collection','--locked','--offline','--target-dir',str(target)]
    if build(cmd,log):r['status']='probe_build_failed';save();return
    exe=T/'rrrah-loftr-sequential-backbone-executable-20261012';assert not exe.exists();shutil.copy2(target/'debug/disc21_heldout_negative_collection',exe);pins[str(exe)]=sha(exe)
    baseline_path=D/'dedup-disc21-heldout-negatives-runtime-20261012.json';baseline=json.loads(baseline_path.read_text())['results'][7];pins[str(baseline_path)]=sha(baseline_path);cmd=baseline['command'].copy();cmd[4]=str(exe);stdout=T/'rrrah-loftr-sequential-backbone-20261012.stdout';stderr=T/'rrrah-loftr-sequential-backbone-20261012.stderr'
    with stdout.open('w') as o,stderr.open('w') as e:
        start=time.monotonic();child=subprocess.Popen(cmd,cwd='/tmp',stdout=o,stderr=e);r.update(status='running_complete_probe',child_pid=child.pid,command=cmd,stdout=str(stdout),stderr=str(stderr));save();rc=child.wait()
    pins.update({str(p):sha(p) for p in [stdout,stderr]});r.update(returncode=rc,elapsed_seconds=time.monotonic()-start)
    if rc:r['status']='complete_probe_failed';save();return
    evidence=json.loads(stdout.read_text());same=evidence==baseline['evidence'];r.update(evidence=evidence,evidence_identical=same,rss_bytes=int(re.search(r'(\d+)\s+maximum resident set size',stderr.read_text())[1]),status='complete_identical_evidence_candidate_not_adopted' if same else 'complete_evidence_changed_not_adopted')
    assert all(sha(p)==v for p,v in pins.items());save();print(r['status'],r['rss_bytes'],flush=True)
if __name__=='__main__':main()
