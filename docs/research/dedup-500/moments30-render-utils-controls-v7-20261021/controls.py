"""Original NumPy-compatible utility bodies, without model imports."""
import ast
import json
import pathlib
import sys
sys.path.insert(0, '.dedup-work/immich15-deps')
import numpy as np
source = ast.parse(pathlib.Path(sys.argv[1]).read_text())
names = {'normalize_0_1', 'float2uint8', 'crop_boundary'}
ns = {'np': np}
exec(compile(ast.Module(body=[n for n in source.body if isinstance(n, ast.FunctionDef) and n.name in names], type_ignores=[]), '<original-utils>', 'exec'), ns)
with np.errstate(invalid='ignore', divide='ignore'):
    assert np.isnan(ns['normalize_0_1'](np.ones((2, 2)))).all()
print(json.dumps({'name': 'constant-disparity-normalization-nan', 'passed': True}))
a = ns['float2uint8'](np.array([1.1, -0.1]))
assert a.tolist() == [24, 231]
print(json.dumps({'name': 'uint8-conversion-wraps-without-clipping', 'actual': a.tolist(), 'passed': True}))
x = np.ones((1, 3, 4, 6))
assert ns['crop_boundary'](x, 0).shape == x.shape
print(json.dumps({'name': 'zero-crop-keeps-image', 'passed': True}))
assert ns['crop_boundary'](x, .5).shape == (1, 3, 0, 0)
print(json.dumps({'name': 'half-crop-empty-image', 'passed': True}))
