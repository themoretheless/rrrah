#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / 'docs/research'
PRODUCER = DIRECTORY / 'dedup-ownerless-actual-engine-20261015.json'
report = json.loads(PRODUCER.read_text())
assert report['status'] == 'complete_selected_modules_pending_independent_audit'
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert sha(ROOT / 'scripts/qualify-dedup-ownerless-actual-engine-20261015.py') == report['producer_sha256']
work = Path(report['executable']).parent
for path, digest in report['source_pins'].items():
    assert sha(path) == digest
    if Path(path).name != 'decode.rs':
        assert sha(work / Path(path).name) == digest
decode_path, = [path for path in report['source_pins'] if Path(path).name == 'decode.rs']
decode = Path(decode_path).read_text()
fragment = decode[decode.index('/// Source observations retained'):decode.index('#[derive(Debug, thiserror::Error)]\npub enum FileError')]
assert (work / 'decode-observations.rs').read_text() == fragment
assert sha(work / 'decode-observations.rs') == report['decode_fragment_sha256']
assert sha(work / 'engine.rs') == report['harness_sha256']
for path, digest in report['dependency_pins'].items():
    assert sha(path) == digest
for row in report['commands']:
    assert row['returncode'] == 0 and sha(row['log']) == row['log_sha256']
assert sha(report['executable']) == report['executable_sha256']
fresh = subprocess.run([report['executable'], '--list'], capture_output=True, text=True)
assert fresh.returncode == 0
names = set(re.findall(r'^(.+): test$', fresh.stdout, re.M))
assert names == set(report['inventory'])
results = dict(re.findall(r'^test (.+) \.\.\. (ok|ignored[^\n]*)$', (work / 'run.log').read_text(), re.M))
assert names == set(results)
assert sum(value == 'ok' for value in results.values()) == report['passed']
assert sum(value.startswith('ignored') for value in results.values()) == report['ignored']
benchmark = 'collection::actual_collection_benchmark::complete_guarded_engine'
assert benchmark in names and results[benchmark].startswith('ignored')
assert sum(name.startswith('collection::ownerless_controls::') for name in names) == 2
out = DIRECTORY / 'dedup-ownerless-actual-engine-audit-20261015.json'
assert not out.exists()
out.write_text(json.dumps(dict(status='verified_complete_selected_module_inventory', producer_report_sha256=sha(PRODUCER), auditor_sha256=sha(__file__), executable=report['executable'], executable_sha256=report['executable_sha256'], passed=report['passed'], ignored=report['ignored'], failed=0, complete_inventory=sorted(names), benchmark=benchmark, scope=report['scope']), indent=2) + '\n')
print(report['passed'], 'selected-module tests passed; full-library gate remains required')
