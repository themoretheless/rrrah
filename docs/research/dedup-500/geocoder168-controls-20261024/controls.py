"""Pinned original preProcess method only; no dedupe, SQL or network execution."""
import ast
import hashlib
import json
from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
source = root / 'evidence/dedupeio--dedupe-geocoder/02-deduper.py'
tree = ast.parse(source.read_text())
cls = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == 'StaticDatabaseGazetteer')
method = next(n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == 'preProcess')
scope = {'re': re}
exec(compile(ast.Module(body=[method], type_ignores=[]), str(source), 'exec'), scope)
normalize = lambda value: scope['preProcess'](None, value)
once = normalize('alpha\n beta')
twice = normalize(once)
results = [
    {'name': 'newline normalization is not idempotent', 'once': once, 'twice': twice, 'passed': once == 'alpha  beta' and twice == 'alpha beta'},
    {'name': 'case and outer whitespace intentionally collapsed', 'passed': normalize('  MAIN STREET  ') == normalize('main street') == 'main street'},
    {'name': 'tabs differ from ordinary spaces', 'passed': normalize('alpha\tbeta') != normalize('alpha beta')},
]
assert all(row['passed'] for row in results)
payload = {'scope': __doc__, 'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'original_tests_run': 0, 'authored_controls': results}
Path(__file__).with_name('results.json').write_text(json.dumps(payload, indent=2) + '\n')
print(json.dumps(payload, indent=2))
