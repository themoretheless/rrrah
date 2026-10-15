from pathlib import Path
import hashlib
import json
import re
import subprocess

root = Path(__file__).resolve().parents[1]
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
specs = [
    ('dedupe209', 'dedupe209-complete-source-review-20261015.json',
     '/private/tmp/rrrah-dedupe209-native-20261015', 'controls.test', 8,
     'dedupe209-controls-20261015.go', 'controls_test.go'),
    ('imgsum389', 'imgsum389-complete-source-review-20261015.json',
     '/private/tmp/rrrah-imgsum389-native-20261015', 'image.test', 6,
     'imgsum389-image-controls-20261015.go', 'image/controls_test.go'),
]
results = []
for name, report_name, folder, binary_name, count, authored_name, copied_name in specs:
    report = root / 'docs/research' / report_name
    data = json.loads(report.read_text())
    work = Path(folder)
    for row in data['source_files']:
        raw = (root / row['local']).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == row['sha256']
        assert hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest() == row['git_blob']
        assert (work / row['path']).read_bytes() == raw
    assert (work / copied_name).read_bytes() == (root / 'docs/research' / authored_name).read_bytes()
    binary = work / binary_name
    listed = subprocess.run([str(binary), '-test.list', '.'], capture_output=True, text=True)
    assert listed.returncode == 0
    names = set(re.findall(r'^Test\S+$', listed.stdout, re.M))
    assert len(names) == count
    run = subprocess.run([str(binary), '-test.v', '-test.timeout=30s'], capture_output=True, text=True)
    assert run.returncode == 0
    assert names == set(re.findall(r'^--- PASS: (\S+) ', run.stdout, re.M))
    assert not re.search(r'^--- (SKIP|FAIL):', run.stdout, re.M)
    results.append(dict(repository=data['repository'], source_report_sha256=sha(report),
                        binary=str(binary), binary_sha256=sha(binary),
                        complete_test_inventory=sorted(names), log=run.stdout))
output = root / 'docs/research/dedupe209-imgsum389-native-independent-audit-20261015.json'
assert not output.exists()
output.write_text(json.dumps(dict(status='verified_fourteen_retained_native_test_replays',
                                  auditor_sha256=sha(__file__), results=results,
                                  scope='Original source blobs and copied authored controls verified; actual pinned dependency binaries. Eight209tests and original389TestAll plus five389controls. No wholeCLI/RAW corpus/network/platform or globalquality proof.'), indent=2) + '\n')
print('Verified 14 complete native test replays and original source pins')
