"""Original RAFT parser and sparse-resize bodies on small synthetic inputs."""
import ast
import json
import pathlib
import sys
import tempfile
import warnings
sys.path.insert(0, '.dedup-work/immich15-deps')
import numpy as np
warnings.filterwarnings('ignore', category=DeprecationWarning)
root = pathlib.Path(sys.argv[1])
source = ast.parse((root / 'core/utils/frame_utils.py').read_text())
ns = {'np': np, 'TAG_CHAR': np.array([202021.25], np.float32)}
exec(compile(ast.Module(body=[n for n in source.body if isinstance(n, ast.FunctionDef) and n.name in {'readFlow', 'writeFlow'}], type_ignores=[]), '<original-raft-flow>', 'exec'), ns)
with tempfile.TemporaryDirectory() as directory:
    path = pathlib.Path(directory) / 'test.flo'
    flow = np.arange(8, dtype=np.float32).reshape(2, 2, 2)
    ns['writeFlow'](str(path), flow)
    assert np.array_equal(ns['readFlow'](str(path)), flow)
    print(json.dumps({'name': 'valid-flow-roundtrip', 'passed': True}))
    with path.open('wb') as f:
        np.array([202021.25], np.float32).tofile(f)
        np.array([2, 2], np.int32).tofile(f)
        np.array([3, 7], np.float32).tofile(f)
    actual = ns['readFlow'](str(path))
    assert actual.reshape(-1).tolist() == [3, 7] * 4
    print(json.dumps({'name': 'truncated-payload-silently-repeated', 'actual': actual.tolist(), 'passed': True}))
    path.write_bytes(np.array([0], np.float32).tobytes())
    assert ns['readFlow'](str(path)) is None
    print(json.dumps({'name': 'invalid-magic-rejected', 'passed': True}))
source = ast.parse((root / 'core/utils/augmentor.py').read_text())
cls = next(n for n in source.body if isinstance(n, ast.ClassDef) and n.name == 'SparseFlowAugmentor')
method = next(n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == 'resize_sparse_flow_map')
exec(compile(ast.Module(body=[method], type_ignores=[]), '<original-sparse-resize>', 'exec'), ns)
flow, valid = ns['resize_sparse_flow_map'](None, np.ones((3, 3, 2)), np.ones((3, 3)), fx=1, fy=1)
assert valid.tolist() == [[0, 0, 0], [0, 1, 1], [0, 1, 1]]
print(json.dumps({'name': 'identity-sparse-resize-drops-top-left-borders', 'valid': valid.tolist(), 'passed': True}))
