#!/usr/bin/env python3
"""Audit fixed six-lane controls against pinned preceding constituent reports."""
import argparse
import ast
import hashlib
import json
import math
import struct
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('report', 'legacy', 'spatial', 'output'):
    p.add_argument(name, type=Path)
a = p.parse_args()
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
pins = {str(path): digest(path) for path in (a.report, a.legacy, a.spatial)}
r, legacy, spatial = [json.loads(path.read_text()) for path in (a.report, a.legacy, a.spatial)]
assert r['status'] == spatial['status'] == 'complete'
assert r['required_queries'] == ['201301.jpg', '204601.jpg']
assert [v['query_id'] for v in r['results']] == r['required_queries']
old = {v['query_id']: v['native_collection_evidence'] for v in legacy['results']}
prior = {v['query_id']: v['evidence'] for v in spatial['results']}
manifest_path = next(path for path in r['input_hashes'] if Path(path).name == 'prepared.json')
manifest = json.loads(Path(manifest_path).read_text())
pairs = {v['query_id']: v for v in manifest['positive_pairs']}
helper = Path('scripts/verify-dedup-bidirectional-regions.py')
tree = ast.parse(helper.read_text())
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n, ast.FunctionDef)
                             and n.name in ('inverse', 'domains')], type_ignores=[]), str(helper), 'exec'))
for document in (r, legacy, spatial):
    assert all(digest(path) == value for path, value in {**document['input_hashes'], **document['image_hashes']}.items())
verified = []
excluded = {'managed_peak', 'indexed_features', 'descriptor_hits', 'spatial_gradient', 'spatial_gradient_regions', 'spatial_grid'}
for row in r['results']:
    qid, e = row['query_id'], row['evidence']
    assert type(row['returncode']) is int and row['returncode'] == 0
    assert json.loads(row['stdout']) == e and e['status'] == 'ok'
    assert {k: v for k, v in e.items() if k not in excluded} == {k: v for k, v in old[qid].items() if k not in excluded}
    assert type(e['managed_used']) is int and e['managed_used'] == 0
    for key, cap in [('managed_peak', 64 * 1024 * 1024), ('indexed_features', 13000), ('descriptor_hits', 45_000_000)]:
        assert type(e[key]) is int and 0 <= e[key] <= cap
    assert e['retrieved'] is True and e['spatial_grid'] == [4, 4, 32]
    whole = e['spatial_gradient']
    assert whole == {k: prior[qid][k] for k in ('candidate', 'correspondences', 'inliers', 'geometry', 'pixels')}
    assert type(whole['candidate']) is bool and whole['candidate'] is False
    region = e['spatial_gradient_regions']
    assert region == {'geometry': prior[qid]['regional_geometry'], 'regions': prior[qid]['regions'], 'region_support_count': prior[qid]['region_support_count']}
    dimensions = []
    for key in ('left', 'right'):
        image = pairs[qid][key]
        assert r['image_hashes'][image['normalized_path']] == image['normalized_sha256']
        header = Path(image['normalized_path']).read_bytes()[:24]
        assert header[:8] == b'\x89PNG\r\n\x1a\n' and header[12:16] == b'IHDR'
        dimensions.append(struct.unpack('>II', header[16:24]))
    h = region['geometry']
    assert len(h) == 3 and all(len(rr) == 3 for rr in h)
    assert all(type(v) in (int, float) and math.isfinite(v) for rr in h for v in rr)
    expected = domains(*dimensions, h) + [pair[::-1] for pair in domains(*dimensions[::-1], inverse(h))]
    assert [v['domains'] for v in region['regions']] == expected
    supports = 0
    for value in region['regions']:
        assert all(len(rect) == 4 and all(type(v) is int for v in rect) for rect in value['domains'])
        accepted = False
        if value['counts'] is not None:
            assert len(value['counts']) == 2 and value['fit_failure'] == 'None'
            for rectangle, (matched, compared, source) in zip(value['domains'], value['counts']):
                assert all(type(v) is int for v in (matched, compared, source))
                assert 0 <= matched <= compared <= source == rectangle[2] * rectangle[3]
            accepted = all(n >= 1000 and n >= s * 0.3 and v >= n * 0.9 for v, n, s in value['counts'])
        assert type(value['accepted_region']) is bool and value['accepted_region'] == accepted
        supports += accepted
    assert type(region['region_support_count']) is int and supports == region['region_support_count'] == 1
    verified.append({'query_id': qid, 'spatial_region_supports': supports, 'whole_candidate': e['candidate'], 'spatial_whole_candidate': whole['candidate']})
assert all(digest(path) == value for path, value in pins.items())
a.output.write_text(json.dumps({'status': 'verified_six_region_fixed_controls', 'input_sha256': pins,
    'helper_sha256': digest(helper), 'verified_pairs': len(verified), 'results': verified,
    'scope': 'Two fixed recovered pairs; complete legacy confirmations preserved, independent spatial domains/counts/admission. Retrieval metadata differs. No full corpus, negatives or scaling qualification.'}, indent=2) + '\n')
