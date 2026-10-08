"""Audit terminal indexed/fresh comparison evidence and frozen provenance."""
import hashlib, json, math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions

D = Path('docs/research')
p = D / 'dedup-anchor-rank-driven-real-collection.json'
out = D / 'dedup-anchor-rank-driven-real-collection-audit.json'
assert not out.exists()
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
digest = sha(p)
r = json.loads(p.read_text())
assert r['status'] == 'complete_real_three_file_indexed_parity'
assert r['returncode'] == 0 and not r['stderr']
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
inputs = [T / 'original-resolution-negative-200201/201300.jpg',
          T / 'original-resolution-strong-all/201303.jpg',
          T / 'original-resolution-negative-200201/201400.jpg']
manifest = json.loads((T / 'prepared.json').read_text())
records = {x['filename']: x for group in manifest['images'].values() for x in group}
dimensions = [list(oriented_dimensions(path, records[path.name]['source_size'])) for path in inputs]
assert dimensions == r['dimensions']
assert r['source_tolerance'] == 2 * math.hypot(*dimensions[0]) / math.hypot(*dimensions[1])
assert r['expected_positive'] == [1, 2]
assert all(str(path) in r['pins'] for path in inputs)
for path, pin in r['pins'].items():
    assert sha(path) == pin
snapshot = Path(r['source_snapshot'])
for path, pin in r['source_hashes'].items():
    assert sha(snapshot / path) == pin
rows = [json.loads(line) for line in r['stdout'].splitlines()]
assert rows == r['results']
assert [(x['left'], x['right']) for x in rows[:-1]] == [(1, 2), (1, 3), (2, 3)]
assert [x['supported'] for x in rows[:-1]] == [True, False, False]
assert all(x['direct_parity'] for x in rows[:-1]) and rows[0]['indexed']
assert rows[-1] == dict(status='complete_three_real_file_parity', memory_used=0)
assert sha(p) == digest
out.write_text(json.dumps(dict(status='verified_real_three_file_indexed_parity',
    supported_pairs=1, rejected_direct_pairs=2,
    pins={str(p): digest, str(Path(__file__)): sha(__file__)},
    scope='Terminal native assertions compare complete original point sets, anchor evidence, geometry and selected recipe metadata for all retrieved pairs; omitted pairs are direct negatives. Frozen report/binary/dedup/input provenance. Three files and one explicit tolerance, not full corpus collection recall/precision or independent pixel recomputation.'), indent=2) + '\n')
print('verified real three-file indexed/direct parity')
