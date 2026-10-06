#!/usr/bin/env python3
"""Verify a finite two-filter Copydays run against both frozen standalone runs."""
import argparse
import collections
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('manifest', type=Path)
p.add_argument('probe', type=Path)
p.add_argument('report', type=Path)
p.add_argument('primary', type=Path)
p.add_argument('secondary', type=Path)
p.add_argument('--checkpoint', action='store_true')
a = p.parse_args()
m = json.loads(a.manifest.read_text())
r = json.loads(a.report.read_text())
assert r['manifest_sha256'] == hashlib.sha256(a.manifest.read_bytes()).hexdigest()
assert r['probe_sha256'] == hashlib.sha256(a.probe.read_bytes()).hexdigest()
assert r['mode'] == 'pyramid_filter_portfolio'
assert r['required_pairs'] == 229
rows = r['results']
assert r['completed_pairs'] == len(rows)
assert 0 < len(rows) <= 229
if not a.checkpoint:
    assert len(rows) == 229
    assert r['status'] == 'complete_measurement_not_full_library_qualification'
assert r['status_counts'] == dict(collections.Counter(x['status'] for x in rows))
standalone = [json.loads(x.read_text()) for x in [a.primary, a.secondary]]
for x, mode in zip(standalone, ['pyramid_file', 'pyramid_blur_radius3']):
    assert x['manifest_sha256'] == r['manifest_sha256']
    assert x['mode'] == mode and x['completed_pairs'] == 229
    assert len(x['results']) == 229 and all(y['status'] == 'ok' for y in x['results'])
refs = [{x['query_id']: x['evidence'] for x in report['results']} for report in standalone]
verified_images = set()
for pair, row in zip(m['positive_pairs'], rows):
    assert row['query_id'] == pair['query_id'] and row['label'] == pair['label']
    assert row['status'] == 'ok' and row['returncode'] == 0
    e = row['evidence']
    assert e['status'] == 'ok'
    assert isinstance(e['candidate'], bool)
    expected = [x[row['query_id']] for x in refs]
    assert e['accepted_filters'] == [x['candidate'] for x in expected]
    assert e['primary_candidate'] == expected[0]['candidate']
    assert e['candidate'] == any(e['accepted_filters'])
    for x in expected:
        for key in ['geometry', 'correspondences', 'inliers']:
            assert e[key] == x[key], (row['query_id'], key)
    assert e['fitted_counts'] == expected[0]['fitted_counts']
    assert e['fit_failure'] == expected[0]['fit_failure']
    assert e['secondary_counts'] == expected[1]['fitted_counts']
    assert e['secondary_fit_failure'] == expected[1]['fit_failure']
    assert e['managed_used'] == 0 and 0 <= e['managed_peak'] <= 64 * 1024 * 1024
    for lane, key in zip(e['accepted_filters'], ['fitted_counts', 'secondary_counts']):
        if lane:
            counts = e[key]
            assert counts is not None and len(counts) == 2
            for matched, compared, source in counts:
                assert all(isinstance(v, int) for v in [matched, compared, source])
                assert 0 <= matched <= compared <= source
                assert compared >= 1000 and compared >= .3 * source and matched >= .9 * compared
    for side in ['left', 'right']:
        value = pair[side]
        path = value['normalized_path']
        if path not in verified_images:
            assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == value['normalized_sha256']
            verified_images.add(path)
assert r['candidates'] == sum(x['evidence']['candidate'] for x in rows)
print(json.dumps({'status': 'verified_checkpoint' if a.checkpoint else 'verified_full_finite_measurement',
    'completed_pairs': len(rows), 'accepted': r['candidates'],
    'normalized_images_verified': len(verified_images),
    'scope': 'Exact reported native geometry, counts, fitting refusals and acceptance union match both frozen standalone runs. Not full-library coverage or semantic/burst precision.'}, indent=2))
