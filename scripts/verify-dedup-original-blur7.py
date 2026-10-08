"""Audit a radius7 diagnostic while preserving geometry and pixel acceptance thresholds."""
import argparse, ast, hashlib, json, math
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('report', type=Path)
p.add_argument('output', type=Path)
a = p.parse_args()
assert not a.output.exists()
h = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
report_hash = h(a.report)
r = json.loads(a.report.read_text())
assert r['status'] == 'verified_native_output_parity'
assert type(r['returncode']) is int and r['returncode'] == 0
assert all(h(k) == v for k, v in r['input_hashes'].items())
e = r['evidence']
canonical = lambda value: json.dumps(value, sort_keys=True, allow_nan=False)
assert canonical(json.loads(r['stdout'])) == canonical(e)
assert e['status'] == 'ok'
assert type(e['whole_candidate']) is bool
for key in ['managed_used', 'managed_peak', 'retained_before_drop', 'region_support_count']:
    assert type(e[key]) is int and e[key] >= 0
assert e['managed_used'] == 0
assert 0 < e['retained_before_drop'] <= e['managed_peak'] <= 512 * 1024 * 1024
points = e['correspondences']
assert all(len(point) == 2 and all(len(x) == 2 and all(type(y) in [int, float] and math.isfinite(y) for y in x) for x in point) for point in points)
helper = Path('scripts/verify-dedup-scale-similarity-diagnostic.py')
helper_hash = h(helper)
tree = ast.parse(helper.read_text())
names = {'verify_model', 'verify_regions'}
nodes = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name in names]
assert {node.name for node in nodes} == names
namespace = {'math': math}
exec(compile(ast.Module(body=nodes, type_ignores=[]), str(helper), 'exec'), namespace)
assert e['matrix'] is not None
namespace['verify_model'](e['matrix'], e['inliers'], points)
namespace['verify_regions'](e, '200201.jpg')
references = [Path(k) for k in r['input_hashes'] if Path(k).name == 'dedup-original-resolution-three-crops.json']
assert len(references) == 1
reference = json.loads(references[0].read_text())
assert reference['status'] == 'complete'
assert all(h(k) == v for k, v in reference['input_hashes'].items())
row = next(v for v in reference['results'] if v['query_id'] == '200101.jpg')
assert canonical(json.loads(row['stdout'])) == canonical(row['evidence'])
previous = row['evidence']['stats'][1]['distinct']
keys = ['correspondences', 'matrix', 'inliers']
assert canonical({k: e[k] for k in keys}) == canonical({k: previous[k] for k in keys})
assert len(points) == 26 and len(e['inliers']) == 16
assert len(e['regions']) == len(previous['regions'])
assert [v['domains'] for v in e['regions']] == [v['domains'] for v in previous['regions']]
def best_fraction(value):
    return max((min(m / c if c else 0 for m, c, t in region['pixels']['counts'])
                for region in value['regions'] if region['pixels'] is not None), default=0)
assert h(a.report) == report_hash and h(helper) == helper_hash
assert all(h(k) == v for k, v in r['input_hashes'].items())
a.output.write_text(json.dumps({
    'status': 'verified_original_resolution_blur7_geometry_and_region_arithmetic',
    'matches': len(points), 'inliers': len(e['inliers']),
    'whole_candidate': e['whole_candidate'], 'region_support_count': e['region_support_count'],
    'managed_peak': e['managed_peak'], 'retained_before_drop': e['retained_before_drop'],
    'previous_region_support_count': previous['region_support_count'],
    'previous_best_bidirectional_fraction': best_fraction(previous),
    'best_bidirectional_fraction': best_fraction(e),
    'input_hashes': {str(a.report): report_hash, str(helper): helper_hash, str(Path(__file__)): h(__file__)},
    'scope': 'One source-resolution known crop: radius7 versus radius3, unchanged geometry/regions and unchanged acceptance thresholds; all pins, typed native output, residual and regional arithmetic checks. Reports omissions or gains explicitly; no full corpus, negative precision, independent pixel-resampling oracle, default promotion or complete allocation bounds.'
}, indent=2) + '\n')
