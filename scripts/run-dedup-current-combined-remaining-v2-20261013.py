#!/usr/bin/env python3
"""Preserve finished targets and continue after writable lifecycle fixture repair."""
import hashlib
import json
import os
import re
import shutil
import subprocess
from pathlib import Path

R = Path(__file__).resolve().parents[1]
T = Path('/Users/themoretheless/.codex/tmp')
D = R / 'docs/research'
S = T / 'rrrah-five-writable-tests-20261013'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
old_path = D / 'dedup-current-combined-remaining-20261013.json'
old = json.loads(old_path.read_text())
assert old['status'] == 'terminal_failure' and old['returncode'] == 101 and old['source_unchanged']
assert sha(old['log']) == old['log_sha256']
assert all(sha(Path(old['source']) / p) == h for p, h in old['source_sha256'].items())
changed = ['crates/rrrah-dedup/tests/' + n + '.rs' for n in
           ['five_lifecycle', 'five_region_lifecycle', 'five_gradient_region_lifecycle']]
assert all(sha(S / p) == h for p, h in old['source_sha256'].items() if p not in changed)
assert all(sha(S / p) == sha(R / p) for p in changed)
text = Path(old['log']).read_text()
blocks = re.split(r'(?=     Running tests/)', text)
completed = []
dest = T / 'rrrah-current-combined-remaining-prefix-qualified-20261013'
assert not dest.exists()
dest.mkdir()
for block in blocks:
    run = re.search(r'Running tests/(\w+)\.rs \(([^)]+)\)', block)
    summary = re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', block)
    if run and summary:
        name, binary = run.groups()
        passed, failed, ignored = map(int, summary.groups())
        assert failed == 0
        output = dest / name
        shutil.copy2(binary, output)
        assert sha(output) == sha(binary)
        completed.append(dict(target=name, passed=passed, ignored=ignored,
                              executable=str(output), executable_sha256=sha(output)))
assert completed and 'five_collection' in [r['target'] for r in completed]
prefix = D / 'dedup-current-combined-remaining-prefix-audit-20261013.json'
assert not prefix.exists()
prefix.write_text(json.dumps(dict(status='verified_finished_prefix', source=old['source'],
    source_sha256=old['source_sha256'], prior_report_sha256=sha(old_path),
    log_sha256=old['log_sha256'], targets=completed,
    passed=sum(r['passed'] for r in completed),
    scope='Only successful targets preceding five_gradient_region_lifecycle PermissionDenied; not complete103 or current root.'), indent=2)+'\n')
names = [n for n in old['targets'] if n not in {r['target'] for r in completed}]
assert 'five_gradient_region_lifecycle' in names
P = D / 'dedup-current-combined-remaining-v2-20261013.json'
L = T / 'rrrah-current-combined-remaining-v2-20261013.log'
assert not P.exists() and not L.exists()
pins = {p: sha(S / p) for p in old['source_sha256']}
cmd = ['cargo', 'test', '-p', 'rrrah-dedup', '--features', 'decode', '--offline', '--locked',
       '--no-fail-fast', '--jobs', '2', '--target-dir', str(T/'rrrah-cache-interpretation-fix-20261012/target')]
for name in names:
    cmd.extend(['--test', name])
data = dict(status='running', producer_pid=os.getpid(), source=str(S), source_sha256=pins,
    changed_tests=changed, targets=names, command=cmd, log=str(L), prior_prefix_audit=str(prefix),
    scope='Remaining targets after successful prefix; three fixture setup/restore repairs only. No production change relative prior snapshot; excludes subsequent exact/visual/local publication fixes, RAW/PFM additions and other platforms.')
def save():
    temp = P.with_suffix('.tmp')
    temp.write_text(json.dumps(data, indent=2)+'\n')
    temp.replace(P)
save()
with L.open('x') as output:
    child = subprocess.Popen(cmd, cwd=S, stdout=output, stderr=subprocess.STDOUT)
    data['child_pid'] = child.pid
    save()
    code = child.wait()
data.update(status='passed_pending_audit' if code == 0 else 'terminal_failure', returncode=code,
            log_sha256=sha(L), source_unchanged=all(sha(S/p)==h for p,h in pins.items()))
save()
print(data['status'])
