#!/usr/bin/env python3
"""Resume the original common-policy corpus; retained completed reports unchanged."""
from pathlib import Path
import hashlib
import json
import os
import re
import signal
import subprocess

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
OLD = D / 'dedup-consensus-current-negative-20261013.json'
old = json.loads(OLD.read_text())
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert old['completed'] == len(old['results']) == 74
assert old['counts']['execution_failed'] == 0
assert (D / 'dedup-consensus-current-negative-prefix-0074-audit-20261013.json').exists()
PLAN = Path(old['plan'])
assert sha(PLAN) == old['plan_sha256']
plan = json.loads(PLAN.read_text())
assert all(sha(path) == pin for path, pin in plan['pins'].items())
manifest = json.loads(Path(plan['source_manifest']).read_text())
source = Path(plan['source'])
assert all(sha(source / name) == pin for name, pin in manifest['source_sha256'].items())
for index, row in enumerate(old['results']):
    assert row['index'] == index and sha(row['report']) == row['sha256']
    report = json.loads(Path(row['report']).read_text())
    assert report['returncode'] == 0 and report['command'][-8:] == plan['cases'][index]['arguments']
    value = json.loads(report['stdout'])
    assert value['managed_used'] == 0
    assert value['support'] == (True if value['forward_support'] is True and value['reverse_support'] is True else None)
F = Path('/private/tmp/rrrah-consensus-current-negative-resume-v3-20261014')
F.mkdir(exist_ok=False)
executable = F / 'probe'
subprocess.run(['cp', '-cp', old['executable'], str(executable)], check=True)
assert sha(executable) == old['executable_sha256'] == plan['pins'][plan['executable']]
failed_path=D/'dedup-consensus-current-negative-resume-v2-20261014.json'
failed=json.loads(failed_path.read_text());assert failed['status']=='terminal_runtime_failure' and failed['active_index']==74 and failed['results'][-1]['returncode']==1
OUT = D / 'dedup-consensus-current-negative-resume-v3-20261014.json'
assert not OUT.exists()
state = dict(old)
state.update(status='running', producer_pid=os.getpid(), executable=str(executable), producer_sha256=sha(__file__), previous_report=str(OLD), previous_report_sha256=sha(OLD), resumed_from=74, resume_scope='Original frozen pre-cache binary and original normalized common recipe. Prefix74 retained exactly; current Root/native collection changes excluded. Original controllers/last children absent and executable commands absent before launch. Unknowns are not verified rejections.')
state.pop('child_pid', None)
state.update(wrapper_failure_report=str(failed_path),wrapper_failure_report_sha256=sha(failed_path),wrapper_scope='Drop time -l only: sandbox denies sysctl kern.clockrate after model output. Retry case74 freshly; original failed output retained, not promoted to a successful result. No RSS measurement from this wrapper.')

def save():
    temporary = OUT.with_suffix('.tmp')
    temporary.write_text(json.dumps(state, indent=2) + '\n')
    temporary.replace(OUT)

def interrupted(signum, frame):
    raise KeyboardInterrupt(f'signal {signum}')

signal.signal(signal.SIGTERM, interrupted)
signal.signal(signal.SIGINT, interrupted)
save()
child = None
try:
    for index in range(74, len(plan['cases'])):
        case = plan['cases'][index]
        # A plan with an optional reused control may instead execute freshly.
        # Record reused=False and the actual new command/results as usual.
        relevant = [case['left'], case['right'], case['arguments'][2]]
        assert all(sha(path) == plan['pins'][path] for path in relevant)
        command = ['/usr/bin/time', '/usr/bin/env', 'DYLD_LIBRARY_PATH=/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib', str(executable), *case['arguments']]
        child = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
        state.update(active_index=index, child_pid=child.pid)
        save()
        stdout, stderr = child.communicate()
        rss = re.search(r'(\d+)\s+maximum resident set size', stderr)
        report = dict(index=index, command=command, returncode=child.returncode, stdout=stdout, stderr=stderr, rss_bytes=int(rss[1]) if rss else None, reused=False, label=case['label'], expected=case['expected'], left=case['left'], right=case['right'])
        child = None
        path = F / f'{index:04}.json'
        path.write_text(json.dumps(report, indent=2) + '\n')
        row = dict(index=index, label=case['label'], expected=case['expected'], report=str(path), sha256=sha(path), returncode=report['returncode'], reused=False)
        if report['returncode'] != 0:
            row['classification'] = 'execution_failed'
            state['counts']['execution_failed'] += 1
            state['results'].append(row)
            state.update(completed=len(state['results']), status='terminal_runtime_failure')
            save()
            raise RuntimeError(f'case {index} failed')
        value = json.loads(stdout)
        assert value['managed_used'] == 0 and value['support'] in [True, None]
        assert value['support'] == (True if value['forward_support'] is True and value['reverse_support'] is True else None)
        kind = 'false_acceptance' if value['support'] is True else 'inconclusive_negative'
        row.update(classification=kind, support=value['support'], forward_support=value['forward_support'], reverse_support=value['reverse_support'], model_attempted=[value['forward_model_attempted'], value['reverse_model_attempted']])
        assert all(sha(path) == plan['pins'][path] for path in relevant)
        state['counts'][kind] += 1
        state['results'].append(row)
        state['completed'] = len(state['results'])
        save()
    assert all(sha(path) == pin for path, pin in plan['pins'].items())
    assert all(sha(source / name) == pin for name, pin in manifest['source_sha256'].items())
    state.update(status='complete_fixed_current_atomic_corpus_pending_audit', source_unchanged=True)
    save()
except BaseException as error:
    if state['status'] == 'running':
        state.update(status='terminal_controller_failure', error=repr(error))
        save()
    raise
finally:
    if child is not None and child.poll() is None:
        os.killpg(child.pid, signal.SIGKILL)
        child.wait()
