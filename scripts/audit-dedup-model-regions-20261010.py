"""Audit the explicit-region milestone without claiming automatic/all-case search."""
import hashlib
import json
from pathlib import Path
import re
import shutil

ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-model-region-audit-20261010.json'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert not OUT.exists()
snapshot_path=D/'dedup-model-region-snapshot-20261010.json'
snapshot=json.loads(snapshot_path.read_text())
assert len(snapshot['file_hashes'])==3050
assert all(sha(Path(snapshot['snapshot'])/p)==h for p,h in snapshot['file_hashes'].items())
measurement_path=D/'dedup-loftr-regions-frozen-20261010.json'
r=json.loads(measurement_path.read_text())
assert r['status']=='complete_twenty_four_native_region_comparisons' and len(r['results'])==24
assert all(sha(p)==h for p,h in r['pins'].items())
runtime_path=D/'dedup-loftr-native-20261010-runtime-pins.json'
assert all(sha(p)==h for p,h in json.loads(runtime_path.read_text())['pins'].items())
for row in r['results']:
    e=row['evidence']
    assert row['status']=='complete' and row['returncode']==0
    assert e['status']=='ok' and e['supported']==row['expected_supported']
    assert e['managed_used']==0 and e['managed_peak']<=512*1024**2
    assert row['process_peak_rss_bytes']>0
assert [row['evidence']['geometry_inliers'] for row in r['results'][:2]]==[12,19]
assert all(row['exact_initial_evidence_parity'] for row in r['results'][:2])
assert r['results'][3]['exact_full_evidence_parity']
foreign=[row for row in r['results'] if row['case'].startswith('foreign-')]
assert len(foreign)==20 and all(not row['evidence']['supported'] for row in foreign)
assert len({(tuple(row['left_rectangle']),tuple(row['right_rectangle'])) for row in foreign})==20


def counts(path):
    text=path.read_text()
    assert 'test result: FAILED' not in text and 'error:' not in text
    return [int(n) for n in re.findall(r'test result: ok\. (\d+) passed; 0 failed;',text)]


integration=D/'dedup-model-region-tests-retry-20261010.log'
unit=D/'dedup-model-region-loftr-unit-tests-20261010.log'
linux=D/'dedup-model-region-linux-tests-20261010.log'
minimal=D/'dedup-model-region-no-default-tests-20261010.log'
assert counts(integration)==[10,6,5,8,6,5]
assert counts(unit)==[58]
assert counts(linux)==[54,10,6,5,8,6,5]
assert counts(minimal)==[4]
# Preserve the actual successful host executables before later work rebuilds them.
binaries={}
for kind,log in [('decode',integration),('minimal',minimal)]:
    for name,relative in re.findall(r'Running tests/([^ ]+)\.rs \(([^)]+)\)',log.read_text()):
        source=ROOT/relative
        destination=T/f'model-region-{kind}-{name}-qualified-20261010'
        assert not destination.exists()
        shutil.copy2(source,destination);binaries[str(destination)]=sha(destination)
build=D/'dedup-model-region-loftr-unit-build-20261010.log'
source=ROOT/re.search(r'Executable unittests .*? \(([^)]+)\)',build.read_text())[1]
destination=T/'model-region-loftr-unit-qualified-20261010'
assert not destination.exists()
shutil.copy2(source,destination);binaries[str(destination)]=sha(destination)
logs=[integration,unit,linux,minimal,build,D/'dedup-model-region-frozen-native-build-20261010.log',D/'dedup-model-region-tests-20261010.log',D/'dedup-loftr-regions-frozen-20261010.log']
report={'status':'complete_explicit_region_api_qualification','scope':__doc__,
    'source_snapshot':str(snapshot_path),'source_files':3050,
    'mac_loftr_unit_tests':58,'mac_decode_integration_tests':40,'mac_no_default_proposal_tests':4,
    'linux_decode_unit_tests':54,'linux_decode_integration_tests':40,'linux_libtorch_runtime_qualified':False,
    'native_cpu_comparisons':24,'native_foreign_crop_controls':20,'crumpled_original_pixel_witnesses':[12,19],
    'full_rephoto_exact_evidence_parity':True,'maximum_process_peak_rss_bytes':max(row['process_peak_rss_bytes'] for row in r['results']),
    'explicit_region_api_integrated':True,'automatic_tiled_portfolio_integrated':False,'automatic_tiled_collection_integrated':False,
    'initial_test_compile_failure':'Rust test macro literals .5 were corrected to 0.5 before the frozen snapshot; failed log preserved.',
    'remaining':['Automatic bounded crop enumeration in native/model portfolio and collection',
                 'Full real corpus and broad captured-texture negative precision; these are local visual-support witnesses, not whole-image identity',
                 'Hard tensor/workspace/RSS admission, single-kernel preemption/latency',
                 'Alpha/HDR model input, broad format/platform qualification; Linux/Windows LoFTR runtime unqualified'],
    'pins':{str(p):sha(p) for p in [Path(__file__),snapshot_path,measurement_path,runtime_path,*logs,D/'dedup-loftr-regions-initial-20261010.json',D/'dedup-loftr-both-tiles-audit-20261010.json',D/'dedup-roma-crumpled-audit-20261010.json']},
    'qualified_mac_binaries':binaries}
OUT.write_text(json.dumps(report,indent=2)+'\n')
print('verified explicit-region API: macOS 58 + 40 + 4 checks; Linux 54 + 40; native 24 comparisons')
