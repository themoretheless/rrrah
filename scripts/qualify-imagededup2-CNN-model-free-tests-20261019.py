"""Selected original CNN tests with real cosine calculations, bypassing model initialization."""
from pathlib import Path, PurePath
from types import SimpleNamespace
from typing import Dict, List, Optional, Union, Tuple
from unittest.mock import Mock
import ast
import hashlib
import json
import sys
import warnings
import numpy as np
import pytest
from sklearn.metrics.pairwise import cosine_similarity

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-CNN-model-free-tests-v2-20261019'
W.mkdir(exist_ok=False)
logger = SimpleNamespace(info=lambda *args: None)
env = dict(PurePath=PurePath, Dict=Dict, List=List, Optional=Optional, Union=Union, Tuple=Tuple,
           np=np, sys=sys, warnings=warnings, cpu_count=lambda: 1,
           cosine_similarity=cosine_similarity, logger=logger)
retrieval = S / 'imagededup/handlers/search/retrieval.py'
nodes = [n for n in ast.parse(retrieval.read_text()).body if isinstance(n, ast.FunctionDef)
         and n.name in ['get_cosine_similarity', 'cosine_similarity_chunk']]
exec(compile(ast.Module(body=nodes, type_ignores=[]), str(retrieval), 'exec'), env)
source = S / 'imagededup/methods/cnn.py'
cls = next(n for n in ast.parse(source.read_text()).body if isinstance(n, ast.ClassDef))
cls.body = [n for n in cls.body if isinstance(n, ast.FunctionDef)
            and n.name in ['_check_threshold_bounds', '_find_duplicates_dict', 'find_duplicates']]
exec(compile(ast.Module(body=[cls], type_ignores=[]), str(source), 'exec'), env)
obj = env['CNN']()
obj.logger, obj.verbose = logger, False
tests = S / 'tests/test_cnn.py'
parsed = ast.parse(tests.read_text())
selected = ['test__check_threshold_bounds_input_not_float', 'test__check_threshold_bounds_input_out_of_range',
            'test__find_duplicates_dict_scores_false', 'test__find_duplicates_dict_scores_true',
            'test_find_duplicates_wrong_threhsold_input', 'test_find_duplicates_wrong_input']
nodes = [n for n in parsed.body if isinstance(n, ast.FunctionDef) and n.name in selected + ['data_encoding_map']]
env.update(pytest=pytest)
exec(compile(ast.Module(body=nodes, type_ignores=[]), str(tests), 'exec'), env)
for name in selected:
    args = [obj, Mock()] if name.endswith('scores_true') else [obj]
    env[name](*args)
inventory = [{'name':n.name,'line':n.lineno,
              'uses_mock_fixture':any('mock' in a.arg for a in n.args.args),
              'decorators':[ast.unparse(d) for d in n.decorator_list]}
             for n in parsed.body if isinstance(n, ast.FunctionDef) and n.name.startswith('test_')]
result = dict(status='passed_selected_original_CNN_model_free_tests',
    source_pins={str(p.relative_to(S)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [source,tests,retrieval]},
    original_tests_passed=selected, top_level_test_function_inventory=inventory,
    findings=['Selected six original test bodies pass with real sklearn cosine path on3vectors; no injected score matrix.',
              'Import-defaults test removes cached MobileNet files then instantiates model; must isolate TORCH_HOME before any future full-suite execution.',
              'Integration tests compare neighbor membership and score type, not numeric scores in expected_duplicates literals.',
              'No tests in this source cover NaN thresholds, all-invalid batches or empty-directory feature stacking.',
              'SVG expected in format integration uses previously identified JPEG bytes under.svg filename.'],
    limits=['Only three original CNN methods retained, model constructor excluded; namespace globals routed explicitly.',
            'Entire original two cosine function bodies retained; small direct sklearn branch only, no chunking/worker execution.',
            'Mock argument supplied only for original save-json-not-called assertion; no metric/score substitution.',
            'Not full CNN test target, torch/model/weight/decode/end-to-end/platform validation; no production adoption.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'original_tests_passed':len(selected),'source_test_functions':len(inventory)}))
