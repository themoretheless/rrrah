"""Audit the completed alternating native-process measurement."""
import hashlib
import json
import math
import statistics
from pathlib import Path

D = Path('docs/research')
path = D / 'dedup-anchor-geometry-alternating.json'
r = json.loads(path.read_text())
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert r['status'] == 'complete'
assert r['required_runs'] == len(r['results']) == 12
for p, digest in r['pins'].items():
    assert sha(p) == digest, p
for index, row in enumerate(r['results']):
    round_index, offset = divmod(index, 2)
    order = ['legacy', 'geometry_only'] if round_index % 2 == 0 else ['geometry_only', 'legacy']
    assert row['round'] == round_index and row['mode'] == order[offset]
    assert row['exact_evidence_equal'] is True
    assert math.isfinite(row['seconds']) and row['seconds'] > 0
medians = {mode: statistics.median(x['seconds'] for x in r['results'] if x['mode'] == mode)
           for mode in ('legacy', 'geometry_only')}
assert medians == r['median_seconds']
assert r['observed_ratio'] == medians['legacy'] / medians['geometry_only']
audit = {'status': 'verified_twelve_alternating_runs', 'report_sha256': sha(path),
         'median_seconds': medians, 'observed_ratio': r['observed_ratio'], 'scope': r['scope']}
(D / 'dedup-anchor-geometry-alternating-audit.json').write_text(json.dumps(audit, indent=2) + '\n')
print(json.dumps(audit, indent=2))
