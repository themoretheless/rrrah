"""Audit bounded prepared-input reuse; require both platform gates and native parity."""
import hashlib,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]; D=ROOT/'docs/research'
OUT=D/'dedup-prepared-reuse-audit-20261011.json'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert not OUT.exists()
manifest_path=D/'dedup-prepared-reuse-snapshot-20261011.json'
manifest=json.loads(manifest_path.read_text())
assert len(manifest['file_hashes'])==3055
assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
assert all(sha(ROOT/p)==h for p,h in manifest['file_hashes'].items() if p.startswith('crates/rrrah-dedup/'))
logs=[D/'dedup-prepared-reuse-tests-20261011.log',D/'dedup-prepared-reuse-linux-20261011.log']
for log in logs:
    counts=[int(v) for v in re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',log.read_text())]
    assert counts==[56,11,6,5,9,10,6,8],(log,counts)
rpath=D/'dedup-prepared-reuse-native-20261011.json'; r=json.loads(rpath.read_text())
assert r['status']=='complete_prepared_reuse_parity_and_two_lifecycle_controls'
assert all(sha(p)==h for p,h in r['pins'].items())
assert len(r['results'])==3
assert [len(x['evidence']['attempts']) for x in r['results']]==[11,25,1]
assert all(x['status']=='complete' and x['exact_all_attempts_parity'] and x['evidence']['managed_used']==0 for x in r['results'])
assert [x['action'] for x in r['controls']]==['cancel','mutate']
for x in r['controls']:
    assert x['status']=='complete' and x['evidence']['status']=='refused' and x['evidence']['managed_used']==0
    assert ('Cancelled' if x['action']=='cancel' else 'Changed') in x['evidence']['error']
    assert sha(x['weights'])==x['after_sha256']
files=[manifest_path,rpath,*logs,D/'dedup-prepared-reuse-native-build-20261011.log',Path(__file__)]
a={'status':'complete_bounded_prepared_input_reuse_qualification','snapshot_files':3055,'macos_generic_tests':111,'linux_decode_generic_tests':111,'native_parity_attempts':37,'native_lifecycle_controls':2,'pins':{str(p):sha(p) for p in files},'scope':'At most ten lazy managed prepared inputs per operation; all 25 Cartesian model comparisons remain. Original source/dependency and fixed weight snapshots, sticky cancellation, and original-file pixel confirmation retained. No timing speedup claim, hard tensor/workspace RSS cap, native Linux Torch qualification, full corpus coverage, or collection scaling claim.','maximum_native_process_rss_bytes':max(x['process_peak_rss_bytes'] for x in r['results'])}
OUT.write_text(json.dumps(a,indent=2)+'\n');print(a['status'])
