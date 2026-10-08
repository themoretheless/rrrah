#!/usr/bin/env python3
"""Compare fresh legacy and spatial file paths on the declared insufficient-match subset."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('manifest', 'probe', 'reference', 'partition', 'output'):
    p.add_argument(name, type=Path)
a = p.parse_args()
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
inputs = {str(path): digest(path) for path in (a.manifest, a.probe, a.reference, a.partition)}
pairs = {v['query_id']: v for v in json.loads(a.manifest.read_text())['positive_pairs']}
ref = json.loads(a.reference.read_text())
partition = json.loads(a.partition.read_text())
assert inputs[str(a.reference)] == partition['input_hashes'][str(a.reference)]
queries = [v['query_id'] for v in partition['classes']['both_below_required_10']]
assert len(queries) == len(set(queries)) == 31
refs = {v['query_id']: v['evidence'] for v in ref['results']}
r = {'status': 'running', 'input_hashes': inputs, 'image_hashes': {},
     'required_queries': queries, 'results': [],
     'scope': '31 prior insufficient-match publisher-origin copies. Fresh legacy baseline required to equal pinned native fields. Explicit spatial mode, unchanged geometry/pixel thresholds; no broad recall/precision or collection qualification.'}
for qid in queries:
    pair = pairs[qid]
    paths = [pair[key]['normalized_path'] for key in ('left', 'right')]
    for key in ('left', 'right'):
        item = pair[key]
        assert digest(item['normalized_path']) == item['normalized_sha256']
        r['image_hashes'][item['normalized_path']] = item['normalized_sha256']
    measurements = []
    for index, recipe in enumerate(('fixed', 'interpolated')):
        measured = {}
        suffix = '-interpolated-file-pair' if index else '-file-pair'
        for spatial in (False, True):
            mode = ('--spatial-gradient' if spatial else '--gradient') + suffix
            run = subprocess.run([str(a.probe), mode, *paths], capture_output=True, text=True)
            assert run.returncode == 0, (qid, mode, run.returncode, run.stderr)
            e = json.loads(run.stdout)
            assert e['status'] == 'ok' and e['managed_used_after_drop'] == 0 and e['managed_peak'] <= 64 * 1024 * 1024
            assert type(e['candidate']) is bool
            for key in ('correspondences', 'inliers'):
                assert type(e[key]) is int and 0 <= e[key] <= 1500
            if not spatial:
                prefix = 'interpolated' if index else 'gradient'
                for key in ('correspondences', 'inliers', 'geometry', 'candidate'):
                    assert e[key] == refs[qid][prefix + '_' + key], (qid, recipe, key, 'legacy drift')
                assert e['pixels'] is None
            else:
                assert e['spatial_grid'] == [4, 4, 32]
            measured['spatial' if spatial else 'legacy'] = e
        measurements.append({'recipe': recipe, **measured})
    r['results'].append({'query_id': qid, 'measurements': measurements})
    assert all(digest(path) == expected for path, expected in {**inputs, **r['image_hashes']}.items())
    a.output.write_text(json.dumps(r, indent=2) + '\n')
    print(json.dumps({'query_id': qid, 'matches': [[v['legacy']['correspondences'], v['spatial']['correspondences']] for v in measurements],
                      'spatial_candidates': [v['spatial']['candidate'] for v in measurements]}), flush=True)
r['status'] = 'complete'
a.output.write_text(json.dumps(r, indent=2) + '\n')
