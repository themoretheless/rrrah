"""Audit the completed bounded automatic tile-search milestone; no broad-corpus claim."""
import hashlib,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
OUT=D/'dedup-model-tiles-audit-20261011.json';assert not OUT.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest=json.loads((D/'dedup-model-tiles-snapshot-20261011.json').read_text())
assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
assert all(sha(ROOT/p)==h for p,h in manifest['file_hashes'].items() if p.startswith('crates/rrrah-dedup/'))
r=json.loads((D/'dedup-model-tiles-native-20261011.json').read_text());assert r['status']=='complete_three_automatic_searches'
assert all(sha(p)==h for p,h in r['pins'].items())
counts={}
expected={'dedup-model-tiles-integration-20261011.log':[56,11,7,10,8], 'dedup-model-tiles-adjacent-tests-20261011.log':[6,5,6], 'dedup-model-tiles-linux-20261011.log':[56,11,6,5,7,10,6,8]}
for name,want in expected.items():
    text=(D/name).read_text();got=list(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',text)))
    assert got==want,(name,got,want);counts[name]=got
for row,n,yes in zip(r['results'],[11,25,1],[True,False,True]):
    assert row['status']=='complete';e=row['evidence'];assert len(e['attempts'])==n and e['supported']==yes
    assert all(not a['supported'] for a in e['attempts'][:-1])
    assert e['attempts'][-1]['supported']==yes and e['managed_used']==0
    if yes:assert row['exact_prior_terminal_proof_parity']
files=[D/n for n in expected]+[D/'dedup-model-tiles-native-20261011.json',D/'dedup-model-tiles-snapshot-20261011.json',D/'dedup-model-tiles-native-build-20261011.log',Path(__file__)]
audit={'status':'complete_bounded_automatic_tiles_qualification','snapshot_files':len(manifest['file_hashes']),'tests':counts,'macos_generic_tests':109,'linux_arm64_decode_tests':109,'native_searches':3,'native_attempts':37,'cases':[{'case':x['case'],'supported':x['evidence']['supported'],'attempts':len(x['evidence']['attempts']),'elapsed_seconds':x['elapsed_seconds'],'process_peak_rss_bytes':x['process_peak_rss_bytes']} for x in r['results']], 'pins':{str(p):sha(p) for p in files},'scope':'Production automatic full/quadrant model API, optional pair/collection integration tested with supplied providers. Real CPU native model API recovers crumpled local support without manual ROI, rejects one foreign pair through all25 attempts, preserves whole rephoto proof. No whole-file identity, transitive grouping or broad corpus precision claim.', 'remaining':['Broad original corpus recall and negatives with this revision','Real tiled portfolio/collection corpus measurement; integration currently tested with supplied providers','Repeated decode/model load and quadratic collection cost','Hard total tensor/workspace/RSS admission and kernel preemption','Alpha/HDR, full100formats, model platforms beyond macOS CPU']}
OUT.write_text(json.dumps(audit,indent=2)+'\n');print(audit['status'])
