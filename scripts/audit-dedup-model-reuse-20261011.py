"""Verify completed operation-scoped model reuse qualification without broadening its scope."""
import hashlib,json,re,statistics
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';OUT=D/'dedup-model-reuse-audit-20261011.json';assert not OUT.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest=json.loads((D/'dedup-model-reuse-fixed-snapshot-20261011.json').read_text())
assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
assert all(sha(ROOT/p)==h for p,h in manifest['file_hashes'].items() if p.startswith('crates/rrrah-dedup/'))
r=json.loads((D/'dedup-model-reuse-native-20261011.json').read_text());assert r['status']=='complete_reuse_parity_and_two_lifecycle_controls'
assert all(sha(p)==h for p,h in r['pins'].items())
assert len(r['results'])==3 and all(x['status']=='complete' and x['exact_all_attempts_parity'] for x in r['results'])
assert sum(len(x['evidence']['attempts']) for x in r['results'])==37
assert [x['action'] for x in r['controls']]==['cancel','mutate']
for x in r['controls']:
    assert x['status']=='complete' and x['evidence']['status']=='refused' and x['evidence']['managed_used']==0
    assert ('Cancelled' if x['action']=='cancel' else 'Changed') in x['evidence']['error']
    assert sha(x['weights'])==x['after_sha256']
t=json.loads((D/'dedup-model-reuse-timing-20261011.json').read_text());assert t['status']=='complete_four_alternating_timing_controls'
assert all(sha(p)==h for p,h in t['pins'].items())
assert [x['mode'] for x in t['results']]==['fresh','reuse','reuse','fresh']
old_manifest=json.loads((D/'dedup-model-tiles-snapshot-20261011.json').read_text())
cli='scripts/dedup-loftr-native-probe/src/bin/tile_model_files.rs'
assert old_manifest['file_hashes'][cli]==manifest['file_hashes'][cli]
assert all(x['status']=='complete' and x['exact_all_25_attempts_parity'] for x in t['results'])
unit=D/'dedup-model-reuse-loftr-unit-tests-20261011.log';assert re.search(r'test result: ok\. 60 passed; 0 failed',unit.read_text())
medians={mode:statistics.median(x['elapsed_seconds'] for x in t['results'] if x['mode']==mode) for mode in ['fresh','reuse']}
cpu={mode:[] for mode in ['fresh','reuse']}
for x in t['results']:
    m=re.search(r'([0-9.]+) real\s+([0-9.]+) user\s+([0-9.]+) sys',x['stderr']);assert m
    cpu[x['mode']].append(float(m[2])+float(m[3]))
cpu_medians={mode:statistics.median(values) for mode,values in cpu.items()}
files=[D/'dedup-model-reuse-fixed-snapshot-20261011.json',D/'dedup-model-reuse-native-20261011.json',D/'dedup-model-reuse-timing-20261011.json',unit,D/'dedup-model-reuse-loftr-unit-build-20261011.log',D/'dedup-model-reuse-fixed-native-build-20261011.log',D/'dedup-model-reuse-timing-build-20261011.log',Path(__file__),Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/model-reuse-loftr-unit-qualified-20261011')]
a={'status':'complete_bounded_operation_scoped_reuse_qualification','snapshot_files':len(manifest['file_hashes']),'macos_loftr_unit_tests':60,'native_parity_attempts':37,'native_lifecycle_controls':2,'alternating_counter_free_timing_controls':4,'timing_median_seconds':medians,'cpu_time_median_seconds':cpu_medians,'observed_fresh_to_reuse_ratio':medians['fresh']/medians['reuse'],'timing_scope':'One fixed foreign pair, two runs per mode, reversed order on a shared host. No controlled general latency or collection throughput claim. Diagnostic callback-counting parity runs are excluded from timing comparison.','maximum_timing_process_rss_bytes':max(x['process_peak_rss_bytes'] for x in t['results']),'pins':{str(p):sha(p) for p in files},'scope':'Private lazy model slot scoped to one public tiles/portfolio/collection call. Each call starts empty, fixed weights guarded by existing per-attempt and final snapshots; no global cache. Real direct tiles retain every prior attempt proof. Portfolio/collection cached-provider runtime still needs real qualification. No hard tensor/workspace/RSS admission or all-case completion.','remaining':['Retained positional-table bounds under changing extreme aspect ratios need qualification; current640 controls fit default256-square coarse table','Real reused-model tiled portfolio/collection measurement','Repeated decoding/preparation and complete current corpus recall/negatives','Indexed scaling rather than quadratic pair enumeration','Total tensor/workspace/RSS admission and kernel preemption','Alpha/HDR, full100format and cross-platform model qualification'],'initial_failed_build':'docs/research/dedup-model-reuse-native-build-20261011.log; syntax-only insertion error, preserved superseded snapshot; fixed source qualified separately'}
OUT.write_text(json.dumps(a,indent=2)+'\n');print(a['status'],medians)
