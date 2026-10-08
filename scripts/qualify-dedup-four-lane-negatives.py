#!/usr/bin/env python3
"""Six fixed different-origin query gates, with gradient constituent parity."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

p = argparse.ArgumentParser()
for name in ('manifest', 'probe', 'output'):
    p.add_argument(name, type=Path)
a = p.parse_args()
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
m = json.loads(a.manifest.read_text())
queries = ('200001.jpg', '201501.jpg', '202701.jpg', '206801.jpg', '207201.jpg', '209901.jpg')
state = {'required_pairs': 936, 'queries': [], 'manifest_sha256': digest(a.manifest),
         'probe_sha256': digest(a.probe), 'scope': 'Six fixed four-lane different-origin gates; not all-query, semantic/burst or full-library qualification.'}
research = a.output.parent
for query in queries:
    stem = 'dedup-four-lane-negative-' + query.removesuffix('.jpg')
    report = research / (stem + '.json')
    reference = research / ('dedup-gradient-public-negative-' + query + '.json')
    # Refuse missing/mismatched references before spending work on a query.
    g = json.loads(reference.read_text())
    assert g['mode'] == 'gradient_pyramid_public_files'
    assert g['manifest_sha256'] == state['manifest_sha256']
    assert g['negative_query'] == query
    assert g['completed_pairs'] == g['required_pairs'] == len(g['results']) == 156
    assert g['status_counts'] == {'ok': 156} and g['candidates'] == 0
    assert g['probe_sha256'] == digest(a.probe.parent / 'photo-probe-gradient-file')
    reference_hash = digest(reference)
    command = ['python3', str(Path(__file__).with_name('qualify-dedup-copydays.py')),
               str(a.manifest), str(a.probe), str(report), '--complementary-gradient', '--negative-query', query]
    if report.exists():
        command.append('--resume')
    with (research / (stem + '.log')).open('a') as log:
        result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
    assert result.returncode == 0, (query, result.returncode)
    r = json.loads(report.read_text())
    assert digest(reference) == reference_hash
    assert r['mode'] == 'complementary_gradient' and g['mode'] == 'gradient_pyramid_public_files'
    for v in (r, g):
        assert v['required_pairs'] == v['completed_pairs'] == len(v['results']) == 156
        assert v['manifest_sha256'] == state['manifest_sha256']
        assert v['status_counts'] == {'ok': 156} and v['candidates'] == 0
    assert r['probe_sha256'] == state['probe_sha256'] == digest(a.probe)
    assert r['status'] == 'complete_measurement_not_full_library_qualification'
    image = next(v['right'] for v in m['positive_pairs'] if v['query_id'] == query)
    originals = [v for v in m['images']['original'] if v['group_id'] != image['group_id']]
    for row, grad, original in zip(r['results'], g['results'], originals):
        for v in (row, grad):
            assert v['query_id'] == query + '/' + original['filename']
            assert v['label'] == 'different_publisher_origin' and v['returncode'] == 0
        e, ge = row['evidence'], grad['evidence']
        assert e['status'] == 'ok' and e['managed_used'] == 0
        assert e['candidate'] is False and e['four_accepted_searches'] == [False] * 4
        assert e['three_candidate'] is False and e['joined_accepted_searches'] == [False] * 3
        for key in ('candidate', 'correspondences', 'inliers', 'geometry'):
            assert e['gradient_' + key] == ge[key], (query, original['filename'], key)
        pixels = ge['pixels']
        assert e['gradient_counts'] == (pixels['counts'] if pixels and pixels['status'] == 'ok' else None)
        failure = 'None' if not pixels or pixels['status'] == 'ok' else pixels['error']
        assert e['gradient_fit_failure'] == failure
        for input_image in (image, original):
            assert digest(Path(input_image['normalized_path'])) == input_image['normalized_sha256']
    state['queries'].append({'query_id': query, 'pairs': 156, 'candidates': 0,
                             'gradient_native_parity': True, 'report_sha256': digest(report),
                             'gradient_reference_sha256': digest(reference)})
    state['completed_pairs'] = sum(v['pairs'] for v in state['queries'])
    temporary = a.output.with_suffix('.tmp')
    temporary.write_text(json.dumps(state, indent=2) + '\n')
    temporary.replace(a.output)
    print(query, '156 verified, zero four-lane candidates', flush=True)
state['status'] = 'complete_six_query_measurement_not_full_library_qualification'
a.output.write_text(json.dumps(state, indent=2) + '\n')
