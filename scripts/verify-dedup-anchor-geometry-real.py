"""Verify frozen real geometry-only parity, including full native evidence."""
import hashlib
import json
from pathlib import Path

D = Path('docs/research')
report = json.loads((D / 'dedup-anchor-geometry-real-parity.json').read_text())
baseline = json.loads((D / 'dedup-anchor-automatic-pixels-prefix-4.json').read_text())
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert report['status'] == 'complete'
assert report['required_pairs'] == len(report['results']) == 4
for name, digest in report['pins'].items():
    assert sha(name) == digest, name
manifest = json.loads((D / 'dedup-anchor-geometry-real-source.json').read_text())
assert sha(manifest['binary']) == manifest['binary_sha256']
for name, digest in manifest['file_hashes'].items():
    assert sha(Path(manifest['snapshot']) / name) == digest, name
for row, old in zip(report['results'], baseline['results'][:4]):
    for key in ('query', 'original', 'source_tolerance', 'evidence'):
        assert row[key] == old[key], (row['query'], key)
    assert row['returncode'] == 0 and row['seconds'] > 0
    assert row['evidence']['status'] == 'ok' and row['evidence']['supported']
audit = {'status': 'verified_four_real_pairs_exact_native_evidence',
         'report_sha256': sha(D / 'dedup-anchor-geometry-real-parity.json'),
         'pairs': [{'query': r['query'], 'seconds': r['seconds']} for r in report['results']],
         'scope': report['scope']}
(D / 'dedup-anchor-geometry-real-parity-audit.json').write_text(json.dumps(audit, indent=2) + '\n')
print(json.dumps(audit, indent=2))
