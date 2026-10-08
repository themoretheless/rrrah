"""Audit frozen fresh explicit asymmetric parity with historical fallback."""
import argparse
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('report', type=Path)
p.add_argument('output', type=Path)
p.add_argument('--checkpoint', action='store_true')
a = p.parse_args()
assert not a.output.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
digest = sha(a.report)
r = json.loads(a.report.read_text())
assert r['status'] == 'complete' or a.checkpoint and r['status'] == 'running'
assert r['required_pairs'] == 3 and 0 < len(r['results']) <= 3
if not a.checkpoint:
    assert len(r['results']) == 3
for name, pin in r['pins'].items():
    assert sha(name) == pin, name
D = Path('docs/research')
baseline = json.loads((D / 'dedup-anchor-fallback-recovered-real.json').read_text())
for row, prior in zip(r['results'], baseline['results']):
    assert row['returncode'] == 0 and row['seconds'] > 0
    for key in ('original', 'query', 'source_tolerance'):
        assert row[key] == prior[key]
    assert row['smoothing_radii'] == prior['evidence']['smoothing_radii']
    expected = {k: v for k, v in prior['evidence'].items() if k not in ('attempted_recipes', 'smoothing_radii')}
    assert row['evidence'] == expected and row['evidence']['supported']
manifest = json.loads((D / 'dedup-anchor-geometry-smoothing-real-source-128m.json').read_text())
assert sha(manifest['binary']) == manifest['binary_sha256']
for name, pin in manifest['file_hashes'].items():
    assert sha(Path(manifest['snapshot']) / name) == pin, name
assert sha(a.report) == digest
audit = {'status': 'verified_explicit_asymmetric_anchor_parity_checkpoint' if a.checkpoint else 'verified_explicit_asymmetric_anchor_parity_terminal',
         'pairs': len(r['results']), 'required_pairs': 3, 'report_sha256': digest,
         'queries': [x['query'] for x in r['results']], 'scope': r['scope']}
a.output.write_text(json.dumps(audit, indent=2) + '\n')
print(json.dumps(audit))
