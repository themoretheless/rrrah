"""Fixed regional projective diagnostic from all frozen learned proposals."""
import hashlib, json, math
from pathlib import Path
import cv2
import numpy as np

D = Path('docs/research')
source = D / 'dedup-low-texture-crumpled-loftr-diagnostic.json'
output = D / 'dedup-low-texture-loftr-regions.json'
assert not output.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
r = json.loads(source.read_text())
assert r['status'] == 'complete_proposal_diagnostic'
pins = {str(source): sha(source), str(Path(__file__)): sha(__file__)}
for p, digest in r['pins'].items():
    assert sha(p) == digest
cv2.setNumThreads(1)
rows = []
for proposal in r['rows']:
    points = proposal['distinct_points']
    for grid in (2, 3, 4):
        cells = []
        union = set()
        for y in range(grid):
            for x in range(grid):
                # Fixed target partition, chosen before observing residuals.
                indices = [i for i, (_, q) in enumerate(points)
                           if x * 800 / grid <= q[0] < (x + 1) * 800 / grid
                           and y * 600 / grid <= q[1] < (y + 1) * 600 / grid]
                cell = dict(cell=[x, y], proposal_indices=indices,
                            matrix=None, bidirectional_inlier_indices=[])
                if len(indices) >= 4:
                    cv2.setRNGSeed(173)
                    H, _ = cv2.findHomography(np.float64([points[i][0] for i in indices]),
                                             np.float64([points[i][1] for i in indices]),
                                             cv2.RANSAC, 2., maxIters=20000, confidence=.999)
                    if H is not None and np.isfinite(H).all() and abs(np.linalg.det(H)) > 1e-15:
                        inverse = np.linalg.inv(H)
                        cell['matrix'] = H.tolist()
                        def project(matrix, point):
                            v = matrix @ np.array([*point, 1.])
                            return v[:2] / v[2]
                        for i in indices:
                            p, q = points[i]
                            if np.linalg.norm(project(H, p) - q) <= 2 and np.linalg.norm(project(inverse, q) - p) <= r['source_tolerance']:
                                cell['bidirectional_inlier_indices'].append(i)
                        union.update(cell['bidirectional_inlier_indices'])
                cells.append(cell)
        rows.append(dict(max_side=proposal['max_side'], clahe=proposal['clahe'],
                         grid=grid, cells=cells, union_inliers=sorted(union),
                         maximum_cell_inliers=max(len(c['bidirectional_inlier_indices']) for c in cells)))
assert all(sha(p) == digest for p, digest in pins.items())
output.write_text(json.dumps(dict(status='complete_fixed_regional_proposal_diagnostic',
    rows=rows, pins=pins, opencv_version=cv2.__version__,
    scope='All frozen proposals partitioned by fixed target grids; same residual tolerances. Four-point fits can interpolate noise. Disconnected inliers do not constitute a conforming mesh or ten witnesses under one model. No native full-domain geometry, original-pixel admission or recovery claim.'), indent=2) + '\n')
print([(v['max_side'], v['clahe'], v['grid'], v['maximum_cell_inliers'], len(v['union_inliers'])) for v in rows])
