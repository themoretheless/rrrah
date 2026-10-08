#!/usr/bin/env python3
"""Reject forged six-lane evidence while retaining native control inputs."""
import argparse
import copy
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('report', 'legacy', 'spatial', 'output'):
    p.add_argument(name, type=Path)
a = p.parse_args()
raw = a.report.read_bytes()
baseline = json.loads(raw)

def evidence(r):
    return r['results'][0]['evidence']

def region(r):
    return next(v for v in evidence(r)['spatial_gradient_regions']['regions'] if v['accepted_region'])

cases = [
    ('integer_region_flag', lambda r: region(r).__setitem__('accepted_region', 1)),
    ('impossible_pixel_counts', lambda r: region(r)['counts'][0].__setitem__(0, region(r)['counts'][0][2] + 1)),
    ('shifted_region', lambda r: region(r)['domains'][0].__setitem__(0, region(r)['domains'][0][0] + 1)),
    ('whole_promotion', lambda r: evidence(r)['spatial_gradient'].__setitem__('candidate', True)),
    ('legacy_decision_changed', lambda r: evidence(r).__setitem__('candidate', True)),
    ('excess_memory', lambda r: evidence(r).__setitem__('managed_peak', 64 * 1024 * 1024 + 1)),
    ('missing_result', lambda r: r['results'].pop()),
    ('duplicate_query', lambda r: r['results'].__setitem__(1, copy.deepcopy(r['results'][0]))),
    ('native_failure', lambda r: r['results'][0].__setitem__('returncode', 1)),
]
results = []
with tempfile.TemporaryDirectory(prefix='rrrah-six-audit-') as temp:
    for name, mutate in [('baseline', None), *cases]:
        data = copy.deepcopy(baseline)
        if mutate:
            mutate(data)
            # Mutations represent consistently forged native output, not merely
            # disagreement between the raw JSON string and its parsed copy.
            for row in data['results']:
                row['stdout'] = json.dumps(row['evidence']) + '\n'
        report = Path(temp) / (name + '.json')
        output = Path(temp) / (name + '-audit.json')
        report.write_text(json.dumps(data))
        run = subprocess.run(['python3', 'scripts/verify-dedup-six-region-controls.py',
                              str(report), str(a.legacy), str(a.spatial), str(output)], capture_output=True, text=True)
        accepted = run.returncode == 0 and output.exists()
        assert accepted == (mutate is None), (name, run.returncode, run.stderr)
        if mutate:
            assert not output.exists()
        results.append({'case': name, 'accepted': accepted, 'exit_code': run.returncode})
assert a.report.read_bytes() == raw
a.output.write_text(json.dumps({'status': 'verified_six_audit_adversaries',
    'report_sha256': hashlib.sha256(raw).hexdigest(), 'results': results,
    'scope': 'Report integrity only; no extra native accuracy measurement.'}, indent=2) + '\n')
