"""Run original three metric/evaluator targets with real sklearn and inspected primitive pickles."""
from pathlib import Path
from types import ModuleType
import hashlib
import importlib.util
import json
import pickletools
import sys
import xml.etree.ElementTree as ET
import numpy as np
import sklearn
import pytest

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-metric-suites-20261019'
W.mkdir(exist_ok=False)
pickle_inventory = []
for path in sorted((S / 'tests/data/evaluation_files').glob('*.pkl')):
    operations = list(pickletools.genops(path.read_bytes()))
    forbidden = {'GLOBAL', 'STACK_GLOBAL', 'REDUCE', 'BUILD', 'INST', 'OBJ', 'NEWOBJ',
                 'NEWOBJ_EX', 'EXT1', 'EXT2', 'EXT4', 'PERSID', 'BINPERSID'}
    assert not (forbidden & {op.name for op, arg, pos in operations})
    pickle_inventory.append(dict(path=str(path.relative_to(S)),
        sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
        opcodes=sorted({op.name for op, arg, pos in operations})))
for name, relative in [('imagededup', 'imagededup'), ('imagededup.utils', 'imagededup/utils'),
                       ('imagededup.handlers', 'imagededup/handlers'),
                       ('imagededup.handlers.metrics', 'imagededup/handlers/metrics'),
                       ('imagededup.evaluation', 'imagededup/evaluation')]:
    package = ModuleType(name)
    package.__path__ = [str(S / relative)]
    sys.modules[name] = package
    if '.' in name:
        parent, child = name.rsplit('.', 1)
        setattr(sys.modules[parent], child, package)
modules = ['imagededup.utils.logger', 'imagededup.handlers.metrics.information_retrieval',
           'imagededup.handlers.metrics.classification', 'imagededup.evaluation.evaluation']
pins = {}
for name in modules:
    path = S / (name.replace('.', '/') + '.py')
    pins[str(path.relative_to(S))] = hashlib.sha256(path.read_bytes()).hexdigest()
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    parent, child = name.rsplit('.', 1)
    setattr(sys.modules[parent], child, module)
targets = [S / 'tests' / n for n in ['test_classification.py', 'test_information_retrieval.py', 'test_evaluator.py']]
for path in targets:
    pins[str(path.relative_to(S))] = hashlib.sha256(path.read_bytes()).hexdigest()
code = pytest.main(['-c', '/dev/null', '--confcutdir=' + str(S / 'tests'), '-q',
                   '-p', 'no:cacheprovider', '--junitxml=' + str(W / 'junit.xml'), *map(str, targets)])
suite = ET.parse(W / 'junit.xml').getroot().find('testsuite')
result = dict(status='original_metric_suites_passed' if code == 0 else 'original_metric_suites_failed',
    pytest_exit_code=int(code), source_pins=pins, inspected_pickle_fixtures=pickle_inventory,
    suite={k: suite.attrib[k] for k in ['tests', 'failures', 'errors', 'skipped']},
    environment={'numpy': np.__version__, 'scikit-learn': sklearn.__version__, 'pytest': pytest.__version__},
    limits=['Original four complete modules and three complete test targets unchanged; namespace package routing bypasses unrelated model/plot imports.',
            'Real sklearn/NumPy, no score-function substitution; pickle opcodes inspected before original tests load primitive fixtures.',
            'Not entire package/CNN/hash/worker/decode/platform or quality-corpus validation.',
            'Original tests do not negate separately reproduced duplicate-entry AP2.0/empty-NaN controls.',
            'No production code adoption; pHash native investigation remains deferred.'])
(W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result['suite']))
raise SystemExit(int(code))
