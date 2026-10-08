"""Audit full real evidence equality and frozen inputs/code."""
import hashlib, json
from pathlib import Path
D = Path('docs/research')
p = D / 'dedup-borrowed-gradient-real-parity.json'
out = D / 'dedup-borrowed-gradient-real-parity-audit.json'
assert not out.exists()
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
digest = sha(p)
r = json.loads(p.read_text())
assert r['status'] == 'complete_borrowed_gradient_real_parity'
assert len(r['results']) == r['required_pairs'] == 3
for path, pin in r['pins'].items():
    assert sha(path) == pin
snapshot = Path(r['source_snapshot'])
for path, pin in r['source_hashes'].items():
    assert sha(snapshot / path) == pin
baseline = D / 'dedup-anchor-rank-driven-fallback-real.json'
old = json.loads(baseline.read_text())
assert old['status'] == 'complete' and len(old['results']) == 3
for row, prior in zip(r['results'], old['results']):
    for key in ['original', 'query', 'source_tolerance']:
        assert row[key] == prior[key]
    assert row['returncode'] == 0 and row['exact_parity']
    assert row['evidence'] == json.loads(row['stdout']) == prior['evidence']
    assert row['evidence']['supported']
assert sha(p) == digest
out.write_text(json.dumps(dict(status='verified_complete_borrowed_gradient_real_parity',
    pairs=3, pins={str(p): digest, str(Path(__file__)): sha(__file__)},
    scope='Exact complete selected attempt/radii, original points, geometry, native anchor counts and support versus frozen three-pair prior. Provenance checked. No timing speedup, fresh independent pixel recomputation, full corpus or collection precision/recall.'), indent=2) + '\n')
print('verified three complete native evidence matches')
