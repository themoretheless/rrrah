#!/usr/bin/env python3
"""Check five-lane shared-file evidence against both pinned constituent runs."""
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
captured = {path: path.read_bytes() for path in (a.manifest, a.report, a.base, a.gradient)}
m, r, b, g = [json.loads(captured[path]) for path in
              (a.manifest, a.report, a.base, a.gradient)]
assert r['mode'] == 'complementary_gradient_portfolio'
assert b['mode'] == 'complementary_gradient'
assert g['mode'] == 'gradient_pyramid_public_files_interpolated'
for v in (r, b, g):
    assert v['manifest_sha256'] == digest(a.manifest)
    assert v['required_pairs'] == 229
    assert v['completed_pairs'] == len(v['results'])
    assert v['status_counts'] == dict(collections.Counter(row['status'] for row in v['results']))
    assert v['candidates'] == sum(row['evidence']['candidate'] for row in v['results'])
assert r['probe_sha256'] == digest(a.probe)
assert b['probe_sha256'] == digest(a.probe.parent / 'photo-probe-complementary-gradient')
assert g['probe_sha256'] == digest(a.probe.parent / 'photo-probe-gradient-interpolated-file')
assert b['completed_pairs'] == g['completed_pairs'] == 229
assert 0 < r['completed_pairs'] <= 229
if not a.checkpoint:
    assert r['completed_pairs'] == 229
    assert r['status'] == 'complete_measurement_not_full_library_qualification'
gains, losses = [], []
verified_images=set()
for row, base, gradient, pair in zip(r['results'], b['results'], g['results'], m['positive_pairs']):
    for v in (row, base, gradient):
        assert v['query_id'] == pair['query_id'] and v['label'] == pair['label']
        assert v['status'] == 'ok' and v['returncode'] == 0
    e, be, ge = [v['evidence'] for v in (row, base, gradient)]
    assert e['status'] == 'ok' and e['managed_used'] == 0
    assert type(e['managed_peak']) is int and 0 < e['managed_peak'] <= 64_000_000
    for name, count in [('five_accepted_searches',5), ('four_accepted_searches',4)]:
        assert len(e[name]) == count and all(type(value) is bool for value in e[name])
    assert type(e['interpolated_candidate']) is bool and type(e['four_candidate']) is bool
    for key, value in be.items():
        if key not in ('candidate', 'managed_peak'):
            assert e[key] == value, (pair['query_id'], key)
    assert e['four_candidate'] == be['candidate']
    for key in ('candidate', 'correspondences', 'inliers', 'geometry'):
        assert e['interpolated_' + key] == ge[key], (pair['query_id'], key)
    pixels = ge['pixels']
    assert e['interpolated_counts'] == (pixels['counts'] if pixels and pixels['status'] == 'ok' else None)
    failure = 'None' if not pixels or pixels['status'] == 'ok' else ('Some(' + pixels['error'][4:-1] + ')' if pixels['error'].startswith('Fit(') else pixels['error'])
    assert e['interpolated_fit_failure'] == failure
    assert e['five_accepted_searches'] == be['four_accepted_searches'] + [ge['candidate']]
    assert type(e['candidate']) is bool and e['candidate'] == any(e['five_accepted_searches'])
    if e['candidate'] and not be['candidate']: gains.append(pair['query_id'])
    if be['candidate'] and not e['candidate']: losses.append(pair['query_id'])
    if e['interpolated_counts'] is not None:
        accepted = True
        for matched, compared, source in e['interpolated_counts']:
            assert 0 <= matched <= compared <= source and source > 0
            accepted &= compared >= 1000 and compared >= source * .3 and matched >= compared * .9
        assert ge['candidate'] == accepted
    for side in ('left', 'right'):
        image = pair[side]
        assert digest(Path(image['normalized_path'])) == image['normalized_sha256']
        verified_images.add(image['normalized_path'])
assert r['status_counts'] == dict(collections.Counter(v['status'] for v in r['results']))
assert r['candidates'] == sum(v['evidence']['candidate'] for v in r['results'])
for path, payload in captured.items():
    assert path.read_bytes() == payload, f'Report changed during verification: {path}'
print(json.dumps({'verified_pairs': r['completed_pairs'], 'required_pairs': 229,
                  'candidates': r['candidates'], 'all_constituent_fields_equal': True,
                  'verified_images':len(verified_images),
                  'gains_over_four': gains, 'losses_over_four': losses,
                  'report_sha256': digest(a.report), 'probe_sha256': digest(a.probe),
                  'constituent_report_sha256': [digest(a.base),digest(a.gradient)],
                  'constituent_probe_sha256': [b['probe_sha256'],g['probe_sha256']],
                  'scope': 'Constituent parity and pinned input integrity; not full-library or negative qualification.'}, indent=2))
