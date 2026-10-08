"""Bounded independent dense proposals, not native geometry/pixel admission."""
import hashlib
import json
import math
from pathlib import Path
import cv2
import numpy as np

D = Path('docs/research')
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
source = T / 'original-resolution-negative-200201' / '200300.jpg'
target = T / 'original-resolution-strong-all' / '200302.jpg'
baseline = D / 'dedup-low-texture-crumpled-sift-diagnostic.json'
out = D / 'dedup-low-texture-crumpled-flow-diagnostic.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins = {str(p): sha(p) for p in [source, target, baseline, Path(__file__)]}
prior = json.loads(baseline.read_text())
for p in (source, target):
    assert sha(p) == prior['pins'][str(p)]
cv2.setNumThreads(1)
a = cv2.imread(str(source), cv2.IMREAD_GRAYSCALE)
b = cv2.imread(str(target), cv2.IMREAD_GRAYSCALE)
assert a is not None and b is not None and b.size <= 640000
height, width = b.shape
tolerance = 2 * math.hypot(a.shape[1], a.shape[0]) / math.hypot(width, height)
rows = []
def apply(matrix, point):
    v = matrix @ np.array([*point, 1.])
    return v[:2] / v[2]
for seed_index, seed in enumerate(prior['rows']):
    if seed['matrix'] is None:
        continue
    H = np.array(seed['matrix'], dtype=np.float64)
    inverse = np.linalg.inv(H)
    warped = cv2.warpPerspective(a, H, (width, height))
    mask = cv2.warpPerspective(np.ones_like(a, dtype=np.uint8), H, (width, height), flags=cv2.INTER_NEAREST)
    for enhanced in [False, True]:
        clahe = cv2.createCLAHE(clipLimit=2., tileGridSize=(8, 8))
        aa = clahe.apply(warped) if enhanced else warped
        bb = clahe.apply(b) if enhanced else b
        forward = cv2.calcOpticalFlowFarneback(aa, bb, None, .5, 5, 21, 5, 7, 1.5, 0)
        reverse = cv2.calcOpticalFlowFarneback(bb, aa, None, .5, 5, 21, 5, 7, 1.5, 0)
        assert np.all(np.isfinite(forward)) and np.all(np.isfinite(reverse))
        candidates = []
        tested = cycle_passed = 0
        for y in range(16, height - 16, 16):
            for x in range(16, width - 16, 16):
                tested += 1
                if not np.all(mask[y-7:y+8, x-7:x+8]):
                    continue
                q = np.array([x, y], dtype=np.float64) + forward[y, x]
                if not (8 <= q[0] < width-8 and 8 <= q[1] < height-8):
                    continue
                ix, iy = map(math.floor, q)
                fx, fy = q[0] - ix, q[1] - iy
                back = ((1-fx)*(1-fy)*reverse[iy, ix] + fx*(1-fy)*reverse[iy, ix+1]
                        + (1-fx)*fy*reverse[iy+1, ix] + fx*fy*reverse[iy+1, ix+1])
                cycle = float(np.linalg.norm(q + back - [x, y]))
                if cycle > 1.:
                    continue
                cycle_passed += 1
                pa = aa[y-7:y+8, x-7:x+8].astype(np.float64)
                pb = cv2.getRectSubPix(bb, (15, 15), tuple(map(float, q))).astype(np.float64)
                if min(pa.std(), pb.std()) < 2.:
                    continue
                pa -= pa.mean()
                pb -= pb.mean()
                score = float(np.sum(pa * pb) / math.sqrt(np.sum(pa*pa) * np.sum(pb*pb)))
                left = apply(inverse, [x, y])
                if not np.all(np.isfinite(left)) or not (0 <= left[0] < a.shape[1] and 0 <= left[1] < a.shape[0]):
                    continue
                if score >= .85:
                    candidates.append((score, left.tolist(), q.tolist()))
        assert tested <= 2400
        points = []
        for _, left, right in sorted(candidates, reverse=True):
            if all(math.dist(left, p[0]) > 2 and math.dist(right, p[1]) > 2 for p in points):
                points.append([left, right])
        row = {'seed_index': seed_index, 'clahe': enhanced, 'tested_sites': tested,
               'cycle_passed': cycle_passed, 'ncc_candidates': len(candidates),
               'points': points, 'matrix': None, 'bidirectional_inliers': []}
        if len(points) >= 4:
            cv2.setRNGSeed(173)
            fitted, _ = cv2.findHomography(np.float64([p[0] for p in points]), np.float64([p[1] for p in points]),
                                          cv2.RANSAC, 2., maxIters=20000, confidence=.999)
            if fitted is not None and np.all(np.isfinite(fitted)) and abs(np.linalg.det(fitted)) > 1e-12:
                inv = np.linalg.inv(fitted)
                row['matrix'] = fitted.tolist()
                row['bidirectional_inliers'] = [i for i, (left, right) in enumerate(points)
                    if np.linalg.norm(apply(fitted, left)-right) <= 2
                    and np.linalg.norm(apply(inv, right)-left) <= tolerance]
        rows.append(row)
assert all(sha(p) == pin for p, pin in pins.items())
report = {'status': 'complete_independent_dense_proposal_diagnostic', 'opencv_version': cv2.__version__,
          'pins': pins, 'source_tolerance': tolerance, 'rows': rows,
          'scope': 'Four prior weak SIFT model seeds; dense forward/backward Farneback on <=640k target sites, fixed16pixel proposal lattice<=2400sites/variant, cycle<=1,15pixel NCC>=.85 and std>=2; aliases>2 both original coordinates; all surviving points retained for independent robust fit target2/source2diagonal ratio. No native domain/resource/pixel qualification, negatives, Rust integration or acceptance-threshold change. Seed models and fitted proposals are not duplicate evidence.'}
out.write_text(json.dumps(report, indent=2) + '\n')
print([(x['seed_index'], x['clahe'], len(x['points']), len(x['bidirectional_inliers'])) for x in rows])
