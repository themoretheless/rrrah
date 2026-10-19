"""Original selected CNN method bodies; injected score matrices, no neural model execution."""
from pathlib import Path, PurePath
from types import SimpleNamespace
from typing import Dict, List, Optional
import ast
import hashlib
import json
import numpy as np

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-cnn-boundaries-v2-20261019'
W.mkdir(exist_ok=False)
path = S / 'imagededup/methods/cnn.py'
cls = next(n for n in ast.parse(path.read_text()).body if isinstance(n, ast.ClassDef))
selected = ['_check_threshold_bounds', '_find_duplicates_dict', 'find_duplicates_to_remove']
nodes = [n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name in selected]
for node in nodes:
    node.decorator_list = []
matrix = np.eye(2)
def supplied_scores(*args, **kwargs):
    return matrix.copy()
env = dict(Dict=Dict, List=List, Optional=Optional, PurePath=PurePath, np=np,
           cpu_count=lambda: 1, get_cosine_similarity=supplied_scores,
           get_files_to_remove=lambda x: [], save_json=lambda *args, **kwargs: None)
exec(compile(ast.Module(body=nodes, type_ignores=[]), str(path), 'exec'), env)
thresholds = []
for value in [-1.0, 0.0, 1.0, float('nan'), float('inf'), -float('inf'), -1.01, 1.01, 0, True]:
    try:
        env['_check_threshold_bounds'](value)
        result = 'accepted'
    except Exception as e:
        result = type(e).__name__
    thresholds.append({'value': repr(value), 'result': result})
assert thresholds[3]['result'] == 'accepted'
obj = SimpleNamespace(logger=SimpleNamespace(info=lambda *args: None), verbose=False)
precision_controls = []
for dtype in ['float64', 'float16']:
    matrix = np.array([[1.0, 0.89999], [0.89999, 1.0]], dtype=dtype)
    results = env['_find_duplicates_dict'](obj, {'a': [1, 0], 'b': [0, 1]}, 0.9, False, num_sim_workers=0)
    precision_controls.append({'dtype': dtype, 'off_diagonal': float(matrix[0, 1]), 'results': results})
assert precision_controls[0]['results'] == {'a': [], 'b': []}
assert precision_controls[1]['results'] == {'a': ['b'], 'b': ['a']}
matrix = np.eye(2)
nan_result = env['_find_duplicates_dict'](obj, {'a': [1, 0], 'b': [0, 1]}, float('nan'), False)
assert nan_result == {'a': [], 'b': []}
try:
    env['find_duplicates_to_remove'](obj)
except UnboundLocalError as e:
    no_input = {'exception': type(e).__name__, 'message': str(e)}
else:
    raise AssertionError('expected original unbound-local failure')
result = dict(status='passed_selected_CNN_boundary_characterization',
    source_pin={'path': str(path.relative_to(S)), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()},
    threshold_controls=thresholds, injected_score_precision_controls=precision_controls,
    nan_score_result=nan_result, no_input_removal=no_input,
    findings=['NaN passes threshold validation, then all comparisons are false, returning empty matches.',
              'Injected score0.89999 is below0.9 but accepted in float16 path with NumPy2.5.4 scalar casting; actual retrieval source uses float16 above row threshold.',
              'find_duplicates_to_remove without directory/encodings reaches uninitialized duplicates local.',
              'Self removal uses diagonal2sentinel; returned neighbors preserve insertion order rather than sorting scores.'],
    limits=['Three original AST-extracted method bodies; staticmethod decorator removed only.',
            'Cosine callback supplies matrices explicitly: not real sklearn precision or model/embedding/end-to-end qualification.',
            'No original CNN tests, torch/torchvision, weights, multiprocessing, GPU, package execution or production adoption.',
            'Full cnn.py source now read across two increments; pHash native investigation remains deferred.'])
(W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'threshold_controls': len(thresholds), 'precision_controls': len(precision_controls), 'no_input': no_input['exception']}))
