"""Replay the pinned built probe after fixing macOS loader environment placement."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
PRIOR = ROOT / '.dedup-work/current-loftr-trace-controls-v2-20261016'
WORK = ROOT / '.dedup-work/current-loftr-trace-loader-v3-20261019'
WORK.mkdir(exist_ok=False)
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior = json.loads((PRIOR / 'execution.json').read_text())
plan = json.loads((PRIOR / 'plan.json').read_text())
assert prior['status'] == 'terminal_control_failure' and prior['build_exit_code'] == 0
assert sha(prior['binary']) == prior['binary_sha256']
assert sha(PRIOR / 'plan.json') == prior['plan_sha256']
assert all(sha(p) == v for p, v in prior['input_pins'].items())
runtime = Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib')
state = {'status': 'running', 'producer_pid': os.getpid(), 'binary': prior['binary'],
         'binary_sha256': prior['binary_sha256'], 'plan_sha256': prior['plan_sha256'],
         'runtime_cpu_sha256': sha(runtime / 'libtorch_cpu.dylib'),
         'engine_source_drift': [p for p,v in plan['source_pins'].items() if sha(ROOT / p) != v],
         'results': [], 'scope': 'Same pinned binary and fixed three-control policy. /usr/bin/env applies DYLD_LIBRARY_PATH after protected macOS time executable. Not fresh current-root/full-dependency/corpus qualification.'}


def save():
    temporary = WORK / 'execution.tmp'
    temporary.write_text(json.dumps(state, indent=2) + '\n')
    temporary.replace(WORK / 'execution.json')


save()
for index, case in enumerate(plan['cases']):
    assert sha(prior['binary']) == prior['binary_sha256']
    assert all(sha(p) == v for p,v in prior['input_pins'].items())
    command = ['/usr/bin/time', '-l', '/usr/bin/env', 'DYLD_LIBRARY_PATH=' + str(runtime), prior['binary']] + case['arguments']
    start = time.monotonic()
    with (WORK / f'{index}.stdout').open('x') as output, (WORK / f'{index}.stderr').open('x') as error:
        child = subprocess.Popen(command, cwd=ROOT, stdout=output, stderr=error)
        state.update(active_index=index, child_pid=child.pid)
        save()
        code = child.wait()
    result = {'label': case['label'], 'command': command, 'exit_code': code,
              'elapsed_seconds': time.monotonic() - start,
              'stdout_sha256': sha(WORK / f'{index}.stdout'), 'stderr_sha256': sha(WORK / f'{index}.stderr')}
    if code == 0:
        result['evidence'] = json.loads((WORK / f'{index}.stdout').read_text())
        assert result['evidence']['managed_used'] == 0
    state['results'].append(result)
    save()
    if code:
        state['status'] = 'terminal_control_failure'
        save()
        raise SystemExit(code)
state['status'] = 'complete_three_controls_pending_audit'
state['engine_source_drift_after'] = [p for p,v in plan['source_pins'].items() if sha(ROOT / p) != v]
save()
print(state['status'])
