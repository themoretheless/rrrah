from pathlib import Path
import hashlib
import json
import re
import subprocess

root = Path(__file__).resolve().parents[1]
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
reports = {}
for label, report_name in [('391', 'perceptive391-complete-source-review-20261015.json'),
                           ('467', 'pyrovski467-complete-source-review-20261015.json')]:
    report = root / 'docs/research' / report_name
    data = json.loads(report.read_text())
    for row in data['source_files']:
        raw = (root / row['local']).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == row['sha256']
        assert hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest() == row['git_blob']
    reports[label] = dict(data=data, sha256=sha(report))
work391 = Path('/private/tmp/rrrah-perceptive391-native-20261015')
for row in reports['391']['data']['source_files']:
    assert sha(work391 / row['path']) == row['sha256']
work467 = Path('/private/tmp/rrrah-pyrovski467-unpack-20261015')
source467 = (root / 'docs/research/dedup-500/evidence/pyrovski--phash/02-phash.go').read_text()
helper = source467[source467.index('func unpackHash('):source467.index('\n// storeHashes')]
assert (work467 / 'unpack.go').read_text().endswith(helper)
for work, authored in [(work391, 'perceptive391-controls-20261015.go'),
                        (work467, 'pyrovski467-unpack-controls-20261015.go')]:
    assert (work / 'controls_test.go').read_bytes() == (root / 'docs/research' / authored).read_bytes()
expected_failures = {'TestCompareImagesWithAverageHash', 'TestAhash'}
specs = [(work391 / 'authored.test', work391, 6, set()),
         (work391 / 'upstream.test', work391 / 'perceptive_test', 5, expected_failures),
         (work467 / 'controls.test', work467, 4, set())]
results = []
for binary, cwd, count, failures in specs:
    listed = subprocess.run([str(binary), '-test.list', '.'], cwd=cwd, capture_output=True, text=True)
    assert listed.returncode == 0
    names = set(re.findall(r'^Test\S+$', listed.stdout, re.M))
    assert len(names) == count
    run = subprocess.run([str(binary), '-test.v', '-test.timeout=30s'], cwd=cwd, capture_output=True, text=True)
    passed = set(re.findall(r'^--- PASS: (\S+) ', run.stdout, re.M))
    failed = set(re.findall(r'^--- FAIL: (\S+) ', run.stdout, re.M))
    assert passed | failed == names and not passed & failed
    assert failed == failures and (run.returncode != 0) == bool(failures)
    assert not re.search(r'^--- SKIP:', run.stdout, re.M)
    if failures:
        assert 'distance got 26 want 27' in run.stdout
        assert '3492678383517052928 want 4069139135820476416' in run.stdout
    results.append(dict(binary=str(binary), sha256=sha(binary), cwd=str(cwd),
                        complete_inventory=sorted(names), passed=sorted(passed),
                        failed=sorted(failed), returncode=run.returncode, log=run.stdout))
output = root / 'docs/research/perceptive391-pyrovski467-native-independent-audit-20261015.json'
assert not output.exists()
output.write_text(json.dumps(dict(
    status='verified13passes_and2preserved_upstream_failures', auditor_sha256=sha(__file__),
    source_reports={key: value['sha256'] for key, value in reports.items()}, results=results,
    scope='Actual391 dependency binaries and selected467 unpack helper. Fifteen named cases fully accounted; two failed original391goldens retained. No original391package green claim, no467OpenCV/SQLite execution, no globalquality/platform/adoption qualification.'), indent=2) + '\n')
print('Verified13passes and2originalfailures; no result promoted to success')
