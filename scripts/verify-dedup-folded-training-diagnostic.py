"""Qualify count parity and diagnostic arithmetic, not a pixel-fit oracle."""
import argparse
import hashlib
import json
import math
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('report', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
assert not args.output.exists()
digest = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
report = json.loads(args.report.read_text())
assert report['status'] == 'complete_native_diagnostic'
assert report['required_cases'] == len(report['rows']) == 7
assert all(digest(k) == v for k, v in report['input_hashes'].items())
base = args.report.parent
old = json.loads((base / 'dedup-folded-witness-common-footprint-regions.json').read_text())
audit = json.loads((base / 'dedup-folded-witness-common-footprint-regions-audit.json').read_text())
assert audit['report_sha256'] == digest(base / 'dedup-folded-witness-common-footprint-regions.json')
assert audit['status'] == 'verified_folded_witness_common_footprint_regions'
screen = json.loads((base / 'dedup-common-footprint-screen-control.json').read_text())
expected = [screen['evidence'], *[row['evidence'] for row in old['rows']]]
summaries = []
for index, (row, prior) in enumerate(zip(report['rows'], expected)):
    assert row['case'] == ('screen_control' if index == 0 else f'folded_{index-1}')
    assert row['returncode'] == 0 and row['original_evidence_parity'] is True
    actual = row['evidence']
    assert actual == json.loads(row['stdout'])
    assert actual['status'] == prior['status']
    assert actual['managed_used'] == 0
    assert actual['managed_peak'] == prior['managed_peak'] <= 512 * 1024 * 1024
    assert actual['footprints'] == prior['footprints'] == ['target', 'source']
    assert len(actual['directions']) == len(prior['directions'])
    directions = []
    for measured, reference in zip(actual['directions'], prior['directions']):
        assert all(measured[k] == v for k, v in reference.items())
        assert type(measured['training_matched']) is int
        assert 0 <= measured['training_matched'] <= measured['training_samples']
        assert measured['training_samples'] >= 1000 and measured['samples'] >= 1000
        assert 0 <= measured['matched'] <= measured['samples']
        for key in ['training_squared_error', 'heldout_squared_error']:
            assert math.isfinite(measured[key]) and measured[key] >= 0
        directions.append({
            'training_fraction': measured['training_matched'] / measured['training_samples'],
            'heldout_fraction': measured['matched'] / measured['samples'],
            'training_rms': math.sqrt(measured['training_squared_error'] / measured['training_samples']),
            'heldout_rms': math.sqrt(measured['heldout_squared_error'] / measured['samples']),
        })
    supported = len(directions) == 2 and all(d['heldout_fraction'] >= .9 for d in directions)
    assert supported == (True if index == 0 else old['rows'][index-1]['supported'])
    summaries.append({'case': row['case'], 'status': actual['status'],
                      'supported': supported, 'directions': directions})
assert all(digest(k) == v for k, v in report['input_hashes'].items())
args.output.write_text(json.dumps({
    'status': 'verified_training_diagnostic_parity',
    'cases': 7, 'screen_supported': summaries[0]['supported'],
    'folded_supports': sum(row['supported'] for row in summaries[1:]),
    'summary': summaries,
    'input_hashes': {str(p): digest(p) for p in [args.report, Path(__file__)]},
    'scope': 'Pinned terminal outputs, exact prior direction/sample/read/memory parity and finite diagnostic arithmetic. The four successful fold fits show similar training/heldout errors; two retain color-bound refusals. No independent color-fit/pixel resampling oracle, causal diagnosis or fold recovery.'
}, indent=2) + '\n')
