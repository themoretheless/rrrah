from pathlib import Path
import collections
import hashlib
import json
import re
import runpy
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
directory = root / 'docs/research'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert len(sys.argv) == 2 and sys.argv[1] in ('baseline', 'candidate')
variant = sys.argv[1]
report = directory / f'dedup-common-worker-fixed-{variant}-gate-v2-20261015.json'
data = json.loads(report.read_text())
assert data['status'] in ('terminal_failure', 'terminal_pending_inventory_audit')
assert sha(root / 'scripts/qualify-dedup-common-worker-fixed-pair-v2-20261015.py') == data['producer_sha256']
assert sha(data['manifest']) == data['manifest_sha256']
prepared = json.loads(Path(data['manifest']).read_text())
snapshot = next(row for row in prepared['variants'] if row['variant'] == variant)
assert data['source'] == snapshot['source']
for row in snapshot['source_files']:
    assert sha(Path(data['source']) / row['path']) == row['sha256']
original_path = directory / 'dedup-complete-collection-terminal-audit-20261015.json'
assert sha(original_path) == data['original_terminal_audit_sha256']
original = json.loads(original_path.read_text())
reference = json.loads(Path(original['producer_report']).read_text())
assert sha(original['producer_report']) == original['producer_report_sha256']


def target_name(path):
    return re.sub(r'-[0-9a-f]{16}$', '', Path(path).name)


expected = collections.Counter(target_name(row['original']) for row in reference['retained_binaries'])
actual = collections.Counter(target_name(row['original']) for row in data['retained_binaries'])
assert actual == expected, 'complete package target inventory changed or pruned'
assert sha(data['cargo_log']) == data['cargo_log_sha256']
body = Path(data['cargo_log']).read_text()
headers = list(re.finditer(r'^\s*Running .+ \((/[^\n]+)\)$', body, re.M))
retained = {row['original']: row for row in data['retained_binaries']}
assert headers and len(headers) == len(retained) and set(retained) == {h[1] for h in headers}
account = runpy.run_path(str(root / 'scripts/audit-dedup-complete-collection-terminal-20261015.py'))['account']
targets = []
for index, header in enumerate(headers):
    row = retained[header[1]]
    assert sha(row['copy']) == row['sha256']
    assert sha(row['inventory']) == row['inventory_sha256'] and row['inventory_returncode'] == 0
    names = set(re.findall(r'^(.+): test$', Path(row['inventory']).read_text(), re.M))
    fresh = subprocess.run([row['copy'], '--list'], capture_output=True, text=True)
    assert fresh.returncode == 0 and names == set(re.findall(r'^(.+): test$', fresh.stdout, re.M))
    section = body[header.end():headers[index + 1].start() if index + 1 < len(headers) else len(body)]
    targets.append(dict(binary=row['copy'], sha256=row['sha256'], **account(section, names)))
failed = sum(row['failed'] for row in targets)
assert (data['returncode'] != 0) == bool(failed)
output = directory / f'dedup-common-worker-fixed-{variant}-audit-v2-20261015.json'
assert not output.exists()
output.write_text(json.dumps(dict(
    status='verified_failed_complete_gate' if failed else 'verified_passing_complete_gate',
    gate_passed=not failed, manifest_sha256=data['manifest_sha256'],
    producer_report_sha256=sha(report), auditor_sha256=sha(__file__),
    inventory_parser_sha256=sha(root / 'scripts/audit-dedup-complete-collection-terminal-20261015.py'),
    targets=targets, passed=sum(row['passed'] for row in targets), failed=failed,
    ignored=sum(row['ignored'] for row in targets), scope=data['scope']), indent=2) + '\n')
print(variant, 'passed', sum(row['passed'] for row in targets), 'failed', failed)
