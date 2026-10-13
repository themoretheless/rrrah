"""Test source-scale/context loss on the unresolved crumpled print.

Every source quadrant is matched against the full target and every target
quadrant, with both fixed raw/CLAHE variants. Full-image baseline, foreign
control and recovered-rephoto control use both variants. Correspondences only:
geometric residuals do not substitute for guarded native pixel admission.
"""
import hashlib
import importlib
import json
import math
import os
from pathlib import Path
import time

import cv2
import kornia
import numpy as np
import torch

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT = D / 'dedup-loftr-both-tiles-20261010.json'
WEIGHTS = T / 'learned-matching-models/loftr_outdoor.ckpt'
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
paths = {
    'source': T / 'original-resolution-negative-200201/200300.jpg',
    'crumpled': T / 'original-resolution-strong-all/200302.jpg',
    'foreign': T / 'original-resolution-negative-200201/201400.jpg',
    'rephoto-source': T / 'original-resolution-negative-200201/209400.jpg',
    'rephoto': T / 'original-resolution-strong-all/209401.jpg',
}
module = importlib.import_module('kornia.feature.loftr.loftr')
assert not OUT.exists()
pins = {str(p): sha(p) for p in [WEIGHTS, Path(__file__), Path(module.__file__), *paths.values()]}
report = {'status': 'running', 'scope': __doc__, 'producer_pid': os.getpid(), 'pins': pins, 'rows': [],
          'versions': {'torch': torch.__version__, 'kornia': kornia.__version__, 'opencv': cv2.__version__, 'numpy': np.__version__},
          'max_side': 640, 'max_raw_proposals_per_attempt': 5000, 'selection_separation': 2.0,
          'mapping': '(model_point + .5) * crop_extent / input_extent - .5 + crop_origin'}


def save():
    temporary = OUT.with_suffix('.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    temporary.replace(OUT)


def fit(points, tolerance):
    result = {'matrix': None, 'bidirectional_inlier_indices': []}
    if len(points) < 4:
        return result
    cv2.setRNGSeed(173)
    p = np.asarray(points, dtype=np.float64)
    matrix, _ = cv2.findHomography(p[:, :2], p[:, 2:], cv2.RANSAC, 2., maxIters=20000, confidence=.999)
    if matrix is None or not np.isfinite(matrix).all() or abs(np.linalg.det(matrix)) < 1e-15:
        return result
    inverse = np.linalg.inv(matrix)
    def project(m, values):
        v = np.column_stack((values, np.ones(len(values)))) @ m.T
        with np.errstate(invalid='ignore', divide='ignore'):
            return v[:, :2] / v[:, 2:]
    good = (np.linalg.norm(project(matrix, p[:, :2]) - p[:, 2:], axis=1) <= 2.) & (np.linalg.norm(project(inverse, p[:, 2:]) - p[:, :2], axis=1) <= tolerance)
    result.update(matrix=matrix.tolist(), bidirectional_inlier_indices=np.flatnonzero(good).tolist())
    return result


def rectangles(image):
    h, w = image.shape
    return [[0, 0, w, h], *[[x*w//2, y*h//2, (x+1)*w//2-x*w//2, (y+1)*h//2-y*h//2] for y in range(2) for x in range(2)]]


def prepare(image, rectangle, clahe):
    x, y, w, h = rectangle
    ratio = min(1., 640/max(w, h))
    iw, ih = max(8, int(w*ratio)//8*8), max(8, int(h*ratio)//8*8)
    cropped = cv2.resize(image[y:y+h, x:x+w], (iw, ih), interpolation=cv2.INTER_AREA)
    if clahe:
        cropped = cv2.createCLAHE(clipLimit=2., tileGridSize=(8, 8)).apply(cropped)
    return torch.from_numpy(cropped.copy()).float()[None, None]/255., np.array([w/iw, h/ih]), np.array([x, y])


save()
try:
    torch.set_num_threads(2)
    torch.manual_seed(173)
    cv2.setNumThreads(1)
    matcher = kornia.feature.LoFTR(pretrained=None).eval()
    matcher.load_state_dict(torch.load(WEIGHTS, map_location='cpu', weights_only=True)['state_dict'])
    images = {name: cv2.imread(str(path), cv2.IMREAD_GRAYSCALE) for name, path in paths.items()}
    assert all(image is not None for image in images.values())
    attempts = []
    for enhanced in (False, True):
        for name, left, right in [('baseline', 'source', 'crumpled'), ('foreign', 'source', 'foreign'), ('rephoto-control', 'rephoto-source', 'rephoto')]:
            attempts.append((name, left, right, rectangles(images[left])[0], rectangles(images[right])[0], enhanced))
    for left_rectangle in rectangles(images['source'])[1:]:
        for right_rectangle in rectangles(images['crumpled']):
            for enhanced in (False, True):
                attempts.append(('both-tiles', 'source', 'crumpled', left_rectangle, right_rectangle, enhanced))
    assert len(attempts) == 46
    for name, left, right, lr, rr, enhanced in attempts:
        row = {'case': name, 'source_rectangle': lr, 'target_rectangle': rr, 'clahe': enhanced, 'status': 'running'}
        report['rows'].append(row)
        save()
        started = time.monotonic()
        a, b = images[left], images[right]
        ia, sa, oa = prepare(a, lr, enhanced)
        ib, sb, ob = prepare(b, rr, enhanced)
        with torch.inference_mode():
            found = matcher({'image0': ia, 'image1': ib})
        lp = (found['keypoints0'].numpy()+.5)*sa-.5+oa
        rp = (found['keypoints1'].numpy()+.5)*sb-.5+ob
        confidence = found['confidence'].numpy()
        assert len(lp) <= 5000
        points, scores = [], []
        for index in np.argsort(-confidence, kind='stable'):
            p, q = lp[index], rp[index]
            if not (0 <= p[0] < a.shape[1] and 0 <= p[1] < a.shape[0] and 0 <= q[0] < b.shape[1] and 0 <= q[1] < b.shape[0]):
                continue
            point = [*p.tolist(), *q.tolist()]
            if all(math.dist(point[:2], old[:2]) > 2 and math.dist(point[2:], old[2:]) > 2 for old in points):
                points.append(point)
                scores.append(float(confidence[index]))
        tolerance = 2*math.hypot(*a.shape)/math.hypot(*b.shape)
        row.update(status='complete_proposals_only', raw_matches=len(lp), distinct_points=points, confidence=scores,
                   source=str(paths[left]), target=str(paths[right]), source_dimensions=[a.shape[1],a.shape[0]], target_dimensions=[b.shape[1],b.shape[0]],
                   source_tolerance=tolerance, geometry=fit(points,tolerance), elapsed_seconds=time.monotonic()-started)
        save()
        print(len(report['rows']), name, lr, rr, enhanced, len(points), len(row['geometry']['bidirectional_inlier_indices']), flush=True)
    # Preserve every distinct union candidate, rather than selecting only a successful tile.
    raw = [(score, point) for row in report['rows'] if row['case'] in ('baseline', 'both-tiles') for score, point in zip(row['confidence'],row['distinct_points'])]
    raw.sort(key=lambda value: -value[0])
    points, scores = [], []
    for score, point in raw:
        if all(math.dist(point[:2],old[:2])>2 and math.dist(point[2:],old[2:])>2 for old in points):
            points.append(point);scores.append(score)
    tolerance=2*math.hypot(*images['source'].shape)/math.hypot(*images['crumpled'].shape)
    union={'distinct_points':points,'confidence':scores,'geometry':fit(points,tolerance),'regions':[]}
    for grid in (2,3,4):
        h,w=images['crumpled'].shape
        for y in range(grid):
            for x in range(grid):
                indices=[i for i,p in enumerate(points) if x*w/grid<=p[2]<(x+1)*w/grid and y*h/grid<=p[3]<(y+1)*h/grid]
                union['regions'].append({'grid':grid,'cell':[x,y],'proposal_indices':indices,**fit([points[i] for i in indices],tolerance)})
    report['crumpled_union']=union
    assert all(sha(path)==digest for path,digest in pins.items())
    report['status']='complete_46_attempt_proposal_diagnostic'
except Exception as error:
    report.update(status='failed',error=f'{type(error).__name__}: {error}')
    raise
finally:
    save()
