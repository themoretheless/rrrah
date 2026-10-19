"""Primitive fixture semantics and two fully read notebooks; never execute notebook cells."""
from pathlib import Path
from types import SimpleNamespace
from typing import Dict
import ast
import hashlib
import importlib.util
import io
import itertools
import json
import pickle

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-notebook-fixtures-20261019'
W.mkdir(exist_ok=False)
class Primitive(pickle.Unpickler):
    def find_class(self, *args):
        raise ValueError('globals forbidden')
fixtures = {}
for p in sorted((S / 'tests/data/evaluation_files').glob('*.pkl')):
    value = Primitive(io.BytesIO(p.read_bytes())).load()
    assert isinstance(value, dict) and all(isinstance(k, str) and isinstance(v, list)
        and all(isinstance(n, str) for n in v) for k, v in value.items())
    fixtures[p.name] = dict(path=str(p.relative_to(S)), sha256=hashlib.sha256(p.read_bytes()).hexdigest(), value=value)
assert fixtures['ground_truth.pkl']['value'] == fixtures['all_correct_retrievals.pkl']['value']
evaluator = S / 'imagededup/evaluation/evaluation.py'
nodes = [n for n in ast.parse(evaluator.read_text()).body if isinstance(n, ast.FunctionDef)
         and n.name in ['_transpose_checker', '_check_map_correctness']]
env = dict(Dict=Dict, itertools=itertools, logger=SimpleNamespace(info=lambda *args: None))
exec(compile(ast.Module(body=nodes, type_ignores=[]), str(evaluator), 'exec'), env)
try:
    env['_check_map_correctness'](fixtures['ground_truth.pkl']['value'], fixtures['all_correct_retrievals.pkl']['value'])
except AssertionError as e:
    validation = {'exception': type(e).__name__, 'message': str(e)}
else:
    raise AssertionError('expected incomplete key domain rejected')
notebooks = {}
for name in ['use_custom_model.ipynb', 'Evaluation.ipynb']:
    p = S / 'examples' / name
    d = json.loads(p.read_text())
    notebooks[name] = dict(path=str(p.relative_to(S)), sha256=hashlib.sha256(p.read_bytes()).hexdigest(),
        cells=len(d['cells']), outputs=sum(len(c.get('outputs', [])) for c in d['cells']),
        attachments=sum(len(c.get('attachments', {})) for c in d['cells']))
    assert all('image/png' not in o.get('data', {}) for c in d['cells'] for o in c.get('outputs', []))
evaluation = json.loads((S / 'examples/Evaluation.ipynb').read_text())
truth_node = ast.parse(''.join(evaluation['cells'][6]['source'])).body[0]
truth = ast.literal_eval(truth_node.value)
retrieved = ast.literal_eval(''.join(evaluation['cells'][4]['outputs'][0]['data']['text/plain']))
env['_check_map_correctness'](truth, retrieved)
spec = importlib.util.spec_from_file_location('original_ir', S / 'imagededup/handlers/metrics/information_retrieval.py')
ir = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ir)
metrics = {k: float(v) for k, v in ir.get_all_metrics(truth, retrieved).items()}
assert metrics == {'map': 0.6, 'ndcg': 0.8, 'jaccard': 0.6}
result = dict(status='passed_primitive_fixture_and_notebook_IR_controls', fixtures=fixtures,
    notebooks=notebooks, original_public_map_validation=validation, reproduced_notebook_IR=metrics,
    findings=['Three lower-level IR pickle fixtures have values outside key domain; public evaluate validator rejects them. Passing direct IR tests do not prove public evaluate on those fixtures.',
              'Evaluation notebook saved outputs originate2019TensorFlow/Python3.6, not current pinned PyTorch model execution.',
              'Original IR module reproduces saved notebook MAP0.6/NDCG0.8/Jaccard0.6 from stored mappings; no inference rerun.',
              'Custom-model notebook contains no saved outputs; pretrained downloads/transformers install cells are unexecuted source examples.'],
    limits=['Full notebook cell source/text outputs read; no notebook cells, shell commands, downloads or models executed.',
            'Primitive-only pickle loading plus full value/schema inspection; not image/visual or production input qualification.',
            'Selected original map-validator bodies and entire IR module used; no whole-package/runtime/platform claim.',
            'No foreign code adopted; pHash native investigation deferred.'])
(W / 'result.json').write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps({'primitive_fixtures': len(fixtures), 'notebooks': len(notebooks), 'IR':metrics}))
