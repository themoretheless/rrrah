#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,re,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
paths=[D/'dedup-duration-default-gate-20261014.json',D/'dedup-duration-no-features-gate-20261014.json'];records=[]
F=D/'dedup-duration-decode-free-gates-20261014';F.mkdir(exist_ok=False);retained={}
for mode,P,expected in zip(['raster_without_decode','no_optional_features'],paths,[(61,0,3),(44,0,2)]):
    d=json.loads(P.read_text());assert d['status']=='passed_pending_audit'and d['build_returncode']==d['test_returncode']==0
    assert (d['passed'],d['failed'],d['ignored'])==expected
    assert all(sha(p)==h for p,h in d['pins'].items())and all(sha(p)==h for p,h in d['dependency_pins'].items())and all(sha(R/p)==h for p,h in d['source_sha256'].items())
    assert 'feature="decode"'not in d['command']
    if mode=='no_optional_features':assert '--cfg'not in d['command']
    else:assert d['command'][d['command'].index('--cfg')+1]=='feature="raster"'
    folder=Path('/private/tmp/rrrah-duration-default-gate-20261014'if mode=='raster_without_decode'else'/private/tmp/rrrah-duration-no-features-gate-20261014')
    listed=re.findall(r'^(.+): test$',(folder/'inventory.log').read_text(),re.M);executed=re.findall(r'^test (.+) \.\.\. (ok|ignored(?:,.*)?)$',(folder/'tests.log').read_text(),re.M)
    assert sorted(listed)==sorted(n for n,_ in executed)and len(listed)==len(set(listed))==expected[0]+expected[2]
    for name in ['large_shared_denominator_reduces_before_overflow','sum_retains_exact_time_and_refuses_unrepresentable_result']:assert ('sequence::duration_sum_tests::'+name,'ok')in executed
    target=F/mode;target.mkdir()
    for p in [folder/'build.log',folder/'inventory.log',folder/'tests.log']:
        copy=target/p.name;shutil.copyfile(p,copy);assert sha(p)==sha(copy);retained[str(copy.relative_to(R))]=sha(copy)
    records.append(dict(mode=mode,report=str(P.relative_to(R)),report_sha256=sha(P),passed=expected[0],failed=0,manual_ignored=expected[2],exact_inventory=True))
manifest_path=D/'dedup-gradient-unroll4-v7-source-20261014.json';m=json.loads(manifest_path.read_text());before=Path(m['source'])/'crates/rrrah-dedup/src/sequence.rs';after=R/'crates/rrrah-dedup/src/sequence.rs';old=before.read_text();new=after.read_text()
assert new.replace('#[cfg(any(test, feature = "decode"))]\n    pub(crate) fn checked_add','#[cfg(feature = "decode")]\n    pub(crate) fn checked_add').replace('#[cfg(test)]\nmod duration_sum_tests','#[cfg(all(test, feature = "decode"))]\nmod duration_sum_tests')==old
O=D/'dedup-duration-decode-free-gates-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_current_root_decode_free_unit_configurations',rows=records,source=str(after.relative_to(R)),source_sha256=sha(after),baseline_manifest=str(manifest_path.relative_to(R)),baseline_manifest_sha256=sha(manifest_path),auditor_sha256=sha(__file__),retained_artifacts=retained,production_implementation_body_unchanged=True,cfg_change='checked_add compiled in tests or decode feature; duration regression module compiled in all test configurations. Non-test nondecode and decode production contents unchanged.',scope='Actual current root dedup unit sources, nofeature44pass2manualignored and raster61pass3manualignored, directrustc with pinned real cached dependency binaries. Existing duration arithmetic overflow regression now exercised without decoder. Real animation timeline/decode/Cargo/fullworkspace/native/defaultcollection/platform/currentcorpus qualification remains separate.'),indent=2)+'\n');print(records)
