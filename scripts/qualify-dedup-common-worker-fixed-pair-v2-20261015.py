"""Run complete package gates sequentially on the prepared immutable source pair."""
from pathlib import Path
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
directory = root / 'docs/research'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert len(sys.argv) == 2 and sys.argv[1] in ('baseline', 'candidate')
variant = sys.argv[1]
manifest = directory / 'dedup-common-worker-fixed-collection-pair-v2-prepared-20261015.json'
prepared = json.loads(manifest.read_text())
snapshot = next(row for row in prepared['variants'] if row['variant'] == variant)
source = Path(snapshot['source'])
original_audit = directory / 'dedup-complete-collection-terminal-audit-20261015.json'
original = json.loads(original_audit.read_text())
assert original['status'] == 'verified_failed_terminal_inventory'
assert original['failed'] == 6
assert {name for target in original['targets'] for name in target['failures']} == {
    'model_worker::tests::successful_worker_response_does_not_leave_background_process',
    'group_callback_panic_still_stops_descendant',
    'group_cancel_stops_live_descendant_and_retry_succeeds',
    'group_capture_limit_stops_descendant',
    'group_deadline_stops_live_descendant',
    'group_normal_and_failed_leader_exit_stop_descendants',
}
if variant == 'candidate':
    baseline_audit = directory / 'dedup-common-worker-fixed-baseline-audit-v2-20261015.json'
    baseline = json.loads(baseline_audit.read_text())
    assert baseline['gate_passed'] and baseline['manifest_sha256'] == sha(manifest)
for row in snapshot['source_files']:
    assert sha(source / row['path']) == row['sha256']
work = Path('/private/tmp') / f'rrrah-common-worker-fixed-{variant}-gate-v2-20261015'
output = directory / f'dedup-common-worker-fixed-{variant}-gate-v2-20261015.json'
assert not output.exists() and not work.exists()
work.mkdir()
command = ['cargo', '+stable', 'test', '--locked', '--offline',
           '-p', 'rrrah-decode', '-p', 'rrrah-dedup', '--features', 'rrrah-dedup/decode',
           '--lib', '--tests', '--no-fail-fast', '--jobs', '2',
           '--config', 'profile.test.lto="off"',
           '--config', 'profile.test.package.rrrah-dedup.debug=0',
           '--config', 'profile.test.package.rrrah-decode.debug=0',
           '--target-dir', '/private/tmp/rrrah-consensus-collection-target-20261014']
data = dict(status='starting', variant=variant, producer_pid=os.getpid(),
            producer_sha256=sha(__file__), manifest=str(manifest), manifest_sha256=sha(manifest),
            original_terminal_audit_sha256=sha(original_audit), source=str(source), command=command,
            scope='Complete lib/integration targets of both frozen packages, decode enabled, no filters or pruning. '
                  'Shared test-only worker overlay. Original optlevels/assertions retained; only these two packages '
                  'omit debug symbols equally, noLTO diagnostic MacARM. Not latest mutable Root/allformats/platform proof.')


def save():
    temporary = output.with_suffix('.tmp')
    temporary.write_text(json.dumps(data, indent=2) + '\n')
    temporary.replace(output)


save()
log = work / 'cargo.log'
with log.open('x') as stream:
    child = subprocess.Popen(command, cwd=source, stdout=stream, stderr=subprocess.STDOUT)
    data.update(status='running', child_pid=child.pid)
    save()
    code = child.wait()
data.update(status='retaining_terminal_inventory', returncode=code,
            cargo_log=str(log), cargo_log_sha256=sha(log))
save()
for row in snapshot['source_files']:
    assert sha(source / row['path']) == row['sha256']
paths = re.findall(r'^\s*Running .+ \((/[^\n]+)\)$', log.read_text(), re.M)
data['retained_binaries'] = []
for index, path in enumerate(dict.fromkeys(paths)):
    binary = Path(path)
    copy = work / f'binary-{index}-{binary.name}'
    shutil.copyfile(binary, copy)
    copy.chmod(0o755)
    assert sha(binary) == sha(copy)
    listed = subprocess.run([str(copy), '--list'], capture_output=True, text=True)
    inventory = work / f'inventory-{index}.log'
    inventory.write_text(listed.stdout + listed.stderr)
    data['retained_binaries'].append(dict(original=path, copy=str(copy), sha256=sha(copy),
                                         inventory=str(inventory), inventory_sha256=sha(inventory),
                                         inventory_returncode=listed.returncode))
data['status'] = 'terminal_failure' if code else 'terminal_pending_inventory_audit'
save()
print(variant, code, 'retained', len(data['retained_binaries']), 'binaries')
raise SystemExit(code)
