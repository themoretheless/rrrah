#!/usr/bin/env python3
"""Audit all native spatial-regional negatives, including actual positive flags."""
import argparse
import ast
import hashlib
import json
import math
import struct
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('manifest', 'probe', 'report', 'query', 'output'):
    p.add_argument(name)
p.add_argument('--checkpoint', action='store_true')
a = p.parse_args()
helper = Path('scripts/verify-dedup-bidirectional-regions.py')
tree = ast.parse(helper.read_text())
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name in ('inverse', 'domains')], type_ignores=[]), str(helper), 'exec'))
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
raw = Path(a.report).read_bytes()
manifest_raw = Path(a.manifest).read_bytes()
r, m = json.loads(raw), json.loads(manifest_raw)
assert r['mode'] == 'spatial_gradient_regions_negative_control' and r['query'] == a.query
assert r['manifest_sha256'] == hashlib.sha256(manifest_raw).hexdigest() and r['probe_sha256'] == digest(a.probe)
query = next(v['right'] for v in m['positive_pairs'] if v['query_id'] == a.query)
originals = [v for v in m['images']['original'] if v['group_id'] != query['group_id']]
assert len(originals) == r['required_pairs'] == 156
assert r['status'] in ('checkpoint', 'complete') and 0 < len(r['results']) <= 156
if not a.checkpoint:
    assert r['status'] == 'complete' and len(r['results']) == 156
images, whole, supported = {}, [], []
retrieved = 0
for row, original in zip(r['results'], originals):
    pair_id = query['filename'] + '/' + original['filename']
    assert row['query_id'] == pair_id and row['label'] == 'different_publisher_origin'
    assert type(row['returncode']) is int and row['returncode'] == 0
    e = row['evidence']
    assert e['status'] == 'ok' and type(e['retrieved']) is bool and type(e['candidate']) is bool
    assert type(e['managed_used_after_drop']) is int and e['managed_used_after_drop'] == 0
    assert type(e['managed_peak']) is int and 0 <= e['managed_peak'] <= 64 * 1024 * 1024
    dimensions = []
    for image in (original, query):
        path = image['normalized_path']
        assert digest(path) == image['normalized_sha256']
        images[path] = image['normalized_sha256']
        header = Path(path).read_bytes()[:24]
        assert header[:8] == b'\x89PNG\r\n\x1a\n' and header[12:16] == b'IHDR'
        dimensions.append(struct.unpack('>II', header[16:24]))
    actual = 0
    if e['retrieved']:
        retrieved += 1
        assert e['spatial_grid'] == [4, 4, 32]
        for key, cap in (('indexed_features', 3000), ('descriptor_hits', 9_000_000), ('correspondences', 1500), ('inliers', 1500)):
            assert type(e[key]) is int and 0 <= e[key] <= cap
        assert e['inliers'] <= e['correspondences']
        pixels = e['pixels']
        accepted_whole = False
        if pixels is not None and pixels['status'] == 'ok':
            assert len(pixels['counts']) == 2
            for matched, compared, source in pixels['counts']:
                assert all(type(v) is int for v in (matched, compared, source)) and 0 <= matched <= compared <= source
            accepted_whole = all(n >= 1000 and n >= s * 0.3 and v >= n * 0.9 for v, n, s in pixels['counts'])
        assert e['candidate'] == accepted_whole
        h = e['regional_geometry']
        if h is None:
            expected = []
        else:
            assert len(h) == 3 and all(len(rr) == 3 for rr in h) and all(math.isfinite(v) for rr in h for v in rr)
            left, right = dimensions
            expected = domains(left, right, h) + [pair[::-1] for pair in domains(right, left, inverse(h))]
        assert [v['domains'] for v in e['regions']] == expected
        for region in e['regions']:
            flag = False
            counts = region['counts']
            if counts is not None:
                assert len(counts) == 2 and region['fit_failure'] == 'None'
                for rectangle, (matched, compared, source) in zip(region['domains'], counts):
                    assert all(type(v) is int for v in (matched, compared, source))
                    assert 0 <= matched <= compared <= source == rectangle[2] * rectangle[3]
                flag = all(n >= 1000 and n >= s * 0.3 and v >= n * 0.9 for v, n, s in counts)
            assert type(region['accepted_region']) is bool and region['accepted_region'] == flag
            actual += flag
    else:
        assert e['candidate'] is False and e['regions'] == []
        assert not any(k in e for k in ('geometry', 'regional_geometry', 'pixels'))
    assert type(e['region_support_count']) is int and e['region_support_count'] == actual
    if e['candidate']:
        whole.append(pair_id)
    if actual:
        supported.append(pair_id)
assert r['image_hashes'] == images
assert all(digest(path) == value for path, value in images.items())
assert Path(a.report).read_bytes() == raw and Path(a.manifest).read_bytes() == manifest_raw and digest(a.probe) == r['probe_sha256']
result = {'status': 'verified_finite_spatial_region_negatives', 'report_sha256': hashlib.sha256(raw).hexdigest(),
          'independent_domain_helper_sha256': digest(helper), 'query': a.query, 'verified_pairs': len(r['results']),
          'required_pairs': 156, 'retrieved_pairs': retrieved, 'whole_positive_pairs': whole, 'region_supported_pairs': supported,
          'verified_images': len(images), 'scope': 'One fixed query against different publisher-origin originals. Actual positive flags retained; no semantic/burst, all-query precision or full-library qualification.'}
Path(a.output).write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
