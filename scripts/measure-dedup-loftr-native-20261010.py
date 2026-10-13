"""Native Rust/tch proposals, diagnostic OpenCV fit, native original-pixel confirmation.

Python performs fixture preparation/postprocessing only. This is not an integrated
native library path and does not qualify inference memory or cancellation.
"""
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import time

import cv2
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
D = ROOT / 'docs/research'
OUT = D / 'dedup-loftr-native-20261010.json'
BINARY = Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-target-20261010/release/dedup-loftr-native-probe')
WEIGHTS = T / 'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
NATIVE = T / 'supplied-anchor-files-probe-qualified-20261010'
MANIFEST = T / 'native-loftr-inputs-20261010/manifest.json'
assert not OUT.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
rows = json.loads(MANIFEST.read_text())
pin_paths = [BINARY, WEIGHTS, NATIVE, MANIFEST, Path(__file__),
             ROOT / 'scripts/dedup-loftr-native-probe/src/main.rs',
             ROOT / 'scripts/dedup-loftr-native-probe/Cargo.toml',
             ROOT / 'scripts/dedup-loftr-native-probe/Cargo.lock']
for row in rows:
    for inp in row['inputs']:
        assert sha(inp['path']) == inp['sha256']
        pin_paths.extend([Path(inp['path']), Path(inp['source'])])
pins = {str(p): sha(p) for p in pin_paths}
report = dict(status='running', pins=pins, results=[], scope=__doc__)
def save():
    temp = OUT.with_suffix('.json.tmp')
    temp.write_text(json.dumps(report, indent=2) + '\n')
    temp.replace(OUT)
save()
cv2.setNumThreads(1)
try:
    for index, row in enumerate(rows):
        started = time.monotonic()
        args = [str(BINARY), str(WEIGHTS)]
        for inp in row['inputs']:
            args += [inp['path'], str(inp['width']), str(inp['height'])]
        env = dict(os.environ)
        env['DYLD_LIBRARY_PATH'] = '/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'
        proc = subprocess.run(args, capture_output=True, text=True, env=env)
        result = dict(side=row['side'], clahe=row['clahe'], inference_returncode=proc.returncode, inference_stderr=proc.stderr)
        report['results'].append(result)
        save()
        if proc.returncode:
            raise RuntimeError('native inference failed')
        raw = json.loads(proc.stdout)
        artifact = T / f'native-loftr-inputs-20261010/{index}-native-proposals.json'
        assert not artifact.exists()
        artifact.write_text(proc.stdout)
        pins[str(artifact)] = sha(artifact)
        coordinates, shapes = [], []
        for name, inp in zip(('keypoints0', 'keypoints1'), row['inputs']):
            image = cv2.imread(inp['source'], cv2.IMREAD_GRAYSCALE)
            shapes.append(image.shape)
            scale = np.array([image.shape[1] / inp['width'], image.shape[0] / inp['height']])
            coordinates.append((np.array(raw[name]) + .5) * scale - .5)
        points = []
        for j in np.argsort(-np.array(raw['confidence']), kind='stable'):
            p, q = coordinates[0][j], coordinates[1][j]
            if not all(0 <= v[0] < shape[1] and 0 <= v[1] < shape[0] for v, shape in zip((p,q), shapes)):
                continue
            if all(math.dist(p,x[0]) > 2 and math.dist(q,x[1]) > 2 for x in points):
                points.append([p.tolist(), q.tolist()])
        result.update(raw_matches=len(raw['confidence']), distinct_points=len(points))
        if len(points) < 4:
            result['status'] = 'no_geometry'
        else:
            cv2.setRNGSeed(173)
            matrix, _ = cv2.findHomography(np.float64([p[0] for p in points]), np.float64([p[1] for p in points]), cv2.RANSAC, 2., maxIters=20000, confidence=.999)
            if matrix is None:
                result['status'] = 'no_geometry'
            else:
                model = artifact.with_suffix('.model.txt')
                matches = artifact.with_suffix('.points.txt')
                model.write_text(' '.join(map(str,matrix.flatten())) + '\n')
                matches.write_text(''.join(' '.join(map(str, [*p,*q]))+'\n' for p,q in points))
                pins[str(model)] = sha(model)
                pins[str(matches)] = sha(matches)
                tolerance = 2 * math.hypot(*shapes[0]) / math.hypot(*shapes[1])
                confirmation = subprocess.run([str(NATIVE), *[x['source'] for x in row['inputs']], str(model), str(matches), str(tolerance)], capture_output=True, text=True)
                result.update(confirmation_returncode=confirmation.returncode, confirmation_stderr=confirmation.stderr, confirmation_stdout=confirmation.stdout)
                if confirmation.returncode:
                    raise RuntimeError('native confirmation failed')
                result['evidence'] = json.loads(confirmation.stdout)
        result['elapsed_seconds'] = time.monotonic() - started
        save()
        print(row['side'], row['clahe'], result.get('evidence', result.get('status')), flush=True)
    assert all(sha(p) == h for p,h in pins.items())
    report['status'] = 'complete_native_inference_diagnostic'
except Exception as error:
    report.update(status='failed_native_inference_diagnostic', error=str(error))
    raise
finally:
    save()
