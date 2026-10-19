"""Original twelve shared AHash/DHash test cases using qualified real-image encoding setup."""
from pathlib import Path
import ast
import hashlib
import json
from PIL import Image
import numpy as np

R = Path(__file__).resolve().parents[1]
support = R / 'scripts/qualify-imagededup2-hash-collisions-20261019.py'
# Reuse only its setup before the first control loop, with a fresh owned work directory.
prefix = support.read_text().split('rows = []\n')[0]
prefix = prefix.replace("imagededup2-hash-collisions-20261019'", "imagededup2-selected-hash-tests-20261019'")
setup = {'__file__': str(support)}
exec(compile(prefix, str(support), 'exec'), setup)
S, W, classes = setup['S'], setup['W'], setup['env']
tests = S / 'tests/test_hashing.py'
node = next(n for n in ast.parse(tests.read_text()).body if isinstance(n, ast.ClassDef) and n.name == 'TestCommon')
node.decorator_list = []
tenv = dict(np=np, Image=Image, Hashing=classes['Hashing'], p=tests,
    PATH_SINGLE_IMAGE=S/'tests/data/mixed_images/ukbench00120.jpg',
    PATH_SINGLE_IMAGE_RESIZED=S/'tests/data/mixed_images/ukbench00120_resize.jpg',
    PATH_SINGLE_GRAY_IMAGE=S/'tests/data/ukbench00120_gray.jpg')
exec(compile(ast.Module(body=[node], type_ignores=[]), str(tests), 'exec'), tenv)
case = tenv['TestCommon']()
results = []
for method in ['AHash', 'DHash']:
    hasher = classes[method](verbose=False)
    for test in [n.name for n in node.body if isinstance(n, ast.FunctionDef)]:
        try:
            getattr(case, test)(hasher.encode_image)
            result = {'status': 'passed'}
        except Exception as e:
            result = {'status': 'failed', 'exception': type(e).__name__, 'message': str(e)}
        results.append(dict(method=method, original_test=test, **result))
result = dict(status='selected_original_hash_cases_characterized', cases=results,
    passed=sum(r['status']=='passed' for r in results), failed=sum(r['status']=='failed' for r in results),
    test_source_sha256=hashlib.sha256(tests.read_bytes()).hexdigest(),
    setup_script_sha256=hashlib.sha256(support.read_bytes()).hexdigest(),
    findings=['Six original common test bodies called unchanged for AHash and DHash using tracked real image/resize/grayscale fixtures.',
              'Full944line hashing test source read; many wrapper/worker tests assert mocked dispatch, not actual worker behavior.',
              'Original common distinct-image case covers one image pair, not absence of hash collisions or general quality.'],
    limits=['Original class parametrize decorator removed; only AHash/DHash cases called explicitly.',
            'Reuses original-class/Pillow/NumPy setup prefix from pinned own harness; no real package imports or full upstream suite.',
            'No PHash/WHash/native/worker execution; pHash native investigation remains deferred.',
            'No production adoption or performance claim.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'passed':result['passed'],'failed':result['failed']}))
