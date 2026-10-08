"""Verify immutable full/checkpoint origin controls; keep errors and refusals."""
import argparse
import hashlib
import json
import math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions

p = argparse.ArgumentParser()
p.add_argument('report', type=Path)
p.add_argument('output', type=Path)
p.add_argument('--checkpoint', action='store_true')
a = p.parse_args()
assert not a.output.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
digest = sha(a.report)
r = json.loads(a.report.read_text())
assert r['status'] == 'complete' or a.checkpoint and r['status'] == 'running'
assert r['required_pairs'] == 157 and r['required_negatives'] == 156
assert 0 < len(r['results']) <= 157
if not a.checkpoint:
    assert len(r['results']) == 157
for name, pin in r['pins'].items():
    assert sha(name) == pin, name
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
m = json.loads((T / 'prepared.json').read_text())
query = next(x for x in m['images']['strong'] if x['filename'] == r['query'])
assert query['group_id'] == r['query_group']
expected = sorted(m['images']['original'], key=lambda x: (x['group_id'] != query['group_id'], x['filename']))
target = T / 'original-resolution-strong-all' / r['query']
assert sha(target) == query['source_sha256']
td = oriented_dimensions(target, query['source_size'])
counts = {'positive_supported': 0, 'positive_missed': 0, 'negative_rejected': 0,
          'false_support': 0, 'native_error': 0, 'refused': 0}
for row, want in zip(r['results'], expected):
    assert row['original'] == want['filename'] and row['original_group'] == want['group_id']
    positive = want['group_id'] == query['group_id']
    assert row['positive_control'] == positive
    source = T / 'original-resolution-negative-200201' / row['original']
    assert sha(source) == want['source_sha256']
    sd = oriented_dimensions(source, want['source_size'])
    assert row['source_tolerance'] == 2 * math.hypot(*sd) / math.hypot(*td)
    if row['returncode']:
        assert 'evidence' not in row
        counts['native_error'] += 1
        continue
    e = row['evidence']
    assert e == json.loads(row['stdout'])
    if e['status'] in ('ok', 'no_geometry'):
        assert 1 <= e['attempted_recipes'] <= 3
        assert e['smoothing_radii'] == [[2, 2], [4, 0], [0, 4]][e['attempted_recipes'] - 1]
        if e['status'] == 'no_geometry': assert e['attempted_recipes'] == 3
    if e['status'] == 'ok':
        assert len(e['matrix']) == 3 and all(len(v) == 3 and all(math.isfinite(x) for x in v) for v in e['matrix'])
        assert len(e['points']) <= 28000
        for point in e['points']:
            assert len(point) == 2
            for xy, (w, h) in zip(point, [sd, td]):
                assert len(xy) == 2 and all(math.isfinite(x) for x in xy)
                assert 0 <= xy[0] < w and 0 <= xy[1] < h
        assert len(e['anchors']) == len(e['directions']) == 2
        passes = []
        for anchors, v in zip(e['anchors'], e['directions']):
            assert anchors >= 10 and v['sites'] == 25 * anchors
            assert 0 <= v['agreeing_pairs'] <= v['informative_pairs'] <= 8 * v['valid_sites'] <= 8 * v['sites']
            passes.append(v['informative_pairs'] >= 1000 and v['valid_sites'] / v['sites'] >= .3
                          and v['agreeing_pairs'] / max(1, v['informative_pairs']) >= .9)
        assert e['supported'] == all(passes)
    elif e['status'] == 'no_geometry':
        assert not e['supported']
    else:
        assert e['status'].startswith('refusal:') and not e['supported']
        counts['refused'] += 1
        continue
    counts[('positive_supported' if e['supported'] else 'positive_missed') if positive
           else ('false_support' if e['supported'] else 'negative_rejected')] += 1
assert sum(counts.values()) == len(r['results']) and sha(a.report) == digest
manifest = json.loads(Path('docs/research/dedup-anchor-rank-driven-fallback-real-source.json').read_text())
assert sha(manifest['binary']) == manifest['binary_sha256']
for name, pin in manifest['file_hashes'].items():
    assert sha(Path(manifest['snapshot']) / name) == pin
audit = {'status': 'verified_rank_driven_origin_checkpoint' if a.checkpoint else 'verified_rank_driven_origin_terminal',
         'pairs': len(r['results']), 'counts': counts, 'report_sha256': digest,
         'scope': 'Exact order/provenance/input and frozen-source hashes, oriented pixel bounds, native count predicates; errors/refusals retained. No independent full geometry/pixel reconstruction or broad semantic/collection precision.'}
a.output.write_text(json.dumps(audit, indent=2) + '\n')
print(json.dumps(audit))
