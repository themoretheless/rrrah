"""Independently check recorded RoMa fields, center mapping and residuals.

No inference rerun or pixel-admission claim. This verifies the fixed diagnostic,
including the failed crumpled recovery, rather than selecting only its control.
"""
import hashlib
import json
import math
from pathlib import Path
import re

import numpy as np

D = Path(__file__).resolve().parents[1] / 'docs/research'
SOURCE = D / 'dedup-roma-crumpled-20261010.json'
OUT = D / 'dedup-roma-crumpled-audit-20261010.json'
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
r = json.loads(SOURCE.read_text())
assert not OUT.exists()
assert r['status'] == 'complete_three_pair_proposal_diagnostic'
assert [row['case'] for row in r['rows']] == ['crumpled', 'foreign', 'rephoto-control']
assert all(sha(path) == digest for path, digest in r['pins'].items())
source_manifest = json.loads((D / 'dedup-roma-oracle-source-20261010.json').read_text())
assert all(sha(Path(source_manifest['path']) / path) == digest for path, digest in source_manifest['source_hashes'].items())
weight_manifest = json.loads((D / 'dedup-roma-oracle-weights-20261010.json').read_text())
assert all(sha(row['path']) == row['sha256'] for row in weight_manifest['weights'])
counts = []


def residuals(points, geometry, tolerance):
    if geometry['matrix'] is None:
        assert geometry['bidirectional_inlier_indices'] == []
        return
    matrix = np.asarray(geometry['matrix'], dtype=np.float64)
    inverse = np.linalg.inv(matrix)
    expected = []
    for index, point in enumerate(points):
        def project(m, p):
            v = m @ np.asarray([p[0], p[1], 1.])
            return v[:2] / v[2]
        if np.linalg.norm(project(matrix, point[:2]) - point[2:]) <= 2. and np.linalg.norm(project(inverse, point[2:]) - point[:2]) <= tolerance:
            expected.append(index)
    assert expected == geometry['bidirectional_inlier_indices']


for row in r['rows']:
    assert row['status'] == 'complete_proposals_only'
    assert sha(row['field']['path']) == row['field']['sha256']
    assert sha(row['points_file']['path']) == row['points_file']['sha256']
    data = np.load(row['field']['path'])
    warp, certainty = data['warp'], data['certainty']
    assert warp.shape == (1, 864, 1728, 4) and certainty.shape == (1, 864, 1728)
    assert np.isfinite(warp).all() and np.isfinite(certainty).all()
    assert ((certainty >= 0) & (certainty <= 1)).all()
    a, b = row['source_dimensions'], row['target_dimensions']
    assert row['source_tolerance'] == 2 * math.hypot(*a) / math.hypot(*b)
    candidates = []
    for reverse in (False, True):
        for y in range(12, 864, 24):
            for x in range(12, 864, 24):
                at = x + (864 if reverse else 0)
                score = float(certainty[0, y, at])
                if score < .05:
                    continue
                v = warp[0, y, at].astype(np.float64)
                point = [*(v[:2] + 1) * np.asarray(a) / 2 - .5, *(v[2:] + 1) * np.asarray(b) / 2 - .5]
                if 0 <= point[0] < a[0] and 0 <= point[1] < a[1] and 0 <= point[2] < b[0] and 0 <= point[3] < b[1]:
                    candidates.append((score, point, reverse))
    candidates.sort(key=lambda value: -value[0])
    points, scores, directions = [], [], []
    for score, point, direction in candidates:
        if all(math.dist(point[:2], old[:2]) > 2 and math.dist(point[2:], old[2:]) > 2 for old in points):
            points.append(point); scores.append(score); directions.append(direction)
    assert points == row['distinct_points'] and scores == row['confidence'] and directions == row['reverse']
    assert len(candidates) == row['raw_lattice_candidates']
    residuals(points, row['global_geometry'], row['source_tolerance'])
    assert len(row['regions']) == 4 + 9 + 16
    for region in row['regions']:
        grid = region['grid']; x, y = region['cell']
        indices = [i for i, point in enumerate(points) if x*b[0]/grid <= point[2] < (x+1)*b[0]/grid and y*b[1]/grid <= point[3] < (y+1)*b[1]/grid]
        assert indices == region['proposal_indices']
        residuals([points[i] for i in indices], region, row['source_tolerance'])
    counts.append({'case': row['case'], 'distinct_points': len(points), 'global_inliers': len(row['global_geometry']['bidirectional_inlier_indices']),
                   'maximum_regional_inliers': max(len(region['bidirectional_inlier_indices']) for region in row['regions'])})
assert counts[0]['global_inliers'] < 10 and counts[0]['maximum_regional_inliers'] < 10
assert counts[1]['distinct_points'] < 4 and counts[2]['global_inliers'] >= 10
log = D / 'dedup-roma-crumpled-20261010.log'
rss = int(re.search(r'(\d+)\s+maximum resident set size', log.read_text())[1])
OUT.write_text(json.dumps({'status': 'verified_three_pair_diagnostic_no_crumpled_recovery', 'scope': __doc__, 'counts': counts,
    'process_peak_rss_bytes': rss, 'model_adopted': False,
    'pins': {str(p): sha(p) for p in [SOURCE, log, Path(__file__), D/'dedup-roma-oracle-source-20261010.json', D/'dedup-roma-oracle-dependency-pins-20261010.json', D/'dedup-roma-oracle-weights-20261010.json']}}, indent=2) + '\n')
print(counts, 'peak RSS bytes', rss)
