#!/usr/bin/env python3
"""Verify the complete frozen decode gate and retain immutable test executables."""
import hashlib
import json
import re
import shutil
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp')

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def main():
    report = D / 'dedup-current-decode-gate-20261012.json'
    r = json.loads(report.read_text())
    assert r['status'] == 'complete_frozen_current_decode_gate' and r['returncode'] == 0
    source = Path(r['source'])
    assert all(sha(source / n) == h for n, h in r['source_hashes'].items())
    assert all(sha(p) == h for p, h in r['pins'].items())
    assert r['command'] == ['cargo', 'test', '-p', 'rrrah-dedup', '--features', 'decode', '--locked',
                            '--lib', '--test', 'raster_equality', '--test', 'animated', '--test', 'pages',
                            '--test', 'container', '--target-dir', str(T / 'rrrah-dedup-exact-portfolio-20261011')]
    text = Path(r['log']).read_text()
    markers = list(re.finditer(r'Running (unittests src/lib\.rs|tests/(?:raster_equality|animated|pages|container)\.rs) \(([^)]+)\)', text))
    assert len(markers) == len(r['targets']) == 5
    destination = T / 'rrrah-dedup-current-decode-qualified-executables-20261012'
    out = D / 'dedup-current-decode-gate-complete-audit-20261012.json'
    assert not destination.exists() and not out.exists()
    rows = []
    for i, (marker, recorded) in enumerate(zip(markers, r['targets'])):
        output = text[marker.end():markers[i+1].start() if i+1 < len(markers) else len(text)]
        summary = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;', output)
        assert len(summary) == 1
        passed, failed, ignored, measured, filtered = map(int, summary[0])
        assert failed == measured == filtered == 0 and passed > 0
        names = re.findall(r'^test ([^\n]+) \.\.\. ok$', output, re.M)
        skipped = re.findall(r'^test ([^\n]+) \.\.\. ignored(?:, ([^\n]+))?$', output, re.M)
        assert recorded == dict(target=marker[1], passed=passed, ignored=ignored, names=names, skipped=[list(x) for x in skipped])
        assert len(names) == len(set(names)) == passed and len(skipped) == ignored
        rows.append(dict(recorded, original_executable=marker[2]))
    assert sum(x['passed'] for x in rows) == r['passed']
    assert sum(x['ignored'] for x in rows) == r['ignored']
    destination.mkdir()
    pins = {str(p): sha(p) for p in [report, Path(r['log']), Path(__file__)]}
    for row in rows:
        original = Path(row['original_executable'])
        saved = destination / original.name
        shutil.copy2(original, saved)
        assert sha(saved) == r['pins'][str(original)]
        row['saved_executable'] = str(saved)
        pins[str(saved)] = sha(saved)
    drift = [n for n, h in r['source_hashes'].items() if not (R/n).is_file() or sha(R/n) != h]
    out.write_text(json.dumps(dict(status='verified_complete_frozen_current_decode_gate',
        passed=r['passed'], ignored=r['ignored'], targets=rows,
        frozen_files_verified=len(r['source_hashes']), root_drift_now=drift,
        pins=pins, scope=r['scope']), indent=2) + '\n')
    print([(x['target'], x['passed'], x['ignored']) for x in rows])

if __name__ == '__main__':
    main()
