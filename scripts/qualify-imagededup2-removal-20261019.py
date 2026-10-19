"""Original selected utility bodies, exhaustive small symmetric graphs and negative controls."""
from pathlib import Path
from types import SimpleNamespace
from typing import Dict, List, Union
from pathlib import PurePath
import ast
import hashlib
import itertools
import json
import os
import tempfile
import numpy as np

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-removal-20261019'
W.mkdir(exist_ok=False)
names = ['get_files_to_remove', 'save_json', 'generate_files', 'generate_relative_names']
source = S / 'imagededup/utils/general_utils.py'
nodes = [n for n in ast.parse(source.read_text()).body if isinstance(n, ast.FunctionDef) and n.name in names]
assert len(nodes) == 4
env = dict(Dict=Dict, List=List, Union=Union, Path=Path, PurePath=PurePath, json=json,
           logger=SimpleNamespace(info=lambda *args: None))
exec(compile(ast.Module(body=nodes, type_ignores=[]), str(source), 'exec'), env)
tests = S / 'tests/test_general_utils.py'
test_nodes = [n for n in ast.parse(tests.read_text()).body if isinstance(n, ast.FunctionDef)]
testenv = dict(os=os, json=json, np=np, general_utils=SimpleNamespace(**{n: env[n] for n in names}))
exec(compile(ast.Module(body=test_nodes, type_ignores=[]), str(tests), 'exec'), testenv)
old_cwd = Path.cwd()
with tempfile.TemporaryDirectory(dir=W) as tmp:
    try:
        os.chdir(tmp)
        for n in test_nodes:
            testenv[n.name]()
    finally:
        os.chdir(old_cwd)
remove = env['get_files_to_remove']
vertices = tuple('abcde')
edges = list(itertools.combinations(vertices, 2))
queries = 0
for mask in range(1 << len(edges)):
    adjacency = {v: set() for v in vertices}
    for i, (a, b) in enumerate(edges):
        if mask & (1 << i):
            adjacency[a].add(b)
            adjacency[b].add(a)
    for order in itertools.permutations(vertices):
        removed = set(remove({v: sorted(adjacency[v]) for v in order}))
        kept = set(vertices) - removed
        assert all(not (adjacency[v] & kept) for v in kept)
        assert all(adjacency[v] & kept for v in removed)
        queries += 1
chain = {'a': ['b'], 'b': ['a', 'c'], 'c': ['b']}
chain_results = [{'order': list(order), 'kept': sorted(set(chain) - set(remove({v: chain[v] for v in order})))}
                 for order in itertools.permutations(chain)]
assert {len(x['kept']) for x in chain_results} == {1, 2}
asymmetric = {'a': ['b'], 'b': [], 'c': ['a']}
assert set(remove(asymmetric)) == {'a', 'b'}
assert remove({'a': ['a']}) == ['a']
with tempfile.TemporaryDirectory(dir=W) as tmp:
    root = Path(tmp)
    (root / '.hidden').mkdir()
    (root / '.hidden' / 'visible.jpg').write_bytes(b'synthetic')
    (root / '.hidden.jpg').write_bytes(b'synthetic')
    (root / 'visible.jpg').write_bytes(b'synthetic')
    discovered = sorted(str(p.relative_to(root)) for p in env['generate_files'](root, True))
    assert discovered == ['.hidden/visible.jpg', 'visible.jpg']
result = dict(status='passed_selected_utilities_and_graph_invariants',
              source_pins={str(p.relative_to(S)): hashlib.sha256(p.read_bytes()).hexdigest() for p in [source, tests]},
              original_tests_passed=[n.name for n in test_nodes], symmetric_graph_order_cases=queries,
              invariants=['survivors independent', 'each removed vertex adjacent to a survivor'],
              chain_order_counterexample=chain_results,
              asymmetric_control={'mapping': asymmetric, 'removed': ['a', 'b'], 'kept': ['c'],
                                  'finding': 'b has no direct edge to survivor c'},
              self_edge_control='single self-matching file selected for removal',
              hidden_parent_control=discovered,
              limits=['Original four AST-extracted function bodies; import routing and logger adapter explicit.',
                      'All 1024 simple undirected five-vertex graphs and all 120 orders; not arbitrary graph sizes.',
                      'Graph adjacency is similarity, not byte identity; no actual files deleted.',
                      'Three unchanged original test bodies only; no multiprocessing or full-package test.'])
(W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'tests': len(test_nodes), 'graph_order_cases': queries, 'status': result['status']}))
