#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import re

ROOT = Path(__file__).resolve().parents[1]
RESEARCH = ROOT / 'docs/research'
P = RESEARCH / 'dedup-current-test-metadata-v2-20261015.json'
report = json.loads(P.read_text())

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

assert report['status'] == 'strict_metadata_passed'
assert not report['failed_targets'] and not report['source_changes']
prior_path = ROOT / report['prior_report']
assert sha(prior_path) == report['prior_report_sha256']
prior = json.loads(prior_path.read_text())
assert sha(ROOT / 'scripts/check-dedup-current-test-metadata-v1-20261015.py') == prior['producer_sha256']
assert sha(ROOT / 'scripts/recheck-dedup-current-test-metadata-v2-20261015.py') == report['producer_sha256']
assert len(report['reused_passed_rows']) == 147 and len(report['rerun_rows']) == 9
rows = report['reused_passed_rows'] + report['rerun_rows']
assert len(rows) == len({row['target'] for row in rows}) == 156
assert {row['target'] for row in rows} == {'production_library'} | {target['name'] for target in report['planned_targets']}
for row in rows:
    assert row['returncode'] == 0 and sha(row['log']) == row['log_sha256']
    assert '-Dwarnings' in row['command'] and '--deny=unsafe_code' in row['command']
for path, digest in report['source_pins'].items():
    assert sha(ROOT / path) == digest
for path, digest in report['dependency_pins'].items():
    assert sha(path) == digest
production = report['reused_passed_rows'][0]['command']
assert sha(production[production.index('-o') + 1]) == report['production_metadata_sha256']
fixture_pins = {}
for path in ['crates/rrrah-dedup/tests/fixtures/joint-geometry-204100.bits.rs', 'crates/rrrah-dedup/tests/fixtures/joint-geometry-211900.bits.rs']:
    assert sha(ROOT / path) == prior['source_pins'][path]
    fixture_pins[path] = sha(ROOT / path)
backup = Path('/private/tmp/rrrah-strict-test-warning-cleanup-20261015')
for path in report['changed_test_sources']:
    before = (backup / Path(path).name).read_text()
    after = (ROOT / path).read_text()
    assert before.count('#[test]') == after.count('#[test]')
    assert re.findall(r'\bfn\s+(\w+)', before) == re.findall(r'\bfn\s+(\w+)', after)
OUTPUT = RESEARCH / 'dedup-current-test-metadata-v2-audit-20261015.json'
assert not OUTPUT.exists()
OUTPUT.write_text(json.dumps({
    'status': 'verified156_strict_metadata_targets_not_runtime',
    'producer_report': str(P.relative_to(ROOT)), 'producer_report_sha256': sha(P),
    'auditor_sha256': sha(__file__), 'accounted_targets': 156,
    'retained_successes': 147, 'rerun_successes': 9,
    'source_changes_during_checks': [], 'historical_fixture_pins': fixture_pins,
    'declared_test_functions_preserved_in_changed_sources': True,
    'required_feature_skips': report['required_feature_skips'],
    'scope': report['scope']
}, indent=2) + '\n')
print('156 strict metadata targets independently accounted; historical captures retained')
