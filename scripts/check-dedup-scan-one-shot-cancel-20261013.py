"""Qualify sticky batch cancellation through decoding and retry."""
import pathlib, json, shutil, subprocess, hashlib, re

R = pathlib.Path(__file__).resolve().parents[1]
T = pathlib.Path('/Users/themoretheless/.codex/tmp')
W = T / 'rrrah-scan-one-shot-cancel-20261013'
S = W / 'source'
P = R / 'docs/research/dedup-scan-one-shot-cancel-20261013.json'
assert not P.exists() and not (W / 'fixed.log').exists()
baseline = (W / 'baseline.log').read_text()
assert 'test result: FAILED. 0 passed; 3 failed;' in baseline
files = ['crates/rrrah-dedup/src/scan.rs', 'crates/rrrah-dedup/tests/scan.rs']
for f in files:
    shutil.copy2(S / f, W / (pathlib.Path(f).stem + '.baseline.rs'))
    shutil.copy2(R / f, S / f)
def sha(p):
    return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
cmd = ['cargo', 'test', '-p', 'rrrah-dedup', '--features', 'decode', '--locked', '--offline', '--test', 'scan', '--test', 'scan_retry', '--jobs', '2', '--target-dir', str(T / 'rrrah-cache-interpretation-fix-20261012/target')]
d = dict(status='running', source=str(S), command=cmd, pins={str(p):sha(p) for p in [W / 'baseline.log', W / 'scan.baseline.rs', pathlib.Path(__file__)]})
for f in files:
    d['pins'][str(R/f)] = sha(R/f)
def save():
    P.write_text(json.dumps(d, indent=2) + '\n')
save()
with (W / 'fixed.log').open('x') as log:
    child = subprocess.Popen(cmd, cwd=S, stdout=log, stderr=subprocess.STDOUT)
    d['child_pid'] = child.pid
    save()
    code = child.wait()
d.update(returncode=code)
d['pins'][str(W / 'fixed.log')] = sha(W / 'fixed.log')
if code == 0:
    counts = re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;', (W / 'fixed.log').read_text())
    assert sorted(map(int,counts)) == [1,13], counts
    assert all(sha(p) == value for p,value in d['pins'].items())
    prior = json.loads((R / 'docs/research/dedup-model-worker-failure-credit-20261013.json').read_text())
    hashes = {str(p.relative_to(S)):sha(p) for p in S.rglob('*') if p.is_file()}
    assert sorted(p for p,h in hashes.items() if prior['source_hashes'].get(p) != h) == sorted(files)
    d.update(status='verified_sticky_cancellation_for_three_batch_paths', baseline_failed_tests=3,
        passed_tests=14, source_hashes=hashes,
        scope='One-shot caller cancellation during actual managed decoding in visual scan, pixel confirmation and grouping. All13scan tests plus1scan-retry lifecycle test. Frozen worker-credit snapshot plus2 changed scan files; excludes fullroot/nativeTorch/platform claims.')
else:
    d['status'] = 'terminal_failure'
save()
print(d['status'])
