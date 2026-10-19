"""Original validation/pair helpers and real NumPy retrieval metrics, no sklearn substitution."""
from pathlib import Path
from types import SimpleNamespace
from typing import Dict, List, Tuple
import ast
import hashlib
import importlib.util
import itertools
import json
import warnings
import numpy as np

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-evaluation-20261019'
W.mkdir(exist_ok=False)
names = ['imagededup/handlers/metrics/classification.py', 'tests/test_classification.py',
         'imagededup/evaluation/evaluation.py', 'imagededup/handlers/metrics/information_retrieval.py']
env = dict(Dict=Dict, List=List, Tuple=Tuple, itertools=itertools,
           logger=SimpleNamespace(info=lambda *args: None))
for relative, allowed in [(names[0], {'_get_unique_ordered_tuples', '_make_all_unique_possible_pairs',
                                     '_make_positive_duplicate_pairs', '_prepare_labels'}),
                          (names[2], {'_transpose_checker', '_check_map_correctness'})]:
    nodes = [n for n in ast.parse((S / relative).read_text()).body if isinstance(n, ast.FunctionDef) and n.name in allowed]
    exec(compile(ast.Module(body=nodes, type_ignores=[]), relative, 'exec'), env)
selected = ['test__get_unique_ordered_tuples', 'test__prepare_labels']
nodes = [n for n in ast.parse((S / names[1]).read_text()).body if isinstance(n, ast.FunctionDef) and n.name in selected]
exec(compile(ast.Module(body=nodes, type_ignores=[]), names[1], 'exec'), env)
for name in selected:
    env[name]()
edges = list(itertools.combinations('abcd', 2))
def mapping(mask):
    result = {k: [] for k in 'abcd'}
    for i, (a, b) in enumerate(edges):
        if mask & (1 << i):
            result[a].append(b)
            result[b].append(a)
    return result
checks = 0
for truth_mask in range(64):
    for retrieved_mask in range(64):
        truth, retrieved = mapping(truth_mask), mapping(retrieved_mask)
        env['_check_map_correctness'](truth, retrieved)
        pairs = env['_make_all_unique_possible_pairs'](truth)
        assert set(pairs) == set(edges)
        positives = env['_make_positive_duplicate_pairs'](truth, retrieved)
        actual = env['_prepare_labels'](pairs, *positives)
        expected = ([int(b in truth[a]) for a, b in pairs], [int(b in retrieved[a]) for a, b in pairs])
        assert actual == expected
        checks += 1
spec = importlib.util.spec_from_file_location('original_ir', S / names[3])
ir = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ir)
truth = {'a': ['b'], 'b': ['a']}
repeated = {'a': ['b', 'b'], 'b': ['a', 'a']}
env['_check_map_correctness'](truth, repeated)
repeated_ap = float(ir.mean_metric(truth, repeated, metric='map'))
assert repeated_ap == 2.0
partial_ndcg = float(ir.ndcg(['b', 'c'], ['b']))
assert partial_ndcg == 1.0
env['_check_map_correctness']({'a': ['a']}, {'a': ['a']})
with warnings.catch_warnings(record=True) as captured:
    warnings.simplefilter('always')
    empty = ir.get_all_metrics({}, {})
assert all(np.isnan(value) for value in empty.values())
result = dict(status='passed_original_selected_evaluation_controls',
    source_pins={n: hashlib.sha256((S / n).read_bytes()).hexdigest() for n in names},
    original_selected_tests_passed=selected, graph_pair_cases=checks,
    duplicate_entries_accepted=True, repeated_average_precision=repeated_ap,
    partial_coverage_ndcg=partial_ndcg, self_edges_accepted=True,
    empty_metrics={k: repr(float(v)) for k, v in empty.items()},
    empty_warnings=[str(w.message) for w in captured], numpy_version=np.__version__,
    findings=['Validator permits duplicate neighbors; valid symmetric repeated mapping yields mean average precision2.0.',
              'NDCG normalizes using retrieved relevance only: one of two relevant items alone scores1.0; no complete recall guarantee.',
              'Empty maps validate, but real NumPy mean metrics return NaN with warnings.',
              'Classification pair helper excludes self-pairs while validation accepts self-edges; metrics have differing domains.'],
    limits=['Original six validation/classification helper bodies AST extracted, imports/logger routed explicitly.',
            'Original whole information_retrieval module loaded unchanged with actual NumPy2.5.4.',
            '4096four-vertex graph-pair combinations and two original test bodies only; sklearn/classification score runtime and full evaluator suite not executed.',
            'No quality-corpus or model/runtime/platform qualification; no foreign production code adopted; pHash native work deferred.'])
(W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'graph_pair_cases': checks, 'repeated_AP': repeated_ap, 'empty': result['empty_metrics']}))
