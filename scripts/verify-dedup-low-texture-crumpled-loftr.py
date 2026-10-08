"""Verify proposal provenance and original-coordinate residual counts, not pixels."""
import hashlib, json, math
from pathlib import Path
import numpy as np
import cv2

D = Path('docs/research')
path = D / 'dedup-low-texture-crumpled-loftr-diagnostic.json'
out = D / 'dedup-low-texture-crumpled-loftr-diagnostic-audit.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
digest = sha(path)
r = json.loads(path.read_text())
assert r['status'] == 'complete_proposal_diagnostic'
for p, expected in r['pins'].items():
    assert sha(p) == expected, p
assert sha(r['weights']['path']) == r['weights']['sha256']
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
a = cv2.imread(str(T / 'original-resolution-negative-200201/200300.jpg'))
b = cv2.imread(str(T / 'original-resolution-strong-all/200302.jpg'))
tolerance = 2 * math.hypot(*a.shape[:2]) / math.hypot(*b.shape[:2])
assert r['source_tolerance'] == tolerance
assert [(x['max_side'], x['clahe']) for x in r['rows']] == [(320, False), (320, True), (640, False), (640, True)]
counts = []
for row in r['rows']:
    points = row['distinct_points']
    assert len(points) == len(row['confidence']) <= row['raw_matches'] <= 5000
    for i, (p, q) in enumerate(points):
        assert all(math.isfinite(x) for x in p + q)
        assert 0 <= p[0] < a.shape[1] and 0 <= p[1] < a.shape[0]
        assert 0 <= q[0] < b.shape[1] and 0 <= q[1] < b.shape[0]
        assert all(math.dist(p, x[0]) > 2 and math.dist(q, x[1]) > 2 for x in points[:i])
    indices = []
    if row['matrix'] is not None:
        H = np.asarray(row['matrix'])
        inverse = np.linalg.inv(H)
        def project(matrix, point):
            v = matrix @ np.array([*point, 1.])
            return v[:2] / v[2]
        for i, (p, q) in enumerate(points):
            if np.linalg.norm(project(H, p) - q) <= 2 and np.linalg.norm(project(inverse, q) - p) <= tolerance:
                indices.append(i)
    assert indices == row['bidirectional_inlier_indices']
    counts.append(len(indices))
assert sha(path) == digest
out.write_text(json.dumps(dict(status='verified_proposal_diagnostic_no_recovery',
    bidirectional_inliers=counts, maximum_inliers=max(counts),
    pins={str(path): digest, str(Path(__file__)): sha(__file__)},
    scope='Pins, bounds, distinctness and bidirectional residual recomputation only. Maximum seven is below ten native anchor witnesses; no full-domain/pixel/precision proof or learned inference recomputation.'), indent=2) + '\n')
print('verified', counts)
