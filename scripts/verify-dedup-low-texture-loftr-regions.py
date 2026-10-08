"""Recompute every fixed-cell membership and residual from saved proposals."""
import hashlib, json
from pathlib import Path
import numpy as np

D = Path('docs/research')
p = D / 'dedup-low-texture-loftr-regions.json'
out = D / 'dedup-low-texture-loftr-regions-audit.json'
assert not out.exists()
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
digest = sha(p)
r = json.loads(p.read_text())
for path, pin in r['pins'].items():
    assert sha(path) == pin
base = json.loads((D / 'dedup-low-texture-crumpled-loftr-diagnostic.json').read_text())
assert r['status'] == 'complete_fixed_regional_proposal_diagnostic'
assert [(v['max_side'], v['clahe'], v['grid']) for v in r['rows']] == [
    (a['max_side'], a['clahe'], grid) for a in base['rows'] for grid in (2, 3, 4)]
maximum = 0
for row in r['rows']:
    proposal = next(a for a in base['rows'] if (a['max_side'], a['clahe']) == (row['max_side'], row['clahe']))
    points = proposal['distinct_points']
    grid = row['grid']
    assert [c['cell'] for c in row['cells']] == [[x, y] for y in range(grid) for x in range(grid)]
    union, partition = set(), []
    for cell in row['cells']:
        x, y = cell['cell']
        indices = [i for i, (_, q) in enumerate(points)
                   if x * 800 / grid <= q[0] < (x + 1) * 800 / grid
                   and y * 600 / grid <= q[1] < (y + 1) * 600 / grid]
        assert indices == cell['proposal_indices']
        partition.extend(indices)
        inliers = []
        if cell['matrix'] is not None:
            assert len(indices) >= 4
            H = np.array(cell['matrix'])
            inverse = np.linalg.inv(H)
            def project(matrix, point):
                v = matrix @ np.array([*point, 1.])
                return v[:2] / v[2]
            for i in indices:
                a, b = points[i]
                if np.linalg.norm(project(H, a) - b) <= 2 and np.linalg.norm(project(inverse, b) - a) <= base['source_tolerance']:
                    inliers.append(i)
        assert inliers == cell['bidirectional_inlier_indices']
        union.update(inliers)
    assert sorted(partition) == list(range(len(points)))
    assert sorted(union) == row['union_inliers']
    m = max(len(c['bidirectional_inlier_indices']) for c in row['cells'])
    assert m == row['maximum_cell_inliers']
    maximum = max(maximum, m)
assert sha(p) == digest
out.write_text(json.dumps(dict(status='verified_fixed_cell_diagnostic_no_recovery',
    variants=len(r['rows']), maximum_cell_inliers=maximum,
    pins={str(p): digest, str(Path(__file__)): sha(__file__)},
    scope='Independent partition and bidirectional residual/count recomputation. No RANSAC repeat, native conforming mesh, original pixel admission or recovery.'), indent=2) + '\n')
print('verified', len(r['rows']), maximum)
