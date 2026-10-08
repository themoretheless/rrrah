#!/usr/bin/env python3
"""Keep the native inverse representation observable at integer pixel boundaries."""
import argparse
import ast
import hashlib
import json
import math
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('report', type=Path)
p.add_argument('output', type=Path)
a = p.parse_args()
raw = a.report.read_bytes()
r = json.loads(raw)
row = next(v for v in r['results'] if v['query_id'] == '201401.jpg')
lane = row['gradient_regions'][1]
h = lane['geometry']
assert h == [[0.9999999999999999, 0.0, 1.4210854715202004e-14],
             [0.0, 0.9999999999999999, 0.0], [0.0, 0.0, 1.0]]
helper = Path('scripts/verify-dedup-bidirectional-regions.py')
tree = ast.parse(helper.read_text())
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n, ast.FunctionDef)
                             and n.name in ('inverse', 'domains')], type_ignores=[]), str(helper), 'exec'))
reverse = inverse(h)
expected = domains((320, 240), (320, 240), h) + [pair[::-1] for pair in domains((320, 240), (320, 240), reverse)]
actual = [v['domains'] for v in lane['regions']]
assert actual == expected
assert actual[17] == [[79, 0, 81, 61], [80, 0, 80, 60]]
assert actual[24] == [[0, 120, 80, 60], [0, 120, 80, 60]]
# A real-valued equivalent conventional inverse reproduces the old auditor bug.
determinant = h[0][0] * h[1][1]
conventional = [[v / determinant for v in rr] for rr in reverse]
wrong = domains((320, 240), (320, 240), h) + [pair[::-1] for pair in domains((320, 240), (320, 240), conventional)]
differences = [i for i, (native, other) in enumerate(zip(actual, wrong)) if native != other]
assert differences and len(actual) == len(wrong) == 32
assert a.report.read_bytes() == raw
a.output.write_text(json.dumps({'status': 'verified_native_grid_rounding_regression',
    'report_sha256': hashlib.sha256(raw).hexdigest(),
    'helper_sha256': hashlib.sha256(helper.read_bytes()).hexdigest(),
    'query': row['query_id'], 'lane': 1, 'verified_regions': 32,
    'conventional_inverse_mismatched_regions': differences,
    'scope': 'Pinned native representation regression; no new accuracy or performance measurement.'}, indent=2) + '\n')
