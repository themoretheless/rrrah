"""Independently check pinned native managed file/collection evidence and parity."""
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
references = [Path(k) for k in r['input_hashes'] if Path(k).name == 'dedup-original-managed-region-200201.json']
assert len(references) == 1
reference = json.loads(references[0].read_text())
assert reference['status'] == 'verified_native_output_parity'
assert all(h(k) == v for k, v in reference['input_hashes'].items())
assert canonical(json.loads(reference['stdout'])) == canonical(reference['evidence'])
keys = ['whole_candidate', 'correspondences', 'matrix', 'inliers', 'regions', 'region_support_count', 'retained_before_drop', 'managed_used']
assert canonical({k: e[k] for k in keys}) == canonical({k: reference['evidence'][k] for k in keys})
assert not e['whole_candidate'] and e['region_support_count'] == 1
assert len(points) == 217 and len(e['inliers']) == 125
assert h(a.report) == report_hash and h(helper) == helper_hash
assert all(h(k) == v for k, v in r['input_hashes'].items())
a.output.write_text(json.dumps({
    'status': 'verified_managed_real_collection_pair',
    'matches': len(points), 'inliers': len(e['inliers']),
    'whole_candidate': e['whole_candidate'], 'region_support_count': e['region_support_count'],
    'managed_peak': e['managed_peak'], 'retained_before_drop': e['retained_before_drop'],
    'input_hashes': {str(a.report): report_hash, str(helper): helper_hash, str(Path(__file__)): h(__file__)},
    'scope': 'One known publisher-origin crop at source resolution: raw typed evidence, all producer pins, projective residuals, regional acceptance arithmetic and exact managed file/collection parity. No full corpus, negative precision, pixel resampling oracle or complete decoder/geometry/RSS bound claim.'
}, indent=2) + '\n')
