#!/usr/bin/env python3
"""Exercise the negative evidence auditor with deliberately corrupted reports."""
import argparse
import copy
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('manifest', 'probe', 'report', 'output'):
    p.add_argument(name, type=Path)
a = p.parse_args()
raw = a.report.read_bytes()
baseline = json.loads(raw)
assert baseline['status'] == 'complete' and len(baseline['results']) == 156

def evidence(r):
    return r['results'][0]['evidence']

cases = [
    ('integer_candidate', lambda r: evidence(r).__setitem__('candidate', 0)),
    ('changed_whole_decision', lambda r: evidence(r).__setitem__('candidate', True)),
    ('negative_inliers', lambda r: evidence(r).__setitem__('inliers', -1)),
    ('invented_region_support', lambda r: evidence(r).__setitem__('region_support_count', 1)),
    ('missing_result', lambda r: r['results'].pop()),
    ('duplicated_pair', lambda r: r['results'].__setitem__(1, copy.deepcopy(r['results'][0]))),
    ('failed_native_execution', lambda r: r['results'][0].__setitem__('returncode', 1)),
    ('changed_probe_pin', lambda r: r.__setitem__('probe_sha256', '0' * 64)),
]
results = []
with tempfile.TemporaryDirectory(prefix='rrrah-spatial-negative-audit-') as temp:
    for name, mutate in [('baseline', None), *cases]:
        data = copy.deepcopy(baseline)
        if mutate:
            mutate(data)
        report = Path(temp) / (name + '.json')
        output = Path(temp) / (name + '-audit.json')
        report.write_text(json.dumps(data))
        run = subprocess.run(['python3', 'scripts/verify-dedup-spatial-region-negatives.py',
                              str(a.manifest), str(a.probe), str(report), baseline['query'],
                              str(output)], capture_output=True, text=True)
        accepted = run.returncode == 0 and output.exists()
        assert accepted == (mutate is None), (name, run.returncode, run.stderr)
        if mutate:
            assert not output.exists(), name
        results.append({'case': name, 'accepted': accepted, 'exit_code': run.returncode})
assert a.report.read_bytes() == raw
a.output.write_text(json.dumps({'status': 'verified_audit_adversaries',
    'report_sha256': hashlib.sha256(raw).hexdigest(), 'results': results,
    'scope': 'Report integrity only; no additional native accuracy measurement.'}, indent=2) + '\n')
