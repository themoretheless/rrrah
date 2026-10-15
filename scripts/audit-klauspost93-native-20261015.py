from pathlib import Path
import hashlib
import json
import re
import subprocess

root = Path(__file__).resolve().parents[1]
report = root / 'docs/research/klauspost93-complete-source-review-20261015.json'
data = json.loads(report.read_text())
work = Path('/private/tmp/rrrah-klauspost93-full-20261015')
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
for row in data['source_files']:
    raw = (root / row['local']).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == row['sha256']
    assert hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest() == row['git_blob']
    assert (work / row['path']).read_bytes() == raw
for path, digest in data['pins'].items():
    assert sha(path) == digest
assert (work / 'controls_test.go').read_bytes() == (root / 'docs/research/klauspost93-selected-controls-20261015.go').read_bytes()
binary = work / 'all-controls.test'
listed = subprocess.run([str(binary), '-test.list', '.*'], capture_output=True, text=True)
assert listed.returncode == 0
names = set(re.findall(r'^(?:Test|Example)\S+$', listed.stdout, re.M))
assert names == set(data['complete_executed_inventory']) and len(names) == 29
run = subprocess.run([str(binary), '-test.v', '-test.timeout=60s'], cwd=work, capture_output=True, text=True)
assert run.returncode == 0
assert names == set(re.findall(r'^--- PASS: (\S+) ', run.stdout, re.M))
assert not re.search(r'^--- (SKIP|FAIL):', run.stdout, re.M)
for row in data['source_files']:
    assert sha(work / row['path']) == row['sha256']
output = root / 'docs/research/klauspost93-native-independent-audit-20261015.json'
assert not output.exists()
output.write_text(json.dumps(dict(
    status='verified_complete29native_replays_and13original_blob_pins',
    source_review_sha256=sha(report), auditor_sha256=sha(__file__),
    binary_sha256=sha(binary), complete_inventory=sorted(names), log=run.stdout,
    scope='Actual original modules and public ZIP fixture. All15upstreamtests10examples+4authoredcontrols replayed withoutfilters/skips. TestBirthdayProblem is printonly. No benchmark/race/fuzz/hardresources/platform/security or globalgoal completeness proof.'), indent=2) + '\n')
print('Verified29native replays and13original source blobs')
