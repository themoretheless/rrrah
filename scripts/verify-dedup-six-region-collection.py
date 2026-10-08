#!/usr/bin/env python3
"""Independent domain and pixel-admission audit of supplied-model measurements."""
import argparse
import hashlib
import json
import math
import struct
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('report', 'output'):
    p.add_argument(name, type=Path)
p.add_argument('--checkpoint', action='store_true')
a = p.parse_args()
raw = a.report.read_bytes()
r = json.loads(raw)
assert len(set(r['required_queries'])) == len(r['required_queries'])
if a.checkpoint:
    assert r['status'] in ('running', 'checkpoint', 'complete')
    assert 0 < len(r['results']) <= len(r['required_queries'])
    assert [v['query_id'] for v in r['results']] == r['required_queries'][:len(r['results'])]
else:
    assert r['status'] == 'complete'
    assert [v['query_id'] for v in r['results']] == r['required_queries']
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
for path, expected in {**r['input_hashes'], **r['image_hashes']}.items():
    assert digest(path) == expected, path
reference_path = next(path for path in r['input_hashes'] if Path(path).name == 'dedup-five-all-regions-229.json')
manifest_path = next(path for path in r['input_hashes'] if Path(path).name == 'prepared.json')
refs = {v['query_id']: v['evidence'] for v in json.loads(Path(reference_path).read_text())['results']}
pairs = {v['query_id']: v for v in json.loads(Path(manifest_path).read_text())['positive_pairs']}

def inverse(h):
    # Match the public projective representation: scaled adjugate, without
    # determinant division. Equivalent real-valued matrices can round mapped
    # integer boundaries differently, so normalization is part of this oracle.
    scale = max(abs(v) for row in h for v in row)
    assert math.isfinite(scale) and scale > 0
    h = [[v / scale for v in row] for row in h]
    cofactors = []
    for i in range(3):
        row = []
        for j in range(3):
            minor = [[h[y][x] for x in range(3) if x != j] for y in range(3) if y != i]
            row.append((-1) ** (i + j) * (minor[0][0] * minor[1][1] - minor[0][1] * minor[1][0]))
        cofactors.append(row)
    determinant = sum(h[0][j] * cofactors[0][j] for j in range(3))
    assert math.isfinite(determinant) and determinant != 0
    return [[cofactors[j][i] for j in range(3)] for i in range(3)]

def domains(source, target, h):
    output = []
    for gy in range(4):
        for gx in range(4):
            x, y = gx * source[0] // 4, gy * source[1] // 4
            width, height = (gx + 1) * source[0] // 4 - x, (gy + 1) * source[1] // 4 - y
            if width == 0 or height == 0:
                continue
            corners = []
            for px, py in ((x, y), (x + width - 1, y), (x, y + height - 1), (x + width - 1, y + height - 1)):
                denominator = h[2][0] * px + h[2][1] * py + h[2][2]
                corners.append([(h[axis][0] * px + h[axis][1] * py + h[axis][2]) / denominator for axis in range(2)])
            low = [max(0, min(target[axis], math.floor(min(c[axis] for c in corners)))) for axis in range(2)]
            high = [max(0, min(target[axis], math.ceil(max(c[axis] for c in corners)) + 1)) for axis in range(2)]
            if all(high[axis] > low[axis] for axis in range(2)):
                output.append([[x, y, width, height], [*low, high[0] - low[0], high[1] - low[1]]])
    return output

gains = []
models = 0
collection_queries = 0
for row in r['results']:
    qid = row['query_id']
    pair, prior = pairs[qid], refs[qid]
    if 'native_collection_evidence' in row:
        collection_queries += 1
        native = row['native_collection_evidence']
        excluded = {'managed_peak', 'gradient_regions', 'indexed_features', 'descriptor_hits', 'spatial_gradient', 'spatial_gradient_regions', 'spatial_grid'}
        assert {k: v for k, v in native.items() if k not in excluded} == {k: v for k, v in prior.items() if k not in excluded}
        assert len(native['gradient_regions']) == len(row['gradient_regions']) == 2
        for measured, derived in zip(native['gradient_regions'], row['gradient_regions']):
            assert (measured is None) == (derived is None)
            if measured is not None:
                assert measured == {k: v for k, v in derived.items() if k not in ('status', 'managed_used', 'managed_peak')}
                assert derived['managed_used'] == native['managed_used']
                assert derived['managed_peak'] == native['managed_peak']
    paths = [pair[key]['normalized_path'] for key in ('left', 'right')]
    assert all(path in r['image_hashes'] for path in paths)
    dimensions = []
    for path in paths:
        png = Path(path).read_bytes()
        assert png[:8] == b'\x89PNG\r\n\x1a\n'
        dimensions.append(struct.unpack('>II', png[16:24]))
    assert len(row['gradient_regions']) == 2
    for lane_index, e in enumerate(row['gradient_regions']):
        old = prior['gradient_regions'][lane_index]
        assert (e is None) == (old is None)
        if e is None:
            continue
        models += 1
        assert e['status'] == 'ok' and type(e['managed_used']) is int and e['managed_used'] == 0
        assert type(e['managed_peak']) is int and 0 <= e['managed_peak'] <= 64 * 1024 * 1024
        h = e['geometry']
        assert h == old['geometry'] and all(math.isfinite(v) for rr in h for v in rr)
        left, right = dimensions
        expected = domains(left, right, h) + [pair[::-1] for pair in domains(right, left, inverse(h))]
        assert [region['domains'] for region in e['regions']] == expected, (qid, lane_index, 'domains')
        assert e['regions'][:len(old['regions'])] == old['regions']
        supports = 0
        for region in e['regions']:
            counts = region['counts']
            accepted = False
            if counts is not None:
                assert len(counts) == 2
                for rectangle, (matched, compared, source) in zip(region['domains'], counts):
                    assert all(type(v) is int for v in (matched, compared, source))
                    assert 0 <= matched <= compared <= source == rectangle[2] * rectangle[3]
                accepted = all(compared >= 1000 and compared >= source * 0.3 and matched >= compared * 0.9 for matched, compared, source in counts)
            assert type(region['accepted_region']) is bool and region['accepted_region'] == accepted
            supports += accepted
        assert type(e['region_support_count']) is int and supports == e['region_support_count']
        if supports > old['region_support_count']:
            gains.append({'query_id': qid, 'lane': lane_index, 'old_supports': old['region_support_count'], 'new_supports': supports})
six_whole = []
spatial_supported = []
union_supported = []
for row in r['results']:
    qid = row['query_id']
    native = row['native_collection_evidence']
    for key, cap in [('managed_peak',64*1024*1024),('indexed_features',13000),('descriptor_hits',45_000_000)]:
        assert type(native[key]) is int and 0 <= native[key] <= cap
    assert native['spatial_grid'] == [4,4,32]
    g = native['spatial_gradient']
    assert type(g['candidate']) is bool
    for key in ('inliers','correspondences'):
        assert type(g[key]) is int and 0 <= g[key] <= 1500
    assert g['inliers'] <= g['correspondences']
    pair = pairs[qid]
    dimensions = [struct.unpack('>II',Path(pair[key]['normalized_path']).read_bytes()[16:24]) for key in ('left','right')]
    whole = False
    if g['pixels'] is not None and g['pixels']['status'] == 'ok':
        counts = g['pixels']['counts']
        assert len(counts) == 2
        for (matched, compared, source),(w,h) in zip(counts,dimensions):
            assert all(type(v) is int for v in (matched,compared,source)) and 0 <= matched <= compared <= source == w*h
        whole = all(n>=1000 and n>=s*.3 and v>=n*.9 for v,n,s in counts)
    assert g['candidate'] == whole
    region = native['spatial_gradient_regions']
    assert (region is None) == (g['geometry'] is None)
    supports = 0
    if region is not None:
        h = region['geometry']
        assert h == g['geometry'] and len(h)==3 and all(len(rr)==3 for rr in h)
        assert all(type(v) in (int,float) and math.isfinite(v) for rr in h for v in rr)
        expected = domains(*dimensions,h)+[pair[::-1] for pair in domains(*dimensions[::-1],inverse(h))]
        assert [v['domains'] for v in region['regions']] == expected
        for value in region['regions']:
            assert all(len(rect)==4 and all(type(v) is int for v in rect) for rect in value['domains'])
            accepted = False
            if value['counts'] is not None:
                assert len(value['counts'])==2 and value['fit_failure']=='None'
                for rect,(matched,compared,source) in zip(value['domains'],value['counts']):
                    assert all(type(v) is int for v in (matched,compared,source)) and 0<=matched<=compared<=source==rect[2]*rect[3]
                accepted=all(n>=1000 and n>=s*.3 and v>=n*.9 for v,n,s in value['counts'])
            assert type(value['accepted_region']) is bool and value['accepted_region']==accepted
            supports += accepted
        assert type(region['region_support_count']) is int and region['region_support_count']==supports
    if native['candidate'] or whole:six_whole.append(qid)
    if supports:spatial_supported.append(qid)
    if native['candidate'] or whole or native['region_support_count'] or supports or any(v and v['region_support_count'] for v in native['gradient_regions']):union_supported.append(qid)
assert a.report.read_bytes() == raw
assert all(digest(path) == expected for path, expected in {**r['input_hashes'], **r['image_hashes']}.items())
result = {'status': 'verified_supplied_model_bidirectional_regions', 'report_sha256': hashlib.sha256(raw).hexdigest(),
          'verified_queries': len(r['results']), 'verified_models': models, 'verified_native_collection_queries': collection_queries, 'lane_support_gains': gains,
          'scope': 'Pinned selected queries. Independent bidirectional grid bounds and original pixel admission; exact prior first-grid parity. Supports overlap; no whole-image promotion, broad precision or full-library qualification.'}
if a.checkpoint:
    result.update(status='verified_bidirectional_completed_prefix', required_queries=len(r['required_queries']),
                  scope='Completed ordered prefix only. Independent domains/pixel admission and prior constituent parity; not full corpus, precision or full-library qualification.')
result.update(status='verified_six_region_collection_prefix' if a.checkpoint else 'verified_six_region_collection',six_whole_candidates=six_whole,spatial_region_supported_queries=spatial_supported,whole_or_region_supported_queries=union_supported)
a.output.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
