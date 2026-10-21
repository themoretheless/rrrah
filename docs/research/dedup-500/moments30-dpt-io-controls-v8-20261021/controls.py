"""Original DPT PFM/depth function bodies with intercepted PNG writer."""
import ast
import json
import pathlib
import re
import sys
import tempfile
from types import SimpleNamespace
sys.path.insert(0, '.dedup-work/immich15-deps')
import numpy as np
names = {'read_pfm', 'write_pfm', 'write_depth'}
source = ast.parse(pathlib.Path(sys.argv[1]).read_text())
outputs = []
ns = {'np': np, 'sys': sys, 're': re, 'cv2': SimpleNamespace(IMWRITE_PNG_COMPRESSION=16, imwrite=lambda path, image, params: outputs.append((path, image.copy())))}
exec(compile(ast.Module(body=[n for n in source.body if isinstance(n, ast.FunctionDef) and n.name in names], type_ignores=[]), '<original-dpt-io>', 'exec'), ns)
with tempfile.TemporaryDirectory() as directory:
    image = np.arange(6, dtype=np.float32).reshape(2, 3)
    path = str(pathlib.Path(directory) / 'gray.pfm')
    ns['write_pfm'](path, image)
    actual, scale = ns['read_pfm'](path)
    assert np.array_equal(actual, image) and scale == 1
    print(json.dumps({'name': 'grayscale-pfm-roundtrip', 'passed': True}))
    try:
        ns['write_pfm'](str(pathlib.Path(directory) / 'color.pfm'), np.zeros((2, 3, 3), dtype=np.float32))
    except TypeError:
        print(json.dumps({'name': 'color-pfm-string-to-binary-typeerror', 'passed': True}))
    else:
        raise AssertionError('expected unencoded color header TypeError')
    stem = str(pathlib.Path(directory) / 'photo')
    ns['write_depth'](stem, np.ones((2, 3)), bits=2)
    assert outputs[0][0] == stem + '.png' and outputs[0][1].dtype == np.uint16 and not outputs[0][1].any()
    print(json.dumps({'name': 'constant-depth-zero-png-extension', 'passed': True}))
