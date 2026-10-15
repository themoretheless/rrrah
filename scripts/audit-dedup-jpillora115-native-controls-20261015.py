#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / 'docs/research/dedup-500'
REPORT = DIRECTORY / 'jpillora115-native-controls-audit-20261015.json'
report = json.loads(REPORT.read_text())
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
prepared = DIRECTORY / 'jpillora115-native-prepared-20261015.json'
assert sha(prepared) == report['prepared_manifest_sha256']
source = Path(json.loads(prepared.read_text())['source'])
for name, digest in report['source_pins'].items():
    assert sha(source / name) == digest
assert sha(source / 'rrrah_test.go') == report['test_sha256']
assert sha(DIRECTORY / 'jpillora115-native-controls-20261015.go') == report['test_sha256']
assert sha(report['executable']) == report['executable_sha256']
log = DIRECTORY / 'jpillora115-native-controls-20261015.log'
assert sha(log) == report['log_sha256']
listed = subprocess.run([report['executable'], '-test.list', '.'], capture_output=True, text=True)
assert listed.returncode == 0
names = listed.stdout.splitlines()
assert len(names) == len(set(names)) == 6
assert set(names) == set(report['fresh_inventory']) == set(report['passed'])
assert set(re.findall(r'^--- PASS: (\w+)', log.read_text(), re.M)) == set(names)
assert log.read_text().endswith('PASS\n')
sumlines = set((source / 'go.sum').read_text().splitlines())
assert len(report['compiled_modules']) == 8
for row in report['compiled_modules']:
    assert f"{row['module']} {row['version']} {row['checksum']}" in sumlines
assert report['module_verification'] == 'all modules verified'
OUTPUT = DIRECTORY / 'jpillora115-native-independent-audit-20261015.json'
assert not OUTPUT.exists()
OUTPUT.write_text(json.dumps(dict(status='verified6_complete_native_control_inventory_and8compiled_module_checksums', auditor_sha256=sha(__file__), producer_report_sha256=sha(REPORT), executable_sha256=report['executable_sha256'], passed=6, failed=0, complete_inventory=names, scope=report['scope'] + ' Independent fresh binary inventory, all executed names, original source/test pins and linked module sums verified; broad module graph remains outside scope.'), indent=2) + '\n')
print('6native controls and8linked module sums independently verified')
