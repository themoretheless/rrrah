"""Audit complete exact-test inventory without treating a running build as success."""
from pathlib import Path
import collections
import hashlib
import json
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
prepared = ROOT / 'docs/research/dedup-exact-read-offsets-prepared-20261015.json'
plan = json.loads(prepared.read_text())
work = Path(plan['source'])
for path, expected in plan['pins'].items():
    assert sha(path) == expected, path
for path, expected in plan['root_source_pins'].items():
    assert sha(ROOT / path) == expected, path
binary = work / 'tests'
assert binary.is_file(), 'test binary not ready'
log = (work / 'tests.log').read_text()
assert re.search(r'test result: ok\. \d+ passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;', log), 'complete passing terminal result required'
inventory = subprocess.run([str(binary), '--list'], capture_output=True, text=True, check=True).stdout
names = [line[:-6] for line in inventory.splitlines() if line.endswith(': test')]
assert names and len(names) == len(set(names))
results = re.findall(r'^test (\S+) \.\.\. (ok|FAILED|ignored)$', log, re.M)
assert collections.Counter(name for name, _ in results) == collections.Counter(names)
assert all(status == 'ok' for _, status in results)
assert any(name.endswith('every_partial_read_failure_preserves_typed_error_and_unread_tail') for name in names)
summary = re.search(r'test result: ok\. (\d+) passed;', log)
assert int(summary[1]) == len(names)
dependencies = {}
for index, arg in enumerate(plan['command']):
    if arg == '--extern':
        path = plan['command'][index + 1].split('=', 1)[1]
        dependencies[path] = sha(path)
prior = json.loads((ROOT / 'docs/research/dedup-root-exact-combined-20261014.json').read_text())
for path, digest in dependencies.items():
    assert prior['dependency_pins'].get(path) == digest, path
output = ROOT / 'docs/research/dedup-exact-read-offsets-audit-20261015.json'
assert not output.exists()
output.write_text(json.dumps({
    'status': 'verified_complete_exact_module_and_file_integrations',
    'prepared': str(prepared.relative_to(ROOT)),
    'prepared_sha256': sha(prepared),
    'auditor_sha256': sha(__file__),
    'passed': len(names), 'failed': 0, 'ignored': 0,
    'new_read_failure_controls': 192,
    'inventory': names,
    'binary': str(binary), 'binary_sha256': sha(binary),
    'dependencies': dependencies,
    'output_pins': {str(work / name): sha(work / name) for name in ['build.log', 'inventory.log', 'tests.log']},
    'scope': 'Current complete exact.rs and exact_files.rs plus three prior large-file synthetic controls with pinned real cached dependencies. Read failures at 64 offsets for three error kinds preserve typed error and unread tail. Not fresh Cargo/full library/decode/platform/corpus or performance qualification.',
}, indent=2) + '\n')
print(output, len(names))
