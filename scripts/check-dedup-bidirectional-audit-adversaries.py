#!/usr/bin/env python3
"""Verify rejection of corrupted native evidence, without rerunning measurements."""
import argparse
import copy
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('report', type=Path)
p.add_argument('output', type=Path)
a = p.parse_args()
raw = a.report.read_bytes()
baseline = json.loads(raw)
assert baseline['status'] == 'complete' and len(baseline['results']) == 5

def region(r):
    row = r['results'][0]
    lane_index = next(i for i, lane in enumerate(row['gradient_regions']) if lane is not None and lane['region_support_count'])
    index = next(i for i, value in enumerate(row['gradient_regions'][lane_index]['regions']) if value['accepted_region'])
    return [row[key][lane_index]['regions'][index] if key == 'gradient_regions'
            else row[key]['gradient_regions'][lane_index]['regions'][index]
            for key in ('gradient_regions', 'native_collection_evidence')]

def corrupt_flag(r):
    for value in region(r):
        value['accepted_region'] = 1

def corrupt_counts(r):
    for value in region(r):
        value['counts'][0][0] = value['counts'][0][2] + 1

def corrupt_domain(r):
    for value in region(r):
        value['domains'][0][0] += 1

cases = [('integer_accepted_flag', corrupt_flag),
         ('matched_count_exceeds_source', corrupt_counts),
         ('shifted_native_domain', corrupt_domain),
         ('missing_result', lambda r: r['results'].pop()),
         ('duplicated_query', lambda r: r['results'].__setitem__(1, copy.deepcopy(r['results'][0]))),
         ('changed_whole_decision', lambda r: r['results'][0]['native_collection_evidence'].__setitem__('candidate', True))]
results = []
with tempfile.TemporaryDirectory(prefix='rrrah-bidirectional-audit-') as temp:
    for name, mutate in [('baseline', None), *cases]:
        data = copy.deepcopy(baseline)
        if mutate:
            mutate(data)
        report, output = Path(temp) / (name + '.json'), Path(temp) / (name + '-audit.json')
        report.write_text(json.dumps(data))
        run = subprocess.run(['python3', 'scripts/verify-dedup-bidirectional-regions.py', str(report), str(output)], capture_output=True, text=True)
        accepted = run.returncode == 0 and output.exists()
        assert accepted == (mutate is None), (name, run.returncode, run.stderr)
        if mutate:
            assert not output.exists(), name
        results.append({'case': name, 'accepted': accepted, 'exit_code': run.returncode})
assert a.report.read_bytes() == raw
a.output.write_text(json.dumps({'status': 'verified_audit_adversaries', 'report_sha256': hashlib.sha256(raw).hexdigest(),
                               'results': results, 'scope': 'Report-integrity checks; no new native accuracy or performance measurement.'}, indent=2) + '\n')
