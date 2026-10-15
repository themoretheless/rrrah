"""Check correctness and source identity before the actual collection benchmark.

This does not compile, run, or authorize timing under competing heavy work.
"""
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
pair_path = D / 'dedup-common-worker-fixed-collection-pair-v2-prepared-20261015.json'
pair = json.loads(pair_path.read_text())
for variant in ('baseline', 'candidate'):
    path = D / f'dedup-common-worker-fixed-{variant}-audit-v2-20261015.json'
    assert path.is_file(), f'{variant} complete audited gate not ready'
    audit = json.loads(path.read_text())
    assert audit['gate_passed'] is True and audit['failed'] == 0
    assert audit['status'] == 'verified_passing_complete_gate'
    assert audit['manifest_sha256'] == sha(pair_path)
    report_path = D / f'dedup-common-worker-fixed-{variant}-gate-v2-20261015.json'
    assert sha(report_path) == audit['producer_report_sha256']
    assert sha(ROOT / 'scripts/audit-dedup-common-worker-fixed-pair-v2-20261015.py') == audit['auditor_sha256']
    assert len(audit['targets']) == 162
    snapshot = next(row for row in pair['variants'] if row['variant'] == variant)
    for row in snapshot['source_files']:
        assert sha(Path(snapshot['source']) / row['path']) == row['sha256'], row['path']

benchmark_path = D / 'dedup-actual-collection-benchmark-v1-prepared-20261015.json'
benchmark = json.loads(benchmark_path.read_text())
original_path = Path(benchmark['baseline_manifest'])
assert sha(original_path) == benchmark['baseline_manifest_sha256']
original = json.loads(original_path.read_text())
expected = {row['path']: row['sha256'] for row in original['source_files']}
expected.update({row['path']: row['candidate_sha256'] for row in benchmark['changed']})
assert len(expected) == 3378
for path, digest in expected.items():
    assert sha(Path(benchmark['source']) / path) == digest, path
integration_path = D / 'dedup-selective-collection-full-v2-prepared-20261015.json'
assert sha(integration_path) == benchmark['integration_candidate_manifest_sha256']
integration = json.loads(integration_path.read_text())
candidate = next(row for row in pair['variants'] if row['variant'] == 'candidate')
candidate_pins = {row['path']: row['sha256'] for row in candidate['source_files']}
baseline = next(row for row in pair['variants'] if row['variant'] == 'baseline')
baseline_pins = {row['path']: row['sha256'] for row in baseline['source_files']}
assert set(pair['only_candidate_differences']) == {
    'crates/rrrah-dedup/src/exact.rs', 'crates/rrrah-dedup/src/collection.rs'
}
for row in integration['changed']:
    assert candidate_pins[row['path']] == row['candidate_sha256']
    assert baseline_pins[row['path']] == row['baseline_sha256']
print('Complete paired correctness and benchmark source identity verified; timing still requires quiescent owned heavy work and opt3 build.')
