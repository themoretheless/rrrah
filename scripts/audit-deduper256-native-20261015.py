from pathlib import Path
import hashlib
import json
import re
import subprocess

root = Path(__file__).resolve().parents[1]
report = root / 'docs/research/deduper256-complete-source-review-20261015.json'
data = json.loads(report.read_text())
work = Path('/private/tmp/rrrah-deduper256-native-20261015')
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
for row in data['source_files']:
    raw = (root / row['local']).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == row['sha256']
    assert hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest() == row['git_blob']
    assert (work / row['path']).read_bytes() == raw
assert (work / 'controls_test.go').read_bytes() == (root / 'docs/research/deduper256-controls-20261015.go').read_bytes()
targets = []
for name, count in [('root.test', 7), ('go1.test', 3)]:
    binary = work / name
    inventory = subprocess.run([str(binary), '-test.list', '.'], capture_output=True, text=True)
    assert inventory.returncode == 0
    names = set(re.findall(r'^Test\S+$', inventory.stdout, re.M))
    assert len(names) == count
    execution = subprocess.run([str(binary), '-test.v', '-test.timeout=30s'], capture_output=True, text=True)
    assert execution.returncode == 0
    assert set(re.findall(r'^--- PASS: (\S+) ', execution.stdout, re.M)) == names
    assert not re.search(r'^--- (FAIL|SKIP):', execution.stdout, re.M)
    targets.append(dict(binary=str(binary), sha256=sha(binary), inventory=sorted(names), log=execution.stdout))
race = (work / 'race.log').read_text()
assert 'WARNING: DATA RACE' not in race
assert len(re.findall(r'^--- PASS:', race, re.M)) == 10
assert len(re.findall(r'^ok\s+github.com/tonalfitness/deduper(?:/deduper_go1)?\s', race, re.M)) == 2
output = root / 'docs/research/deduper256-native-independent-audit-20261015.json'
assert not output.exists()
output.write_text(json.dumps(dict(
    status='verified_ten_retained_binary_replays_and_race_instrumented_tests',
    producer_report_sha256=sha(report), auditor_sha256=sha(__file__),
    targets=targets, race_log_sha256=sha(work / 'race.log'),
    scope='Actual unchanged original Go implementations, all six upstream tests and four authored helper controls. Race detector observed no race in this execution; not proof of all schedules, cancellation, bounded memory or production adoption.'), indent=2) + '\n')
print('Verified ten tests, all source pins, and race-instrumented execution')
