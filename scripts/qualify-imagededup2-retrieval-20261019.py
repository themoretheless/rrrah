"""Original HashEval body with explicit serial dispatcher; no native/worker qualification."""
from pathlib import Path
import ast
import hashlib
import importlib.util
import json
import sys
from typing import Callable, Dict, Union

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-retrieval-20261019'
W.mkdir(exist_ok=False)
pins = {}
def load(relative, name):
    path = S / relative
    pins[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module
brute = load('imagededup/handlers/search/brute_force.py', 'original_brute').BruteForce
tree = load('imagededup/handlers/search/bktree.py', 'original_tree').BKTree
class NativeNotQualified:
    def __init__(self, *args):
        raise RuntimeError('native backend not qualified by this harness')
class Logger:
    def info(self, *args):
        pass
def serial(function, data, verbose, num_workers):
    return list(map(function, data))
path = S / 'imagededup/handlers/search/retrieval.py'
pins[str(path.relative_to(S))] = hashlib.sha256(path.read_bytes()).hexdigest()
node = next(n for n in ast.parse(path.read_text()).body if isinstance(n, ast.ClassDef))
env = dict(Callable=Callable, Dict=Dict, Union=Union, sys=sys, cpu_count=lambda: 1,
           BKTree=tree, BruteForce=brute, BruteForceCython=NativeNotQualified,
           parallelise=serial, logger=Logger())
exec(compile(ast.Module(body=[node], type_ignores=[]), str(path), 'exec'), env)
HashEval = env['HashEval']
distance = lambda a, b: (int(a, 16) ^ int(b, 16)).bit_count()
tests = S / 'tests/test_retrieval.py'
pins[str(tests.relative_to(S))] = hashlib.sha256(tests.read_bytes()).hexdigest()
selected = ['test_max_hamming_threshold_not_violated', 'test_results_sorted_in_ascending_distance_order']
nodes = [n for n in ast.parse(tests.read_text()).body if isinstance(n, ast.FunctionDef) and n.name in selected]
tenv = dict(HashEval=HashEval, HAMMING_DISTANCE_FUNCTION=distance)
exec(compile(ast.Module(body=nodes, type_ignores=[]), str(tests), 'exec'), tenv)
for name in selected:
    tenv[name]()
db = {f'file-{i}': f'{i:016x}' for i in range(256)}
checks = 0
for method in ['brute_force', 'bktree']:
    for radius in range(9):
        actual = HashEval(db, db, distance, verbose=False, threshold=radius,
                          search_method=method, num_dist_workers=0).retrieve_results(scores=True)
        for key, value in db.items():
            expected = sorted([(k, distance(value, v)) for k, v in db.items()
                               if k != key and distance(value, v) <= radius], key=lambda x: x[1])
            # Tie order is backend traversal-dependent; compare exact membership plus monotone distances.
            assert set(actual[key]) == set(expected)
            assert [x[1] for x in actual[key]] == sorted(x[1] for x in actual[key])
            checks += 1
same_key = HashEval({'same.jpg': '0'}, {'same.jpg': '0'}, distance,
                   search_method='brute_force', num_dist_workers=0).retrieve_results()
assert same_key == {'same.jpg': []}
errors = {}
for method in ['brute_force', 'bktree', 'typo']:
    try:
        result = HashEval({}, {}, distance, search_method=method, num_dist_workers=0).retrieve_results()
        errors[method] = {'result': result}
    except Exception as e:
        errors[method] = {'exception': type(e).__name__, 'message': str(e)}
assert errors['brute_force'] == {'result': {}}
assert errors['bktree']['exception'] == 'IndexError'
assert errors['typo']['exception'] == 'RuntimeError'
result = dict(status='passed_selected_original_retrieval_body_with_explicit_adapters', source_pins=pins,
    original_tests_passed=selected, oracle_queries=checks, backends=['brute_force', 'bktree'],
    oracle='all256eightbit fixed64bit hashes,9radii, self-free exact membership and monotone distance order',
    empty_and_unknown_backend_controls=errors, same_key_control=same_key,
    findings=['Self suppression compares filename keys only; same key across separate namespaces is excluded.',
              'Empty database differs between Python brute force and BKTree.',
              'Unknown backend string falls through to native constructor; harness sentinel records dispatch only.'],
    limits=['Original entire HashEval class AST unchanged; original Python backend modules loaded unchanged.',
            'Serial parallelise adapter, logger adapter, cpu_count1 and independent integer XOR metric explicit.',
            'Not multiprocessing, native Cython, original Hashing distance, cosine/sklearn, package or full test target qualification.',
            'Tie ordering intentionally not treated as stable backend contract; no production adoption or speedup claim.'])
(W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'tests': len(selected), 'oracle_queries': checks, 'status': result['status']}))
