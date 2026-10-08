#!/usr/bin/env python3
"""Audit regional copy recovery without promoting regions to whole decisions."""
import argparse
import collections
import hashlib
import json
import math
import struct
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('manifest', 'probe', 'report', 'whole_reference'):
    p.add_argument(name, type=Path)
p.add_argument('--checkpoint', action='store_true')
a = p.parse_args()
m = json.loads(a.manifest.read_text())
r = json.loads(a.report.read_text())
w = json.loads(a.whole_reference.read_text())
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
assert r['manifest_sha256'] == w['manifest_sha256'] == digest(a.manifest)
assert r['probe_sha256'] == digest(a.probe)
assert r['mode'] == 'pyramid_region_grid'
assert w['mode'] == 'pyramid_blur_radius3'
assert r['required_pairs'] == w['required_pairs'] == len(m['positive_pairs']) == 229
assert w['completed_pairs'] == len(w['results']) == 229
assert r['completed_pairs'] == len(r['results']) <= 229
if not a.checkpoint:
    assert r['completed_pairs'] == 229
counts = collections.Counter()
whole = supported = total = regional_only = 0
checked_images = set()
dimensions = {}


def expected_domains(matrix, source, target):
    """Reconstruct the declared uniform grid from image headers and native H."""
    if matrix is None:
        return []
    assert len(matrix) == 3 and all(len(row) == 3 for row in matrix)
    assert all(math.isfinite(v) for row in matrix for v in row)
    sw, sh = source
    tw, th = target
    result = []
    for row in range(4):
        for col in range(4):
            x, y = col * sw // 4, row * sh // 4
            width, height = (col + 1) * sw // 4 - x, (row + 1) * sh // 4 - y
            if not width or not height:
                continue
            mapped = []
            for cx, cy in ((x, y), (x + width - 1, y), (x, y + height - 1), (x + width - 1, y + height - 1)):
                q = [r[0] * cx + r[1] * cy + r[2] for r in matrix]
                assert all(math.isfinite(v) for v in q) and q[2] != 0
                mapped.append((q[0] / q[2], q[1] / q[2]))
            starts = [max(0, min(bound, math.floor(min(v[axis] for v in mapped)))) for axis, bound in enumerate((tw, th))]
            ends = [max(0, min(bound, math.ceil(max(v[axis] for v in mapped)) + 1)) for axis, bound in enumerate((tw, th))]
            if ends[0] <= starts[0] or ends[1] <= starts[1]:
                continue
            result.append([[x, y, width, height], [*starts, ends[0] - starts[0], ends[1] - starts[1]]])
    return result


for row, pair, reference in zip(r['results'], m['positive_pairs'], w['results']):
    assert row['query_id'] == reference['query_id'] == pair['query_id']
    assert row['label'] == reference['label'] == pair['label'] == 'publisher_origin_copy'
    assert row['status'] in ('ok', 'error', 'timeout', 'process_error')
    counts[row['status']] += 1
    for side in ('left', 'right'):
        image = pair[side]
        path = Path(image['normalized_path'])
        if path not in checked_images:
            assert digest(path) == image['normalized_sha256']
            header = path.read_bytes()[:24]
            assert header[:8] == b'\x89PNG\r\n\x1a\n' and header[12:16] == b'IHDR'
            dimensions[path] = struct.unpack('>II', header[16:24])
            checked_images.add(path)
    if row['status'] != 'ok':
        continue  # Refusals never become rejected copies.
    e = row['evidence']
    assert row['returncode'] == 0 and e['status'] == 'ok'
    assert reference['status'] == 'ok'
    assert type(e['candidate']) is bool
    assert e['candidate'] == reference['evidence']['candidate']
    assert e['geometry'] == reference['evidence']['geometry']
    assert e['managed_used_after_drop'] == 0
    regions = e['regions']
    assert len(regions) <= 16
    assert [region['domains'] for region in regions] == expected_domains(
        e['geometry'], dimensions[Path(pair['left']['normalized_path'])],
        dimensions[Path(pair['right']['normalized_path'])])
    if e['geometry'] is None:
        assert not regions and not e['candidate']
    for region in regions:
        assert type(region['accepted_region']) is bool
        areas = []
        for domain, side in zip(region['domains'], ('left', 'right')):
            assert len(domain) == 4 and all(type(v) is int and v >= 0 for v in domain)
            assert domain[2] > 0 and domain[3] > 0
            width, height = dimensions[Path(pair[side]['normalized_path'])]
            assert domain[0] + domain[2] <= width and domain[1] + domain[3] <= height
            areas.append(domain[2] * domain[3])
        assert len(areas) == 2
        if region['counts'] is None:
            assert not region['accepted_region'] and region['fit_failure'] != 'None'
        else:
            assert region['fit_failure'] == 'None' and len(region['counts']) == 2
            accepted = True
            for values, area in zip(region['counts'], areas):
                assert len(values) == 3 and all(type(v) is int and v >= 0 for v in values)
                matched, compared, source = values
                assert matched <= compared <= source == area
                accepted &= compared >= 1000 and compared >= source * .3 and matched >= compared * .9
            assert region['accepted_region'] == accepted
    n = sum(v['accepted_region'] for v in regions)
    assert type(e['region_support_count']) is int and e['region_support_count'] == n
    whole += e['candidate']
    supported += n > 0
    total += n
    regional_only += n > 0 and not e['candidate']
assert r['status_counts'] == dict(counts)
assert (r['candidates'], r['region_supported_pairs'], r['region_support_total']) == (whole, supported, total)
print(json.dumps({'completed_pairs': r['completed_pairs'], 'status_counts': dict(counts),
    'whole_candidates': whole, 'region_supported_pairs': supported,
    'region_support_total': total, 'regional_only_pairs': regional_only,
    'verified_images': len(checked_images), 'scope': 'Regional diagnostics; whole decisions and geometry equal fixed radius3 reference. Errors stay explicit; not full coverage.'}, indent=2))
