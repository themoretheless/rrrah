#!/usr/bin/env python3
"""Integrated five-search negatives for control and every measured gradient gain."""
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
queries = ('200001.jpg', '201501.jpg', '202701.jpg', '206801.jpg',
           '207201.jpg', '209901.jpg', '207302.jpg', '208102.jpg', '214501.jpg')
state = {'required_pairs': len(queries)*156, 'queries': [],
         'manifest_sha256': digest(a.manifest), 'probe_sha256': digest(a.probe),
         'scope': 'Control and eight measured gradient-gain queries against different publisher origins through integrated five-search API. Not all-query, semantic/burst or full-library qualification.'}
for query in queries:
    stem = 'dedup-five-lane-negative-' + query.removesuffix('.jpg')
    report = a.output.parent / (stem + '.json')
    command = ['python3', str(Path(__file__).with_name('qualify-dedup-copydays.py')),
               str(a.manifest), str(a.probe), str(report),
               '--complementary-gradient-portfolio', '--negative-query', query]
    if report.exists():
        command.append('--resume')
    with (a.output.parent / (stem + '.log')).open('a') as log:
        result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
    assert result.returncode == 0, (query, result.returncode)
    payload = report.read_bytes()
    r = json.loads(payload)
    assert r['mode'] == 'complementary_gradient_portfolio'
    assert r['manifest_sha256'] == state['manifest_sha256'] == digest(a.manifest)
    assert r['probe_sha256'] == state['probe_sha256'] == digest(a.probe)
    assert r['required_pairs'] == r['completed_pairs'] == len(r['results']) == 156
    assert r['status_counts'] == {'ok':156} and r['candidates'] == 0
    assert r['status'] == 'complete_measurement_not_full_library_qualification'
    image = next(v['right'] for v in m['positive_pairs'] if v['query_id'] == query)
    originals = [v for v in m['images']['original'] if v['group_id'] != image['group_id']]
    assert len(originals) == 156
    for row, original in zip(r['results'], originals):
        assert row['query_id'] == query+'/'+original['filename']
        assert row['label'] == 'different_publisher_origin'
        assert row['status'] == 'ok' and row['returncode'] == 0
        e = row['evidence']
        assert e['status'] == 'ok' and e['managed_used'] == 0
        assert type(e['managed_peak']) is int and 0 < e['managed_peak'] <= 64_000_000
        assert e['candidate'] is False and e['four_candidate'] is False
        for field, count in [('five_accepted_searches',5),('four_accepted_searches',4),('joined_accepted_searches',3)]:
            assert len(e[field]) == count and all(value is False for value in e[field])
        assert e['gradient_candidate'] is False and e['interpolated_candidate'] is False
        for prefix in ('gradient','interpolated'):
            counts=e[prefix+'_counts']
            if counts is not None:
                assert len(counts)>0
                accepted=True
                for matched,compared,source in counts:
                    assert all(type(value) is int for value in (matched,compared,source))
                    assert 0 <= matched <= compared <= source and source > 0
                    accepted &= compared>=1000 and compared>=source*.3 and matched>=compared*.9
                assert not accepted, (query,original['filename'],prefix)
        for input_image in (image,original):
            assert digest(Path(input_image['normalized_path'])) == input_image['normalized_sha256']
    assert report.read_bytes() == payload
    state['queries'].append({'query_id':query,'pairs':156,'candidates':0,
                             'report_sha256':hashlib.sha256(payload).hexdigest()})
    state['completed_pairs'] = sum(v['pairs'] for v in state['queries'])
    temporary = a.output.with_suffix('.tmp')
    temporary.write_text(json.dumps(state,indent=2)+'\n')
    temporary.replace(a.output)
    print(query,'156 verified; zero integrated five-search candidates',flush=True)
state['status'] = 'complete_nine_query_measurement_not_full_library_qualification'
a.output.write_text(json.dumps(state,indent=2)+'\n')
