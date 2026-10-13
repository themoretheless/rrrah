#!/usr/bin/env python3
"""Run current pure pixel/linear integration tests and preserve terminal evidence."""
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path

R = Path(__file__).resolve().parents[1]
T = Path('/Users/themoretheless/.codex/tmp')
P = R / 'docs/research/dedup-current-pixel-gate-20261012.json'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def main():
    assert not P.exists()
    paths = [R / n for n in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
                             '.cargo/config.toml', 'crates/rrrah-dedup/Cargo.toml',
                             'crates/rrrah-dedup/build.rs',
                             'crates/rrrah-dedup/tests/pixels.rs',
                             'crates/rrrah-dedup/tests/linear.rs']]
    paths += sorted((R / 'crates/rrrah-dedup/src').rglob('*.rs'))
    before = {str(p.relative_to(R)): sha(p) for p in paths}
    log = T / 'rrrah-dedup-current-pixel-gate-20261012.log'
    assert not log.exists()
    cmd = ['cargo', 'test', '-p', 'rrrah-dedup', '--no-default-features', '--locked',
           '--test', 'pixels', '--test', 'linear', '--target-dir',
           str(T / 'rrrah-dedup-exact-portfolio-20261011')]
    r = dict(status='prepared', producer_pid=os.getpid(), source_before=before,
             command=cmd, log=str(log), pins={str(Path(__file__)): sha(__file__)},
             scope='Current pure pixels/linear integration on macOS; no decoder, file formats, Torch, other platforms or full corpus/all500 completion.')
    def save():
        tmp = P.with_suffix('.tmp')
        tmp.write_text(json.dumps(r, indent=2) + '\n')
        tmp.replace(P)
    save()
    with log.open('w') as output:
        child = subprocess.Popen(cmd, cwd=R, stdout=output, stderr=subprocess.STDOUT)
        r.update(status='running', child_pid=child.pid)
        save()
        code = child.wait()
    r.update(returncode=code, source_after={p: sha(R / p) for p in before})
    r['pins'][str(log)] = sha(log)
    if code:
        r['status'] = 'terminal_test_failure'
        save()
        return
    if before != r['source_after']:
        r['status'] = 'terminal_source_drift_unqualified'
        save()
        return
    text = log.read_text()
    blocks = re.split(r'Running tests/(linear|pixels)\.rs \(([^)]+)\)', text)
    assert len(blocks) == 7
    rows = []
    for i in range(1, len(blocks), 3):
        name, executable, output = blocks[i:i+3]
        summary = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;', output)
        assert len(summary) == 1
        passed, failed, ignored, measured, filtered = map(int, summary[0])
        assert passed > 0 and failed == ignored == measured == filtered == 0
        names = re.findall(r'^test ([^\n]+) \.\.\. ok$', output, re.M)
        assert len(names) == len(set(names)) == passed
        assert re.search(rf'^running {passed} tests$', output, re.M)
        rows.append(dict(target=name, passed=passed, names=names, executable=executable))
        r['pins'][executable] = sha(executable)
    assert {row['target'] for row in rows} == {'pixels', 'linear'}
    r.update(status='verified_current_pixels_linear_passed', targets=rows,
             passed=sum(row['passed'] for row in rows))
    save()
    print([(row['target'], row['passed']) for row in rows])

if __name__ == '__main__':
    main()
