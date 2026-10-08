#!/usr/bin/env python3
"""Check four-lane shared-file evidence against both pinned constituent runs."""
import argparse
import collections
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('manifest', 'probe', 'report', 'base', 'gradient'):
    p.add_argument(name, type=Path)
p.add_argument('--checkpoint', action='store_true')
a = p.parse_args()
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
m, r, b, g = [json.loads(path.read_text()) for path in
              (a.manifest, a.report, a.base, a.gradient)]
assert r['mode'] == 'complementary_gradient'
assert b['mode'] == 'complementary_filter_portfolio'
assert g['mode'] == 'gradient_pyramid_public_files'
for v in (r, b, g):
    assert v['manifest_sha256'] == digest(a.manifest)
    assert v['required_pairs'] == 229
    assert v['completed_pairs'] == len(v['results'])
assert r['probe_sha256'] == digest(a.probe)
assert b['completed_pairs'] == g['completed_pairs'] == 229
assert 0 < r['completed_pairs'] <= 229
if not a.checkpoint:
    assert r['completed_pairs'] == 229
    assert r['status'] == 'complete_measurement_not_full_library_qualification'
for row, base, gradient, pair in zip(r['results'], b['results'], g['results'], m['positive_pairs']):
    for v in (row, base, gradient):
        assert v['query_id'] == pair['query_id'] and v['label'] == pair['label']
        assert v['status'] == 'ok' and v['returncode'] == 0
    e, be, ge = [v['evidence'] for v in (row, base, gradient)]
    assert e['status'] == 'ok' and e['managed_used'] == 0
    for key, value in be.items():
        if key not in ('candidate', 'managed_peak'):
            assert e[key] == value, (pair['query_id'], key)
    assert e['three_candidate'] == be['candidate']
    for key in ('candidate', 'correspondences', 'inliers', 'geometry'):
        assert e['gradient_' + key] == ge[key], (pair['query_id'], key)
    pixels = ge['pixels']
    assert e['gradient_counts'] == (pixels['counts'] if pixels and pixels['status'] == 'ok' else None)
    failure = 'None' if not pixels or pixels['status'] == 'ok' else pixels['error']
    assert e['gradient_fit_failure'] == failure
    assert e['four_accepted_searches'] == be['joined_accepted_searches'] + [ge['candidate']]
    assert type(e['candidate']) is bool and e['candidate'] == any(e['four_accepted_searches'])
    if e['gradient_counts'] is not None:
        accepted = True
        for matched, compared, source in e['gradient_counts']:
            assert 0 <= matched <= compared <= source and source > 0
            accepted &= compared >= 1000 and compared >= source * .3 and matched >= compared * .9
        assert ge['candidate'] == accepted
    for side in ('left', 'right'):
        image = pair[side]
        assert digest(Path(image['normalized_path'])) == image['normalized_sha256']
assert r['status_counts'] == dict(collections.Counter(v['status'] for v in r['results']))
assert r['candidates'] == sum(v['evidence']['candidate'] for v in r['results'])
print(json.dumps({'verified_pairs': r['completed_pairs'], 'required_pairs': 229,
                  'candidates': r['candidates'], 'all_constituent_fields_equal': True,
                  'report_sha256': digest(a.report), 'probe_sha256': digest(a.probe),
                  'scope': 'Constituent parity and pinned input integrity; not full-library or negative qualification.'}, indent=2))
