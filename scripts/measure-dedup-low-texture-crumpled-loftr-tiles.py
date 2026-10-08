"""Bounded learned proposal reference; never a duplicate admission decision."""
import hashlib, importlib, json, math, os, time
from pathlib import Path
import cv2
import numpy as np
import torch
import kornia

ROOT = Path(__file__).resolve().parents[1]
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT = ROOT / 'docs/research/dedup-low-texture-crumpled-loftr-tiles.json'
assert not OUT.exists()
source = T / 'original-resolution-negative-200201/200300.jpg'
target = T / 'original-resolution-strong-all/200302.jpg'
module = importlib.import_module('kornia.feature.loftr.loftr')
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins = {str(p): sha(p) for p in (source, target, Path(__file__), Path(module.__file__))}
rows = []
report = dict(status='running', pins=pins, rows=rows,
              versions=dict(torch=torch.__version__, kornia=kornia.__version__,
                            numpy=np.__version__, opencv=cv2.__version__),
              scope='Learned proposals only; no native full-domain geometry, pixel confirmation or precision proof. Fixed nonoverlapping2x2target crops condition inference; full source maxside640. All distinct original-coordinate proposals retained per tile before fitting. No conforming union mesh or acceptance threshold changes.')
def save():
    temporary = OUT.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    os.replace(temporary, OUT)
save()
try:
    torch.set_num_threads(2)
    torch.manual_seed(173)
    cv2.setNumThreads(1)
    cache = T / 'learned-matching-models'
    cache.mkdir(exist_ok=True)
    url = module.urls['outdoor'].replace('http://', 'https://', 1)
    weights = torch.hub.load_state_dict_from_url(url, model_dir=str(cache),
                                                map_location='cpu', weights_only=True)
    checkpoint = cache / url.rsplit('/', 1)[-1]
    report['weights'] = dict(url=url, path=str(checkpoint), sha256=sha(checkpoint))
    matcher = kornia.feature.LoFTR(pretrained=None).eval()
    matcher.load_state_dict(weights['state_dict'])
    a, b = [cv2.imread(str(p), cv2.IMREAD_GRAYSCALE) for p in (source, target)]
    assert a is not None and b is not None
    tolerance = 2 * math.hypot(*a.shape) / math.hypot(*b.shape)
    report['source_tolerance'] = tolerance
    assert b.shape == (600, 800)
    for tile_index in range(4):
        side = 640
        offset = np.array([(tile_index % 2) * 400, (tile_index // 2) * 300])
        tile = b[offset[1]:offset[1] + 300, offset[0]:offset[0] + 400]
        for enhanced in (False, True):
            started = time.monotonic()
            images, scales = [], []
            for original in (a, tile):
                ratio = min(1., side / max(original.shape))
                width = max(8, int(original.shape[1] * ratio) // 8 * 8)
                height = max(8, int(original.shape[0] * ratio) // 8 * 8)
                resized = cv2.resize(original, (width, height), interpolation=cv2.INTER_AREA)
                if enhanced:
                    resized = cv2.createCLAHE(clipLimit=2., tileGridSize=(8, 8)).apply(resized)
                images.append(torch.from_numpy(resized.copy()).float()[None, None] / 255.)
                scales.append(np.array([original.shape[1] / width, original.shape[0] / height]))
            with torch.inference_mode():
                found = matcher(dict(image0=images[0], image1=images[1]))
            left = (found['keypoints0'].numpy() + .5) * scales[0] - .5
            right = (found['keypoints1'].numpy() + .5) * scales[1] - .5 + offset
            confidence = found['confidence'].numpy()
            assert len(left) <= 5000, 'Proposal cap exceeded; do not truncate'
            points, scores = [], []
            for index in np.argsort(-confidence, kind='stable'):
                p, q = left[index], right[index]
                if not (0 <= p[0] < a.shape[1] and 0 <= p[1] < a.shape[0]
                        and 0 <= q[0] < b.shape[1] and 0 <= q[1] < b.shape[0]):
                    continue
                if all(math.dist(p, x[0]) > 2 and math.dist(q, x[1]) > 2 for x in points):
                    points.append([p.tolist(), q.tolist()])
                    scores.append(float(confidence[index]))
            row = dict(max_side=side, tile_index=tile_index, target_rectangle=[int(offset[0]), int(offset[1]), 400, 300], clahe=enhanced, raw_matches=len(left),
                       distinct_points=points, confidence=scores, matrix=None,
                       bidirectional_inlier_indices=[])
            if len(points) >= 4:
                cv2.setRNGSeed(173)
                H, _ = cv2.findHomography(np.float64([p[0] for p in points]),
                                         np.float64([p[1] for p in points]), cv2.RANSAC,
                                         2., maxIters=20000, confidence=.999)
                if H is not None and np.isfinite(H).all() and abs(np.linalg.det(H)) > 1e-15:
                    inverse = np.linalg.inv(H)
                    row['matrix'] = H.tolist()
                    def apply(matrix, point):
                        v = matrix @ np.array([*point, 1.])
                        return v[:2] / v[2]
                    for i, (p, q) in enumerate(points):
                        if np.linalg.norm(apply(H, p) - q) <= 2 and np.linalg.norm(apply(inverse, q) - p) <= tolerance:
                            row['bidirectional_inlier_indices'].append(i)
            row['elapsed_seconds'] = time.monotonic() - started
            rows.append(row)
            save()
            print(tile_index, side, enhanced, len(points), len(row['bidirectional_inlier_indices']), flush=True)
    assert all(sha(p) == digest for p, digest in pins.items())
    report['status'] = 'complete_proposal_diagnostic'
except Exception as error:
    report['status'] = 'failed_proposal_diagnostic'
    report['error'] = f'{type(error).__name__}: {error}'
    raise
finally:
    save()
