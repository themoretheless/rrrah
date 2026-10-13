"""Qualify managed ownership of retained nonzero-exit diagnostics."""
import pathlib, shutil, subprocess, json, hashlib, re

R = pathlib.Path(__file__).resolve().parents[1]
T = pathlib.Path('/Users/themoretheless/.codex/tmp')
W = T / 'rrrah-model-worker-failure-credit-20261013'
S = W / 'source'
F = pathlib.Path('crates/rrrah-dedup/src/model_worker.rs')
P = R / 'docs/research/dedup-model-worker-failure-credit-20261013.json'
assert not P.exists() and not (W / 'fixed.log').exists()
baseline = (W / 'baseline.log').read_text()
assert 'retained diagnostics escaped their reservation' in baseline
assert 'test result: FAILED. 0 passed; 1 failed' in baseline
shutil.copy2(S / F, W / 'model_worker.baseline.rs')
shutil.copy2(R / F, S / F)
def sha(p):
    return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
cmd = ['cargo', 'test', '-p', 'rrrah-dedup', '--features', 'decode', '--locked', '--offline', '--lib', 'model_worker::', '--jobs', '2', '--target-dir', str(T / 'rrrah-cache-interpretation-fix-20261012/target')]
d = dict(status='running', source=str(S), command=cmd,
         pins={str(p):sha(p) for p in [W / 'baseline.log', W / 'model_worker.baseline.rs', R / F, pathlib.Path(__file__)]})
def save():
    P.write_text(json.dumps(d, indent=2) + '\n')
save()
with (W / 'fixed.log').open('x') as log:
    child = subprocess.Popen(cmd, cwd=S, stdout=log, stderr=subprocess.STDOUT)
    d['child_pid'] = child.pid
    save()
    code = child.wait()
d['returncode'] = code
d['pins'][str(W / 'fixed.log')] = sha(W / 'fixed.log')
if code == 0:
    assert re.search(r'test result: ok\. 10 passed; 0 failed; 0 ignored;', (W / 'fixed.log').read_text())
    assert all(sha(p) == h for p, h in d['pins'].items())
    assert sha(S / F) == sha(R / F)
    prior = json.loads((R / 'docs/research/dedup-model-worker-audit-20261013.json').read_text())
    hashes = {str(p.relative_to(S)):sha(p) for p in S.rglob('*') if p.is_file()}
    changed = [p for p, h in hashes.items() if prior['source_hashes'].get(p) != h]
    assert changed == [str(F)], changed
    d.update(status='verified_retained_failure_credit', passed_tests=10,
             baseline_failed_tests=1, source_hashes=hashes,
             scope='Frozen model-worker snapshot plus one changed module. Actual Unix child exits7 with retained stderr; budget remains charged across error drop and diagnostic clone, then releases. Ten decode-only protocol tests; no new Torch/RSS qualification.')
else:
    d['status'] = 'terminal_failure'
save()
print(d['status'])
