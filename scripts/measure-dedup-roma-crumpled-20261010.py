"""Pinned upstream RoMa CPU proposal diagnostic, not duplicate admission.

Default 560 coarse / 864 refinement, two-way matching. Deterministic fixed
24-site lattice instead of stochastic sampling/KDE; confidence 0.05 is the
upstream sampling threshold. Native pixel decisions remain independently
required. Foreign-image control and an existing rephoto control are mandatory.
"""
import hashlib
import json
import math
import os
from pathlib import Path
import time

import cv2
import numpy as np
from PIL import Image
import torch
import torch.nn.functional as F
import torchvision
from romatch import roma_outdoor

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT = D / 'dedup-roma-crumpled-20261010.json'
DATA = T / 'roma-crumpled-20261010'
SOURCE = D / 'dedup-roma-oracle-source-20261010.json'
WEIGHTS = D / 'dedup-roma-oracle-weights-20261010.json'
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
source_manifest = json.loads(SOURCE.read_text())
weight_manifest = json.loads(WEIGHTS.read_text())
assert weight_manifest['status'] == 'complete'
weights = {Path(row['path']).name: row for row in weight_manifest['weights']}
cases = [
    ('crumpled', T / 'original-resolution-negative-200201/200300.jpg', T / 'original-resolution-strong-all/200302.jpg'),
    ('foreign', T / 'original-resolution-negative-200201/200300.jpg', T / 'original-resolution-negative-200201/201400.jpg'),
    ('rephoto-control', T / 'original-resolution-negative-200201/209400.jpg', T / 'original-resolution-strong-all/209401.jpg'),
]


def verify():
    for relative, digest in source_manifest['source_hashes'].items():
        assert sha(Path(source_manifest['path']) / relative) == digest
    for row in weights.values():
        assert sha(row['path']) == row['sha256']


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
    H, _ = cv2.findHomography(p[:, :2], p[:, 2:], cv2.RANSAC, 2.0, maxIters=20000, confidence=.999)
    if H is None or not np.isfinite(H).all() or abs(np.linalg.det(H)) < 1e-15:
        return result
    inverse = np.linalg.inv(H)
    def project(matrix, values):
        homogeneous = np.column_stack((values, np.ones(len(values)))) @ matrix.T
        with np.errstate(divide='ignore', invalid='ignore'):
            return homogeneous[:, :2] / homogeneous[:, 2:]
    good = (np.linalg.norm(project(H, p[:, :2]) - p[:, 2:], axis=1) <= 2.0) & (np.linalg.norm(project(inverse, p[:, 2:]) - p[:, :2], axis=1) <= tolerance)
    result.update(matrix=H.tolist(), bidirectional_inlier_indices=np.flatnonzero(good).tolist())
    return result


assert not OUT.exists() and not DATA.exists()
verify()
DATA.mkdir()
pins = {str(path): sha(path) for path in [SOURCE, WEIGHTS, Path(__file__)] + [p for _, a, b in cases for p in (a, b)]}
report = {'status': 'running', 'scope': __doc__, 'producer_pid': os.getpid(), 'pins': pins, 'rows': [],
          'versions': {'torch': torch.__version__, 'torchvision': torchvision.__version__, 'opencv': cv2.__version__, 'numpy': np.__version__},
          'coordinate_mapping': '(normalized + 1) * original_extent / 2 - 0.5; zero-based pixel centers',
          'coarse_resolution': 560, 'refinement_resolution': 864, 'lattice_stride': 24, 'proposal_confidence': .05}
save()
try:
    torch.set_num_threads(2)
    torch.manual_seed(173)
    torch.set_float32_matmul_precision('highest')
    cv2.setNumThreads(1)
    matcher = roma_outdoor(device='cpu',
        weights=torch.load(weights['roma_outdoor.pth']['path'], map_location='cpu', weights_only=True),
        dinov2_weights=torch.load(weights['dinov2_vitl14_pretrain.pth']['path'], map_location='cpu', weights_only=True),
        amp_dtype=torch.float32, use_custom_corr=False).eval()
    for name, a_path, b_path in cases:
        started = time.monotonic()
        row = {'case': name, 'status': 'running', 'source': str(a_path), 'target': str(b_path)}
        report['rows'].append(row)
        save()
        with Image.open(a_path) as image:
            a = image.convert('RGB')
        with Image.open(b_path) as image:
            b = image.convert('RGB')
        tolerance = 2 * math.hypot(*a.size) / math.hypot(*b.size)
        with torch.inference_mode():
            warp, certainty = matcher.match(a, b, device='cpu')
        assert warp.shape == (1, 864, 1728, 4) and certainty.shape == (1, 864, 1728)
        assert bool(torch.isfinite(warp).all()) and bool(torch.isfinite(certainty).all())
        field_path = DATA / (name + '.npz')
        np.savez_compressed(field_path, warp=warp.numpy(), certainty=certainty.numpy())
        # Sample the two directions on the same fixed predeclared lattice.
        raw = []
        for reverse in (False, True):
            for y in range(12, 864, 24):
                for x in range(12, 864, 24):
                    at = x + (864 if reverse else 0)
                    confidence = float(certainty[0, y, at])
                    if confidence < .05:
                        continue
                    v = warp[0, y, at].double().numpy()
                    p = (v[:2] + 1) * np.asarray(a.size) / 2 - .5
                    q = (v[2:] + 1) * np.asarray(b.size) / 2 - .5
                    if not (0 <= p[0] < a.width and 0 <= p[1] < a.height and 0 <= q[0] < b.width and 0 <= q[1] < b.height):
                        continue
                    raw.append((confidence, [*p.tolist(), *q.tolist()], reverse))
        raw.sort(key=lambda value: -value[0])
        points, scores, directions = [], [], []
        for confidence, point, reverse in raw:
            if all(math.dist(point[:2], old[:2]) > 2 and math.dist(point[2:], old[2:]) > 2 for old in points):
                points.append(point)
                scores.append(confidence)
                directions.append(reverse)
        assert len(points) <= 2592
        point_path = DATA / (name + '-points.txt')
        point_path.write_text(''.join(' '.join(map(str, point)) + '\n' for point in points))
        row.update(source_dimensions=list(a.size), target_dimensions=list(b.size), source_tolerance=tolerance,
                   raw_lattice_candidates=len(raw), distinct_points=points, confidence=scores, reverse=directions,
                   field={'path': str(field_path), 'sha256': sha(field_path)}, points_file={'path': str(point_path), 'sha256': sha(point_path)},
                   global_geometry=fit(points, tolerance), regions=[])
        for grid in (2, 3, 4):
            for y in range(grid):
                for x in range(grid):
                    indices = [i for i, p in enumerate(points) if x * b.width / grid <= p[2] < (x + 1) * b.width / grid and y * b.height / grid <= p[3] < (y + 1) * b.height / grid]
                    geometry = fit([points[i] for i in indices], tolerance)
                    row['regions'].append({'grid': grid, 'cell': [x, y], 'proposal_indices': indices, **geometry})
        row.update(status='complete_proposals_only', elapsed_seconds=time.monotonic() - started)
        save()
        print(name, len(points), 'global', len(row['global_geometry']['bidirectional_inlier_indices']), 'regional max', max(len(region['bidirectional_inlier_indices']) for region in row['regions']), flush=True)
        del warp, certainty
    verify()
    assert all(sha(path) == digest for path, digest in pins.items())
    report['status'] = 'complete_three_pair_proposal_diagnostic'
except Exception as error:
    report.update(status='failed', error=f'{type(error).__name__}: {error}')
    raise
finally:
    save()
