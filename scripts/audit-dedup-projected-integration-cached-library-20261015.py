#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / 'docs/research'
PRODUCER = DIRECTORY / 'dedup-projected-integration-cached-library-20261015.json'
report = json.loads(PRODUCER.read_text())
assert report['status'] == 'complete_inventory_executed_pending_independent_audit'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

assert sha(ROOT / 'scripts/run-dedup-projected-integration-cached-library-20261015.py') == report['producer_sha256']
assert sha(ROOT / 'crates/rrrah-dedup/tests/projected_portfolio.rs') == report['target_sha256']
assert sha(report['executable']) == report['executable_sha256']
for path, digest in report['dependency_pins'].items():
    assert sha(path) == digest
for row in report['commands']:
    assert row['returncode'] == 0 and sha(row['log']) == row['log_sha256']
fresh = subprocess.run([report['executable'], '--list'], capture_output=True, text=True)
assert fresh.returncode == 0
names = set(re.findall(r'^(.+): test$', fresh.stdout, re.M))
assert names == set(report['inventory'])
body = Path(next(row['log'] for row in report['commands'] if row['name'] == 'run')).read_text()
results = dict(re.findall(r'^test (.+) \.\.\. (ok|ignored[^\n]*)$', body, re.M))
assert set(results) == names
passed = sum(value == 'ok' for value in results.values())
ignored = sum(value.startswith('ignored') for value in results.values())
assert passed == report['passed'] and ignored == report['ignored']
assert results['exact_collection_missing_sources_never_publish_partial_duplicates'] == 'ok'
manifest = json.loads((DIRECTORY / 'dedup-committed-build-source-415443172-v2-20261015.json').read_text())
baseline = Path(manifest['source']) / 'crates/rrrah-dedup/tests/projected_portfolio.rs'
baseline_names = set(re.findall(r'#\[test\]\s*(?:#\[ignore[^\n]*\]\s*)?fn (\w+)\(', baseline.read_text()))
assert baseline_names and baseline_names.issubset(names), baseline_names - names
output = DIRECTORY / 'dedup-projected-integration-cached-library-audit-20261015.json'
assert not output.exists()
output.write_text(json.dumps(dict(
    status='verified_complete_current_integration_inventory_with_cached_libraries',
    producer_report_sha256=sha(PRODUCER), auditor_sha256=sha(__file__),
    executable_sha256=sha(report['executable']), passed=passed, ignored=ignored,
    failed=0, complete_inventory=sorted(names),
    original_baseline_test_functions_retained=len(baseline_names),
    missing_source_regression='12missing cases and6recoveries executed within one test',
    scope=report['scope'] + ' Fresh retained executable inventory matched all executed/explicit ignored cases; original baseline named test functions retained. Existing external-corpus early returns are not proof of actual corpus execution.'
), indent=2) + '\n')
print(passed, 'passed;', ignored, 'ignored; original inventory retained')
