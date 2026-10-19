"""Original AHash/DHash and public array encoding path with actual NumPy/Pillow."""
from pathlib import Path, PurePath
from types import SimpleNamespace
from typing import Dict, List, Optional
import ast
import hashlib
import json
import os
import sys
import warnings
import numpy as np

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-hash-collisions-20261019'
W.mkdir(exist_ok=False)
logger = SimpleNamespace(info=lambda *args: None, warning=lambda *args: None)
image_path = S / 'imagededup/utils/image_utils.py'
image_ast = ast.parse(image_path.read_text())
image_ast.body = [n for n in image_ast.body if not (isinstance(n, ast.ImportFrom) and n.module == 'imagededup.utils.logger')]
image_env = dict(return_logger=lambda *args: logger)
exec(compile(image_ast, str(image_path), 'exec'), image_env)
path = S / 'imagededup/methods/hashing.py'
nodes = [n for n in ast.parse(path.read_text()).body if isinstance(n, ast.ClassDef) and n.name in ['Hashing', 'AHash', 'DHash']]
env = dict(os=os, sys=sys, warnings=warnings, Path=Path, PurePath=PurePath, Dict=Dict,
           List=List, Optional=Optional, np=np, cpu_count=lambda: 1, logger=logger,
           **{n: image_env[n] for n in ['load_image', 'preprocess_image', 'check_image_array_hash']})
exec(compile(ast.Module(body=nodes, type_ignores=[]), str(path), 'exec'), env)
rows = []
for name, expected in [('AHash', 'ffffffffffffffff'), ('DHash', '0000000000000000')]:
    hasher = env[name](verbose=False)
    hashes = [hasher.encode_image(image_array=np.full((8, 9, 3), level, dtype=np.uint8)) for level in range(256)]
    assert set(hashes) == {expected}
    assert hasher.hamming_distance(hashes[0], hashes[-1]) == 0
    rows.append(dict(method=name, distinct_constant_RGB_arrays=256, unique_hashes=1,
                     shared_hash=expected, black_white_hamming_distance=0))
thresholds = []
for value in [False, True, np.int64(3), 0, 64, 65, -1, 1.0]:
    try:
        env['Hashing']._check_hamming_distance_bounds(value)
        result = 'accepted'
    except Exception as e:
        result = type(e).__name__
    thresholds.append({'value': repr(value), 'type': type(value).__name__, 'result': result})
assert thresholds[0]['result'] == thresholds[1]['result'] == 'accepted'
assert thresholds[2]['result'] == 'TypeError'
try:
    env['AHash']().find_duplicates(encoding_map={})
except ValueError as e:
    empty = {'exception': type(e).__name__, 'message': str(e)}
else:
    raise AssertionError('expected rejected empty map')
result = dict(status='passed_original_selected_hash_encoding_characterization',
    source_pins={str(p.relative_to(S)): hashlib.sha256(p.read_bytes()).hexdigest() for p in [path, image_path]},
    collision_controls=rows, threshold_controls=thresholds, empty_map_control=empty,
    findings=['AHash and DHash zero-distance matches do not establish exact pixel or byte identity: black and white arrays collide.',
              'Each method collapses all256distinct constant RGB levels into one64bit hash through original public encode_image preprocessing.',
              'Hash threshold validator accepts booleans as int, rejects NumPy int64; empty mapping rejected by public truthiness check.',
              'BKTree source fully read; previously qualified empty-root and recursive equal-hash chain limitations retained.'],
    limits=['Original Hashing/AHash/DHash class bodies AST extracted unchanged; entire original image-utils body with logger import routing.',
            'Actual NumPy2.5.4/Pillow12.3.0; no pywt/scipy/Cython/native backend or original hashing test-suite execution.',
            'Synthetic array path only; no files/corpus/quality/performance/platform claims or production adoption.',
            'PHash/WHash source read, not executed; pHash native memory-error investigation remains deferred.'])
(W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'methods': len(rows), 'distinct_arrays_per_method': 256, 'zero_distance_black_white': True}))
