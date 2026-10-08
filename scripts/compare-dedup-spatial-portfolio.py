#!/usr/bin/env python3
"""Paired HPatches decisions; execution refusals are never candidate negatives."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

p = argparse.ArgumentParser()
for name in ('manifest', 'baseline', 'baseline_probe', 'spatial', 'spatial_probe', 'output'):
    p.add_argument(name, type=Path)
p.add_argument('--checkpoint', action='store_true')
a = p.parse_args()
captures = [path.read_bytes() for path in (a.baseline, a.spatial)]
reports = [json.loads(value) for value in captures]
manifest = json.loads(a.manifest.read_text())
assert reports[0]['feature_selection'] == 'portfolio_global_500_sampled_2048_anchor_1_and_raw'
assert reports[1]['feature_selection'] == 'portfolio_spatial_4x4_quota_32_max_500_sampled_2048_anchor_1_and_raw'
assert reports[0]['completed_pairs'] == 580
if not a.checkpoint:
    assert reports[1]['completed_pairs'] == 580
    assert reports[1]['status'] == 'complete_measurement_not_qualification'
assert 0 < reports[1]['completed_pairs'] <= 580
with tempfile.TemporaryDirectory(prefix='rrrah-spatial-paired-audit-') as directory:
    for index, (raw, report, probe) in enumerate(zip(captures, reports, (a.baseline_probe, a.spatial_probe))):
        snapshot = Path(directory) / f'report-{index}.json'
        audit = Path(directory) / f'audit-{index}.json'
        snapshot.write_bytes(raw)
        cp = subprocess.run(['python3', str(Path(__file__).with_name('verify-dedup-hpatches-report.py')),
                             str(a.manifest), str(snapshot), str(probe), str(audit)], capture_output=True, text=True)
        assert cp.returncode == 0, cp.stdout + cp.stderr
        for row, pair in zip(report['results'], manifest['pairs']):
            assert (row['sequence'], row['target_index']) == (pair['sequence'], pair['target_index'])
            assert row['status'] in ('ok', 'error', 'timeout', 'process_error')
counts = collections.Counter()
changes = []
for base, spatial in zip(reports[0]['results'], reports[1]['results']):
    key = (base['sequence'], base['target_index'])
    assert key == (spatial['sequence'], spatial['target_index'])
    if base['status'] != 'ok' or spatial['status'] != 'ok':
        category = ('execution_recovery' if spatial['status'] == 'ok' else
                    'new_execution_refusal' if base['status'] == 'ok' else 'both_execution_refusals')
    else:
        before, after = base['evidence']['candidate'], spatial['evidence']['candidate']
        category = ('candidate_retained' if before and after else 'candidate_gain' if after else
                    'candidate_loss' if before else 'both_reject')
    counts[category] += 1
    if category not in ('candidate_retained', 'both_reject'):
        changes.append({'sequence': key[0], 'target_index': key[1], 'category': category,
                        'baseline_status': base['status'], 'spatial_status': spatial['status'],
                        'baseline_stderr': base.get('stderr'), 'spatial_stderr': spatial.get('stderr')})
result = {'verified_pairs': reports[1]['completed_pairs'], 'required_pairs': 580,
          'complete': reports[1]['completed_pairs'] == 580, 'counts': dict(counts), 'changes': changes,
          'report_snapshot_sha256': [hashlib.sha256(raw).hexdigest() for raw in captures],
          'probe_sha256': [report['probe_sha256'] for report in reports],
          'managed_peak_max': [max(v.get('evidence', {}).get('managed_peak', 0) for v in report['results']) for report in reports],
          'scope': 'Paired fixed-policy candidate changes and explicit execution refusals. Published correspondence pairs are not duplicate truth. Managed peak excludes RSS/allocation completeness; no performance or default-promotion claim.'}
assert sum(counts.values()) == result['verified_pairs']
temporary = a.output.with_suffix('.tmp')
temporary.write_text(json.dumps(result, indent=2) + '\n')
temporary.replace(a.output)
print(json.dumps({k: result[k] for k in ('verified_pairs', 'required_pairs', 'complete', 'counts')}, indent=2))
