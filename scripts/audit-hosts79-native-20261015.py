"""Replay all synthetic Hosts-BL controls against retained native executables."""
from pathlib import Path
import hashlib
import json
import re
import subprocess

root = Path(__file__).resolve().parents[1]
report = root / 'docs/research/hosts79-complete-source-review-20261015.json'
data = json.loads(report.read_text())
work = Path('/private/tmp/rrrah-hosts79-native-20261015')
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
for row in data['source_files']:
    raw = (root / row['local']).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == row['sha256']
    assert hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest() == row['git_blob']
    assert (work / row['path']).read_bytes() == raw
assert (work / 'controls_test.go').read_bytes() == (root / 'docs/research/hosts79-controls-20261015.go').read_bytes()
assert sha(work / 'hosts-bl') == data['executable_sha256']
binary = work / 'controls.test'
listing = subprocess.run([str(binary), '-test.list', '.'], capture_output=True, text=True)
assert listing.returncode == 0
names = set(re.findall(r'^Test\S+$', listing.stdout, re.M))
assert len(names) == 5
run = subprocess.run([str(binary), '-test.v'], capture_output=True, text=True)
assert run.returncode == 0
assert set(re.findall(r'^--- PASS: (\S+) ', run.stdout, re.M)) == names
assert not re.search(r'^--- (SKIP|FAIL):', run.stdout, re.M)
for row in data['native_controls']:
    command = row['command']
    inp, out = Path(command[-2]), Path(command[-1])
    assert inp.parent == out.parent == work
    assert sha(inp) == row['input_sha256']
    # Replay to fresh owned output, preserving the original output evidence.
    replay = out.with_suffix('.replay')
    assert not replay.exists()
    result = subprocess.run(command[:-1] + [str(replay)], capture_output=True, text=True)
    assert result.returncode == 0 and replay.read_text() == row['output']
    assert sha(replay) == row['output_sha256'] == sha(out)
output = root / 'docs/research/hosts79-native-independent-audit-20261015.json'
assert not output.exists()
output.write_text(json.dumps(dict(
    status='verified_five_complete_native_tests_and_four_cli_replays',
    producer_report_sha256=sha(report), auditor_sha256=sha(__file__),
    test_binary_sha256=sha(binary), complete_test_inventory=sorted(names),
    test_log=run.stdout, cli_controls=4,
    scope='Original unchanged Go source; synthetic data and injected hash collision. No natural collision, image/file identity, scaling or other-platform proof.'), indent=2) + '\n')
print('Verified 5 retained-binary tests and 4 fresh CLI replays')
