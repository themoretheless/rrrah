#!/usr/bin/env python3
"""Require exact reported native parity; do not infer it from candidate totals."""
import argparse
import collections
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('manifest', 'probe', 'report', 'reference'):
    p.add_argument(name, type=Path)
p.add_argument('--checkpoint', action='store_true')
a = p.parse_args()
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
m = json.loads(a.manifest.read_text())
r = json.loads(a.report.read_text())
reference = json.loads(a.reference.read_text())
assert r['manifest_sha256'] == reference['manifest_sha256'] == digest(a.manifest)
assert r['probe_sha256'] == digest(a.probe)
assert r['mode'] == 'gradient_pyramid_public_files' and r['negative_query'] is None
assert reference['mode'] == 'gradient_pyramid_three_levels' and reference['negative_query'] is None
assert r['required_pairs'] == reference['required_pairs'] == len(m['positive_pairs']) == 229
assert reference['measurement_complete'] and reference['completed_pairs'] == len(reference['results']) == 229
assert r['completed_pairs'] == len(r['results']) <= 229
if not a.checkpoint:
    assert r['measurement_complete'] and r['completed_pairs'] == 229
checked = set()
for row, pair, old in zip(r['results'], m['positive_pairs'], reference['results']):
    assert row['query_id'] == old['query_id'] == pair['query_id']
    assert row['label'] == old['label'] == pair['label'] == 'publisher_origin_copy'
    assert row['status'] == old['status'] == 'ok' and row['returncode'] == old['returncode'] == 0
    e = row['evidence']
    assert e['managed_used_after_drop'] == 0 and type(e['candidate']) is bool
    for key in ('candidate', 'correspondences', 'inliers', 'geometry', 'pixels'):
        assert e[key] == old['evidence'][key], (row['query_id'], key)
    if e['pixels'] is not None:
        assert e['pixels']['status'] == 'ok'
        counts = e['pixels']['counts']
        assert len(counts) == 2
        for matched, compared, source in counts:
            assert all(type(v) is int for v in (matched, compared, source))
            assert 0 <= matched <= compared <= source and source > 0
        assert e['candidate'] == all(c >= 1000 and c >= s * .3 and n >= c * .9 for n, c, s in counts)
    else:
        assert e['geometry'] is None and not e['candidate']
    for side in ('left', 'right'):
        image = pair[side]
        path = Path(image['normalized_path'])
        if path not in checked:
            assert digest(path) == image['normalized_sha256']
            checked.add(path)
assert r['status_counts'] == dict(collections.Counter(v['status'] for v in r['results']))
assert r['candidates'] == sum(v['evidence']['candidate'] for v in r['results'])
print(json.dumps({'completed_pairs': r['completed_pairs'], 'candidates': r['candidates'],
    'verified_images': len(checked), 'all_native_reported_fields_equal': True,
    'scope': 'Public-file/native diagnostic equality only; no fourth-lane union/index or full-library qualification.'}, indent=2))
