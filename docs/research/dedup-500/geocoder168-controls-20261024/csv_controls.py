"""Original CSV writer block and original leading next(f), owned fixtures only.

The database copy block is not executed; file remainder is measured instead.
No SQL/psycopg2/dedupe execution or original address data access.
"""
import ast
import csv
import hashlib
import io
import json
from pathlib import Path
from types import SimpleNamespace

source = Path(__file__).resolve().parents[1] / 'evidence/dedupeio--dedupe-geocoder/03-linkNewData.py'
tree = ast.parse(source.read_text())
blocks = [n for n in ast.walk(tree) if isinstance(n, ast.With) and isinstance(n.items[0].context_expr, ast.Call) and isinstance(n.items[0].context_expr.func, ast.Name) and n.items[0].context_expr.func.id == 'open']
writer = next(n for n in blocks if any(isinstance(x, ast.Assign) and isinstance(x.value, ast.Call) and isinstance(x.value.func, ast.Attribute) and x.value.func.attr == 'writer' for x in n.body))
reader = next(n for n in blocks if isinstance(n.body[0], ast.Expr) and isinstance(n.body[0].value, ast.Call) and isinstance(n.body[0].value.func, ast.Name) and n.body[0].value.func.id == 'next')
class RetainedStringIO(io.StringIO):
    def close(self):
        pass

observations = []
for label, matches, expected_rows, expected_exception in [
    ('two accepted matches loses first', [[((1,101),.9),((2,102),.95)]], 1, None),
    ('one accepted match loses all', [[((1,101),.9)]], 0, None),
    ('no accepted matches raises StopIteration', [[((1,101),.5)]], 0, 'StopIteration'),
]:
    owned = RetainedStringIO()
    def fixture_open(path, mode):
        assert path == 'geocoder/data/owned_matches.csv' and mode == 'w'
        return owned
    scope = {'open':fixture_open, 'args':SimpleNamespace(name='owned'), 'matches':matches, 'csv':csv}
    exec(compile(ast.Module(body=[writer],type_ignores=[]),str(source),'exec'),scope)
    data = owned.getvalue()
    reader_scope = {'f':io.StringIO(data)}
    error = None
    try:
        exec(compile(ast.Module(body=[reader.body[0]],type_ignores=[]),str(source),'exec'),reader_scope)
    except StopIteration:
        error = 'StopIteration'
    rows = list(csv.reader(reader_scope['f']))
    assert len(rows)==expected_rows and error==expected_exception
    observations.append({'case':label,'written_csv':data,'remaining_rows':rows,'exception':error})
payload = {'scope':__doc__,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'authored_controls':observations,'original_tests_run':0}
Path(__file__).with_name('csv-results.json').write_text(json.dumps(payload,indent=2)+'\n')
print(json.dumps(payload,indent=2))
