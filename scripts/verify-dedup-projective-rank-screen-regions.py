#!/usr/bin/env python3
"""Verify cached center-region counts against previously independently audited pixels."""
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
def sha(path):
    p = Path(path)
    return hashlib.sha256((p if p.is_absolute() else ROOT / p).read_bytes()).hexdigest()
def read(name):
    return json.loads((D / name).read_text())
pins = read('dedup-projective-rank-screen-regions-pins.json')
for path, digest in pins.items():
    assert sha(path) == digest, path
reference = read('dedup-filtered-rank-real.json')
oracle = read('dedup-rank-pixel-oracle-audit.json')
assert oracle['status'] == 'verified_independent_rank_pixel_math'
assert oracle['input_hashes']['docs/research/dedup-filtered-rank-real.json'] == sha('docs/research/dedup-filtered-rank-real.json')
rows = [json.loads(line) for line in (D / 'dedup-projective-rank-screen-regions.jsonl').read_text().splitlines()]
assert [(r['direction'], r['filter_radius']) for r in rows] == [(d, f) for d in ('forward', 'reverse') for f in (0, 3, 8)]
for row in rows:
    index = ('forward', 'reverse').index(row['direction'])
    ref = next(r for r in reference['rows'] if r['case'] == 'screen_control' and r['filter_radius'] == row['filter_radius'])
    independent = next(r for r in oracle['summary'] if r['case'] == 'screen_control' and r['filter_radius'] == row['filter_radius'])
    expected = ref['evidence']['directions'][index]
    for key in ('sites', 'valid_sites', 'informative_pairs', 'agreeing_pairs'):
        assert row[key] == expected[key], (row, key)
    for key in ('valid_sites', 'informative_pairs', 'agreeing_pairs'):
        assert row[key] == independent['directions'][index][key], (row, key)
    assert row['pixel_reads'] == row['covered'] * 5
old = read('dedup-projective-rank-screen-pins.json')
assert sha('docs/research/dedup-mesh-rank-before-center-regions.rs') == old['crates/rrrah-dedup/src/mesh_rank.rs']
for name in ('dedup-projective-rank-screen-regions.jsonl', 'dedup-projective-rank-screen-regions.log'):
    pins['docs/research/' + name] = sha('docs/research/' + name)
pins['scripts/verify-dedup-projective-rank-screen-regions.py'] = sha('scripts/verify-dedup-projective-rank-screen-regions.py')
result = dict(status='verified_cached_projective_center_region_pixel_counts', directions=6, rows=rows, pins=pins,
    scope='Exact six count comparisons against direct reference and its independent NumPy pixel audit. Shared normalized decoder inputs; no independent decoding, runtime benchmark, full copy admission, broad precision or Linux qualification. Covered counts atlas context; sites counts selected centers. Unsupported context pixels may refuse even outside center region.')
(D / 'dedup-projective-rank-screen-regions-audit.json').write_text(json.dumps(result, indent=2) + '\n')
print(result['status'], '6 directions')
