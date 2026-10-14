#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import os
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PLAN = ROOT / 'docs/research/dedup-native-collection-harness-20261014.json'
plan = json.loads(PLAN.read_text())
H = Path(plan['harness'])
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert all(sha(H / name) == pin for name, pin in plan['harness_sha256'].items())
M = Path(plan['source_manifest'])
assert sha(M) == plan['source_manifest_sha256']
manifest = json.loads(M.read_text())
S = Path(manifest['source'])
assert all(sha(S / name) == pin for name, pin in manifest['source_sha256'].items())
OUT = ROOT / 'docs/research/dedup-native-collection-focused-20261014.json'
assert not OUT.exists()
F = Path('/private/tmp/rrrah-native-collection-focused-20261014')
F.mkdir(exist_ok=False)
env = os.environ.copy()
runtime = Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010')
env.update(CARGO_HOME='/private/tmp/rrrah-runner-full-cargo-home-20261014', LIBTORCH_USE_PYTORCH='1', PATH=str(runtime / 'bin') + os.pathsep + env['PATH'], DYLD_LIBRARY_PATH=str(runtime / 'lib/python3.12/site-packages/torch/lib'))
command = ['cargo', '+stable', 'test', '--offline', '--locked', '--no-run', '--test', 'portfolio_collection', '--test', 'search_portfolio', '--jobs', '2', '--target-dir', plan['target'], '--message-format=json']
state = dict(status='building', producer_pid=os.getpid(), command=command, plan=str(PLAN), plan_sha256=sha(PLAN), producer_sha256=sha(__file__), rows=[], scope=plan['scope'] + ' Full selected tests use synthetic proposal providers; empty native adapter test checks lazy loading, not actual learned inference.')

def save():
    temporary = OUT.with_suffix('.tmp')
    temporary.write_text(json.dumps(state, indent=2) + '\n')
    temporary.replace(OUT)

save()
with (F / 'build.jsonl').open('x') as output, (F / 'build.log').open('x') as errors:
    child = subprocess.Popen(command, cwd=H, env=env, stdout=output, stderr=errors)
    state['child_pid'] = child.pid
    save()
    code = child.wait()
state['build_returncode'] = code
if code == 0:
    artifacts = []
    for line in (F / 'build.jsonl').read_text().splitlines():
        item = json.loads(line)
        if item.get('reason') == 'compiler-artifact' and item.get('executable') and item['target']['kind'] == ['test']:
            artifacts.append(item)
    assert {item['target']['name'] for item in artifacts} == {'portfolio_collection', 'search_portfolio'} and len(artifacts) == 2
    for item in artifacts:
        name = item['target']['name']
        executable = F / name
        subprocess.run(['cp', '-cp', item['executable'], str(executable)], check=True)
        state.update(status='testing', target=name)
        save()
        with (F / (name + '.log')).open('x') as log:
            child = subprocess.Popen([str(executable)], cwd=H, env=env, stdout=log, stderr=subprocess.STDOUT)
            state['child_pid'] = child.pid
            save()
            rc = child.wait()
        state['rows'].append(dict(target=name, returncode=rc, executable=str(executable), executable_sha256=sha(executable), log=str(F / (name + '.log')), log_sha256=sha(F / (name + '.log'))))
        save()
state['source_unchanged'] = all(sha(S / name) == pin for name, pin in manifest['source_sha256'].items())
state['status'] = 'passed_pending_audit' if code == 0 and len(state['rows']) == 2 and all(row['returncode'] == 0 for row in state['rows']) and state['source_unchanged'] else 'build_or_test_failure'
state['build_pins'] = {str(F / name): sha(F / name) for name in ['build.log', 'build.jsonl']}
save()
print(state['status'])
