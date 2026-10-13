#!/usr/bin/env python3
"""Freeze current path sources and fixtures for decode/container qualification."""
import hashlib
import json
import os
import re
import shutil
import subprocess
from pathlib import Path

R = Path(__file__).resolve().parents[1]
T = Path('/Users/themoretheless/.codex/tmp')
S = T / 'rrrah-dedup-current-decode-source-20261012'
P = R / 'docs/research/dedup-current-decode-gate-20261012.json'

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def main():
    assert not S.exists() and not P.exists()
    inputs = []
    for root in [R / 'crates', R / 'vendor', R / 'tests/fixtures/raster', R / '.cargo']:
        for parent, dirs, files in os.walk(root):
            dirs[:] = [d for d in dirs if d not in ['target', '.git', 'node_modules', '__pycache__']]
            for name in files:
                p = Path(parent) / name
                if not p.is_symlink():
                    inputs.append(p)
    inputs += [R / n for n in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'rustfmt.toml']]
    hashes = {str(p.relative_to(R)): sha(p) for p in inputs}
    for p in inputs:
        q = S / p.relative_to(R)
        q.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p, q)
    assert all(sha(R / n) == sha(S / n) == h for n, h in hashes.items())
    log = T / 'rrrah-dedup-current-decode-gate-20261012.log'
    assert not log.exists()
    cmd = ['cargo', 'test', '-p', 'rrrah-dedup', '--features', 'decode', '--locked',
           '--lib', '--test', 'raster_equality', '--test', 'animated', '--test', 'pages',
           '--test', 'container', '--target-dir', str(T / 'rrrah-dedup-exact-portfolio-20261011')]
    r = dict(status='prepared_frozen_sources', producer_pid=os.getpid(), source=str(S),
             source_hashes=hashes, command=cmd, log=str(log),
             pins={str(Path(__file__)): sha(__file__)},
             scope='Frozen current macOS decode-feature library units plus raster_equality/animated/pages/container integration targets. Includes current path dependencies and their changes. Not all image formats, Torch, complete visual corpus, other platforms or full500 reviews.')
    def save():
        tmp = P.with_suffix('.tmp')
        tmp.write_text(json.dumps(r, indent=2) + '\n')
        tmp.replace(P)
    save()
    with log.open('w') as output:
        child = subprocess.Popen(cmd, cwd=S, stdout=output, stderr=subprocess.STDOUT)
        r.update(status='running', child_pid=child.pid)
        save()
        rc = child.wait()
    r.update(returncode=rc)
    r['pins'][str(log)] = sha(log)
    assert all(sha(S / n) == h for n, h in hashes.items())
    r['root_drift_after_snapshot'] = [n for n, h in hashes.items() if not (R / n).is_file() or sha(R / n) != h]
    if rc:
        r['status'] = 'terminal_build_or_test_failure'
        save()
        return
    text = log.read_text()
    markers = list(re.finditer(r'Running (unittests src/lib\.rs|tests/(?:raster_equality|animated|pages|container)\.rs) \(([^)]+)\)', text))
    assert len(markers) == 5
    rows = []
    for i, marker in enumerate(markers):
        output = text[marker.end():markers[i+1].start() if i+1 < len(markers) else len(text)]
        summary = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;', output)
        assert len(summary) == 1
        passed, failed, ignored, measured, filtered = map(int, summary[0])
        assert passed > 0 and failed == measured == filtered == 0
        names = re.findall(r'^test ([^\n]+) \.\.\. ok$', output, re.M)
        skipped = re.findall(r'^test ([^\n]+) \.\.\. ignored(?:, ([^\n]+))?$', output, re.M)
        assert len(names) == len(set(names)) == passed and len(skipped) == ignored
        assert re.search(rf'^running {passed+ignored} tests$', output, re.M)
        rows.append(dict(target=marker[1], passed=passed, ignored=ignored, names=names, skipped=skipped))
        r['pins'][marker[2]] = sha(marker[2])
    assert {x['target'] for x in rows} == {'unittests src/lib.rs', 'tests/raster_equality.rs',
                                          'tests/animated.rs', 'tests/pages.rs', 'tests/container.rs'}
    r.update(status='complete_frozen_current_decode_gate', targets=rows,
             passed=sum(x['passed'] for x in rows), ignored=sum(x['ignored'] for x in rows))
    save()
    print([(x['target'], x['passed'], x['ignored']) for x in rows])

if __name__ == '__main__':
    main()
