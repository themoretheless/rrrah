"""Original DPT sizing/preparation classes without image codec or model imports."""
import ast
import json
import pathlib
import sys
from types import SimpleNamespace
sys.path.insert(0, '.dedup-work/immich15-deps')
import numpy as np
source = ast.parse(pathlib.Path(sys.argv[1]).read_text())
ns = {'np': np, 'cv2': SimpleNamespace(INTER_AREA=3)}
exec(compile(ast.Module(body=[n for n in source.body if isinstance(n, ast.ClassDef) and n.name in {'Resize', 'PrepareForNet'}], type_ignores=[]), '<original-dpt-transforms>', 'exec'), ns)
resize = ns['Resize'](384, 384, resize_target=None, keep_aspect_ratio=True, ensure_multiple_of=32, resize_method='minimal')
size = resize.get_size(640, 480)
assert size == (512, 384)
print(json.dumps({'name': 'normal-size-multiple32', 'actual': [int(x) for x in size], 'passed': True}))
size = resize.get_size(10000, 1)
assert size == (384, 0)
print(json.dumps({'name': 'extreme-aspect-rounds-height-to-zero', 'actual': [int(x) for x in size], 'passed': True}))
x = np.arange(24).reshape(2, 4, 3)
sample = ns['PrepareForNet']()({'image': x})
assert sample['image'].shape == (3, 2, 4) and sample['image'].dtype == np.float32 and sample['image'].flags.c_contiguous and np.array_equal(sample['image'], x.transpose(2, 0, 1))
print(json.dumps({'name': 'prepare-contiguous-chw-float32', 'passed': True}))
