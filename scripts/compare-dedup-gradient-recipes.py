#!/usr/bin/env python3
"""Audit fixed/interpolated real-copy measurements without hiding refusals."""
import argparse
import collections
import hashlib
import json
import math
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('manifest', 'fixed', 'fixed_probe', 'interpolated', 'interpolated_probe', 'output'):
    p.add_argument(name, type=Path)
p.add_argument('--checkpoint', action='store_true')
a = p.parse_args()
digest = lambda raw: hashlib.sha256(raw).hexdigest()
raw_manifest = a.manifest.read_bytes()
m = json.loads(raw_manifest)
captures = [path.read_bytes() for path in (a.fixed, a.interpolated)]
reports = [json.loads(raw) for raw in captures]
assert len(m['positive_pairs']) == m['required_positive_pairs'] == 229

def verify_domains(matrix, pair):
    scale = max(abs(v) for row in matrix for v in row)
    assert scale > 0
    h = [[v / scale for v in row] for row in matrix]
    aa, bb, cc = h[0]
    dd, ee, ff = h[1]
    gg, hh, ii = h[2]
    cofactors = [[ee*ii-ff*hh, ff*gg-dd*ii, dd*hh-ee*gg],
                 [cc*hh-bb*ii, aa*ii-cc*gg, bb*gg-aa*hh],
                 [bb*ff-cc*ee, cc*dd-aa*ff, aa*ee-bb*dd]]
    determinant = sum(v*c for v, c in zip(h[0], cofactors[0]))
    assert math.isfinite(determinant) and determinant != 0
    inverse_up_to_scale = [[cofactors[col][row] for col in range(3)] for row in range(3)]
    for transform, side in ((h, 'left'), (inverse_up_to_scale, 'right')):
        width, height = pair[side]['normalized_size']
        values = [transform[2][0]*x + transform[2][1]*y + transform[2][2]
                  for x, y in ((0, 0), (width-1, 0), (0, height-1), (width-1, height-1))]
        assert all(math.isfinite(v) and v != 0 and (v > 0) == (values[0] > 0) for v in values)

for r, mode, probe in zip(reports, ('gradient_pyramid_public_files', 'gradient_pyramid_public_files_interpolated'), (a.fixed_probe, a.interpolated_probe)):
    assert r['mode'] == mode and r['negative_query'] is None
    assert r['manifest_sha256'] == digest(raw_manifest) and r['probe_sha256'] == digest(probe.read_bytes())
    assert r['required_pairs'] == 229 and r['completed_pairs'] == len(r['results'])
    assert 0 < r['completed_pairs'] <= 229
    if not a.checkpoint or r is reports[0]:
        assert r['completed_pairs'] == 229 and r['measurement_complete'] is True
    for row, pair in zip(r['results'], m['positive_pairs']):
        assert row['query_id'] == pair['query_id'] and row['label'] == pair['label']
        assert row['status'] in ('ok', 'pixel_refusal', 'process_error', 'timeout')
        if row['status'] in ('process_error', 'timeout'):
            assert 'evidence' not in row
            continue
        e = row['evidence']
        assert row['returncode'] == 0 and e['status'] == 'ok'
        assert type(e['candidate']) is bool and e['managed_used_after_drop'] == 0
        assert type(e['managed_peak']) is int and 0 <= e['managed_peak'] <= 64*1024*1024
        for key in ('correspondences', 'inliers'):
            assert type(e[key]) is int and e[key] >= 0
        assert e['inliers'] <= e['correspondences']
        if e['geometry'] is None:
            assert e['inliers'] == 0 and e['pixels'] is None and not e['candidate']
        else:
            matrix = e['geometry']
            assert len(matrix) == 3 and all(len(row) == 3 for row in matrix)
            assert all(type(v) in (int, float) and math.isfinite(v) for row in matrix for v in row)
            verify_domains(matrix, pair)
        pixels = e['pixels']
        if pixels and pixels['status'] != 'ok':
            assert row['status'] == 'pixel_refusal' and not e['candidate'] and pixels['error']
        else:
            assert row['status'] == 'ok'
            if pixels:
                assert len(pixels['counts']) == 2 and e['geometry'] is not None
                accepted = True
                for values, side in zip(pixels['counts'], ('left', 'right')):
                    assert len(values) == 3 and all(type(v) is int for v in values)
                    matched, compared, source = values
                    width, height = pair[side]['normalized_size']
                    assert 0 <= matched <= compared <= source == width * height
                    accepted &= compared >= 1000 and compared >= source * .3 and matched >= compared * .9
                assert e['candidate'] == accepted
            else:
                assert not e['candidate']
    assert r['status_counts'] == dict(collections.Counter(v['status'] for v in r['results']))
    assert r['candidates'] == sum(v['status'] == 'ok' and v['evidence']['candidate'] for v in r['results'])
counts, changes = collections.Counter(), []
for fixed, interpolated in zip(reports[0]['results'], reports[1]['results']):
    if fixed['status'] != 'ok' or interpolated['status'] != 'ok':
        category = 'explicit_refusal_or_execution_error'
    else:
        before, after = fixed['evidence']['candidate'], interpolated['evidence']['candidate']
        category = 'retained_candidate' if before and after else 'gain' if after else 'loss' if before else 'both_reject'
    counts[category] += 1
    if category not in ('retained_candidate', 'both_reject'):
        changes.append({'query_id': fixed['query_id'], 'category': category,
                        'fixed_status': fixed['status'], 'interpolated_status': interpolated['status']})
images = {v['normalized_path']: v['normalized_sha256'] for family in m['images'].values() for v in family}
assert len(images) == 386 and all(digest(Path(path).read_bytes()) == expected for path, expected in images.items())
result = {'verified_pairs': reports[1]['completed_pairs'], 'required_pairs': 229,
          'complete': reports[1]['completed_pairs'] == 229, 'counts': dict(counts), 'changes': changes,
          'report_snapshot_sha256': [digest(raw) for raw in captures], 'verified_input_images': 386,
          'scope': 'Ordered publisher-origin pairs, native admission invariants and fixed/interpolated candidate differences. Explicit refusals remain separate; not semantic/burst precision, default promotion or full-library qualification.'}
assert sum(counts.values()) == result['verified_pairs']
temporary = a.output.with_suffix('.tmp')
temporary.write_text(json.dumps(result, indent=2) + '\n')
temporary.replace(a.output)
print(json.dumps({key: result[key] for key in ('verified_pairs', 'required_pairs', 'counts')}, indent=2))
