#!/usr/bin/env python3
"""Native collection confirmation, with complete preceding constituent parity."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('manifest', 'probe', 'reference', 'output'):
    p.add_argument(name, type=Path)
p.add_argument('query_ids', nargs='+')
a = p.parse_args()
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
inputs = {str(path): digest(path) for path in (a.manifest, a.probe, a.reference)}
pairs = {v['query_id']: v for v in json.loads(a.manifest.read_text())['positive_pairs']}
prior = json.loads(a.reference.read_text())
assert prior['status'] == 'complete'
refs = {v['query_id']: v['evidence'] for v in prior['results']}
assert len(set(a.query_ids)) == len(a.query_ids)
r = {'status': 'running', 'input_hashes': inputs, 'image_hashes': {},
     'required_queries': a.query_ids, 'results': [],
     'scope': 'Native two-source collection confirmations. Complete prior whole/binary fields and first gradient grids retained. Regional support does not promote whole equality; no precision or single full-corpus collection qualification.'}
for qid in a.query_ids:
    pair = pairs[qid]
    paths = [pair[key]['normalized_path'] for key in ('left', 'right')]
    for key in ('left', 'right'):
        item = pair[key]
        assert digest(item['normalized_path']) == item['normalized_sha256']
        r['image_hashes'][item['normalized_path']] = item['normalized_sha256']
    run = subprocess.run([str(a.probe), '--six-regions-collection-pair', *paths], capture_output=True, text=True)
    assert run.returncode == 0, (qid, run.returncode, run.stderr)
    e = json.loads(run.stdout)
    assert e['status'] == 'ok' and e['retrieved'] is True and e['managed_used'] == 0
    assert e['managed_peak'] <= 64 * 1024 * 1024
    old = refs[qid]
    excluded = {'managed_peak', 'gradient_regions', 'indexed_features', 'descriptor_hits', 'spatial_gradient', 'spatial_gradient_regions', 'spatial_grid'}
    assert {k: v for k, v in e.items() if k not in excluded} == {k: v for k, v in old.items() if k not in excluded}, qid
    lanes = []
    for new_lane, old_lane in zip(e['gradient_regions'], old['gradient_regions']):
        assert (new_lane is None) == (old_lane is None)
        if new_lane is None:
            lanes.append(None)
            continue
        assert new_lane['geometry'] == old_lane['geometry']
        assert new_lane['regions'][:len(old_lane['regions'])] == old_lane['regions']
        assert new_lane['region_support_count'] >= old_lane['region_support_count']
        lanes.append({**new_lane, 'status': 'ok', 'managed_used': e['managed_used'], 'managed_peak': e['managed_peak']})
    assert len(lanes) == 2
    r['results'].append({'query_id': qid, 'gradient_regions': lanes, 'native_collection_evidence': e})
    assert all(digest(path) == expected for path, expected in {**inputs, **r['image_hashes']}.items())
    a.output.write_text(json.dumps(r, indent=2) + '\n')
    print(json.dumps({'query_id': qid, 'supports': [None if lane is None else lane['region_support_count'] for lane in lanes]}), flush=True)
r['mode'] = 'six_region_collection_controls'
r['status'] = 'complete'
a.output.write_text(json.dumps(r, indent=2) + '\n')
