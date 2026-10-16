"""Execute predeclared native controls; preserve drift/failure as unqualified evidence."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / '.dedup-work/current-loftr-trace-controls-v2-20261016'
REPORT = WORK / 'execution.json'
PLAN = json.loads((WORK / 'plan.json').read_text())


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


assert not REPORT.exists(), 'Never overwrite or restart a prior execution'
assert all(sha(WORK / p) == v for p, v in PLAN['harness_pins'].items())
assert all(sha(ROOT / p) == v for p, v in PLAN['source_pins'].items())
inputs = {argument: sha(argument) for case in PLAN['cases'] for argument in case['arguments'][:3]}
state = {'status': 'building', 'producer_pid': os.getpid(), 'plan_sha256': sha(WORK / 'plan.json'),
         'input_pins': inputs, 'results': [], 'scope': PLAN['scope']}


def save():
    temporary = REPORT.with_suffix('.tmp')
    temporary.write_text(json.dumps(state, indent=2) + '\n')
    temporary.replace(REPORT)


environment = os.environ.copy()
runtime = Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010')
environment['LIBTORCH_USE_PYTORCH'] = '1'
environment['PATH'] = str(runtime / 'bin') + os.pathsep + environment['PATH']
environment['DYLD_LIBRARY_PATH'] = str(runtime / 'lib/python3.12/site-packages/torch/lib')
command = ['cargo', '+1.98.1', 'build', '--offline', '--manifest-path', str(WORK / 'Cargo.toml'),
           '--bin', 'native_bank_current_all_positives', '--jobs', '2',
           '--target-dir', str(ROOT / 'target'), '--message-format=json']
state['build_command'] = command
save()
with (WORK / 'build.log').open('x') as output:
    child = subprocess.Popen(command, cwd=WORK, env=environment, stdout=output, stderr=subprocess.STDOUT)
    state['child_pid'] = child.pid
    save()
    code = child.wait()
state['build_exit_code'] = code
state['build_log_sha256'] = sha(WORK / 'build.log')
if code:
    state['status'] = 'terminal_build_failure'
    save()
    raise SystemExit(code)
artifacts = []
for line in (WORK / 'build.log').read_text().splitlines():
    try:
        item = json.loads(line)
    except ValueError:
        continue
    if item.get('reason') == 'compiler-artifact' and item.get('executable'):
        artifacts.append(item['executable'])
assert len(artifacts) == 1
executable = artifacts[0]
state.update(status='running', binary=executable, binary_sha256=sha(executable),
             focused_lock_sha256=sha(WORK / 'Cargo.lock'))
save()
for index, case in enumerate(PLAN['cases']):
    assert all(sha(path) == value for path, value in inputs.items())
    command = ['/usr/bin/time', '-l', executable] + case['arguments']
    start = time.monotonic()
    with (WORK / f'control-{index}.stdout').open('x') as output, (WORK / f'control-{index}.stderr').open('x') as error:
        child = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=output, stderr=error)
        state.update(active_index=index, child_pid=child.pid)
        save()
        code = child.wait()
    result = {'label': case['label'], 'command': command, 'exit_code': code,
              'elapsed_seconds': time.monotonic() - start,
              'stdout_sha256': sha(WORK / f'control-{index}.stdout'),
              'stderr_sha256': sha(WORK / f'control-{index}.stderr')}
    if code == 0:
        result['evidence'] = json.loads((WORK / f'control-{index}.stdout').read_text())
        assert result['evidence']['managed_used'] == 0
    state['results'].append(result)
    save()
    if code:
        state['status'] = 'terminal_control_failure'
        save()
        raise SystemExit(code)
state['source_drift'] = [p for p, v in PLAN['source_pins'].items() if sha(ROOT / p) != v]
state['status'] = 'complete_three_controls_pending_audit' if not state['source_drift'] else 'complete_unqualified_source_drift'
save()
print(state['status'])
