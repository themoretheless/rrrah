#!/usr/bin/env python3
"""Pinned supplied-model measurements; require original grid prefix parity."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

p = argparse.ArgumentParser()
for key in ('manifest', 'probe', 'reference', 'output'):
    p.add_argument(key, type=Path)
p.add_argument('query_ids', nargs='+')
a = p.parse_args()
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
inputs = {str(path): digest(path) for path in (a.manifest, a.probe, a.reference)}
manifest = json.loads(a.manifest.read_text())
reference = json.loads(a.reference.read_text())
assert reference['status'] == 'complete'
pairs = {v['query_id']: v for v in manifest['positive_pairs']}
refs = {v['query_id']: v['evidence'] for v in reference['results']}
assert len(set(a.query_ids)) == len(a.query_ids)
report = {'status': 'running', 'input_hashes': inputs, 'image_hashes': {},
          'required_queries': a.query_ids, 'results': [],
          'scope': 'Supplied prior models; native first-grid parity required. No new model estimation, whole-copy promotion, broad accuracy or collection qualification.'}
for qid in a.query_ids:
    pair = pairs[qid]
    paths = [pair[key]['normalized_path'] for key in ('left', 'right')]
    for key in ('left', 'right'):
        item = pair[key]
        assert digest(item['normalized_path']) == item['normalized_sha256']
        report['image_hashes'][item['normalized_path']] = item['normalized_sha256']
    results = []
    for lane_index, prior in enumerate(refs[qid]['gradient_regions']):
        if prior is None:
            results.append(None)
            continue
        coefficients = [str(v) for row in prior['geometry'] for v in row]
        run = subprocess.run([str(a.probe), '--bidirectional-region-transform', *paths, *coefficients], capture_output=True, text=True)
        assert run.returncode == 0, (qid, lane_index, run.returncode, run.stderr)
        e = json.loads(run.stdout)
        assert e['status'] == 'ok' and e['managed_used'] == 0 and e['managed_peak'] <= 64 * 1024 * 1024
        assert e['geometry'] == prior['geometry']
        assert e['regions'][:len(prior['regions'])] == prior['regions'], (qid, lane_index, 'first-grid parity')
        assert len(e['regions']) <= 32
        assert e['region_support_count'] == sum(v['accepted_region'] for v in e['regions'])
        assert e['region_support_count'] >= prior['region_support_count']
        results.append(e)
    report['results'].append({'query_id': qid, 'gradient_regions': results})
    assert all(digest(path) == value for path, value in inputs.items())
    assert all(digest(path) == value for path, value in report['image_hashes'].items())
    a.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'query_id': qid, 'supports': [None if v is None else v['region_support_count'] for v in results]}), flush=True)
report['status'] = 'complete'
a.output.write_text(json.dumps(report, indent=2) + '\n')
