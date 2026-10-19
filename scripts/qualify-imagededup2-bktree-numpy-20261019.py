"""Run original BKTree test bodies with original distance and real NumPy; render pinned SVG."""
from pathlib import Path
from types import SimpleNamespace
import ast
import hashlib
import importlib.util
import json
import numpy as np
from PyQt5.QtCore import QByteArray
from PyQt5.QtGui import QImage, QPainter, QColor
from PyQt5.QtSvg import QSvgRenderer

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-bktree-numpy-20261019'
W.mkdir(exist_ok=False)
path = S / 'imagededup/handlers/search/bktree.py'
spec = importlib.util.spec_from_file_location('original_bktree', path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
hash_path = S / 'imagededup/methods/hashing.py'
cls = next(n for n in ast.parse(hash_path.read_text()).body if isinstance(n, ast.ClassDef) and n.name == 'Hashing')
method = next(n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == 'hamming_distance')
method.decorator_list = []
env = {'np': np}
exec(compile(ast.Module(body=[method], type_ignores=[]), str(hash_path), 'exec'), env)
tests = S / 'tests/test_bktree.py'
parsed = ast.parse(tests.read_text())
parsed.body = [n for n in parsed.body if not (isinstance(n, ast.ImportFrom) and n.module.startswith('imagededup'))]
testenv = dict(Hashing=SimpleNamespace(hamming_distance=env['hamming_distance']),
               BKTree=module.BKTree, BkTreeNode=module.BkTreeNode)
exec(compile(parsed, str(tests), 'exec'), testenv)
passed = sorted(n for n in testenv if n.startswith('test_'))
for name in passed:
    testenv[name]()
svg = S / 'mkdocs/docs/img/logo.svg'
renderer = QSvgRenderer(QByteArray(svg.read_bytes()))
assert renderer.isValid()
image = QImage(320, 320, QImage.Format_ARGB32)
image.fill(QColor('#333333'))
painter = QPainter(image)
renderer.render(painter)
painter.end()
assert image.save(str(W / 'logo.png'))
result = dict(status='passed_original_BKTree_test_bodies_real_numpy_and_SVG_render',
    source_pins={str(p.relative_to(S)): hashlib.sha256(p.read_bytes()).hexdigest() for p in [path, hash_path, tests, svg]},
    original_tests_passed=passed, numpy_version=np.__version__, SVG_valid=True,
    findings=['13unchanged original test bodies now run using original hamming_distance body with real NumPy, superseding sum-adapter limitation for these tests.',
              'Existing tests use small nonempty dictionary, no empty/equal-hash long-chain or oversized-hash guard.',
              'SVG logo is white overlapping square outlines; rendered on dark background for inspection.'],
    limits=['Whole original BKTree module unchanged; original Hashing method extracted by AST, staticmethod decorator removed, test package imports routed.',
            'These13tests overlap earlier selected qualification; not13additional distinct upstream tests.',
            'SVG rendered by QtSvg only; visual inspection recorded separately, not browser/platform fidelity proof.',
            'No production code adopted; native pHash investigation deferred.'])
(W / 'result.json').write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps({'tests':len(passed), 'SVG_valid':True}))
