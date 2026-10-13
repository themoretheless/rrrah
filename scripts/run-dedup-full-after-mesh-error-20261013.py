"""Rerun the frozen full decode gate after the error-contract test correction."""
import pathlib, json, subprocess, hashlib, os

R = pathlib.Path(__file__).resolve().parents[1]
T = pathlib.Path('/Users/themoretheless/.codex/tmp')
S = T / 'rrrah-mesh-rank-error-contract-20261013/source'
P = R / 'docs/research/dedup-full-after-mesh-error-20261013.json'
L = T / 'rrrah-mesh-rank-error-contract-20261013/full.log'
assert not P.exists() and not L.exists()
cmd = ['cargo', 'test', '-p', 'rrrah-dedup', '--features', 'decode', '--locked', '--offline', '--tests', '--jobs', '2', '--target-dir', str(T / 'rrrah-dedup-exact-portfolio-20261011')]
d = dict(status='running', source=str(S), producer_pid=os.getpid(), command=cmd, log=str(L), scope='Original frozen full-fixture snapshot plus one corrected test expectation; excludes subsequent model-worker/cache changes.')
def save():
    P.write_text(json.dumps(d, indent=2) + '\n')
save()
with L.open('x') as out:
    child = subprocess.Popen(cmd, cwd=S, stdout=out, stderr=subprocess.STDOUT)
    d['child_pid'] = child.pid
    save()
    code = child.wait()
d.update(status='passed' if code == 0 else 'terminal_failure', returncode=code, log_sha256=hashlib.sha256(L.read_bytes()).hexdigest())
save()
print(d['status'])
