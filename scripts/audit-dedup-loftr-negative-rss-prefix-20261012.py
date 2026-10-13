#!/usr/bin/env python3
"""Freeze complete inference stage observations while the diagnostic continues."""
import hashlib
import json
import re
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp')

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def main():
    report_path = D / 'dedup-loftr-negative-rss-profile-20261012.json'
    raw_report = report_path.read_bytes()
    report = json.loads(raw_report)
    assert report['build_returncode'] == 0
    assert all(sha(p) == h for p, h in report['pins'].items())
    mp = D / 'dedup-disc21-heldout-negative-source-20261012.json'
    original = json.loads(mp.read_text())
    name = 'vendor/loftr/src/loftr_model.rs'
    expected = (Path(original['snapshot']) / name).read_text()
    for before, after in report['instrumentation_replacements']:
        assert expected.count(before) == 1
        expected = expected.replace(before, after)
    expected += report['instrumentation_helper']
    assert expected == (Path(report['source']) / name).read_text()
    assert all(sha(Path(report['source']) / n) == h for n, h in original['file_hashes'].items() if n != name)
    raw = Path(report['stderr']).read_bytes()
    raw = raw[:raw.rfind(b'\n') + 1]
    rows = [dict(call=int(c), stage=s, rss_bytes=int(r), shapes=json.loads(shape))
            for c, s, r, shape in re.findall(r'RRRAH_RSS call=(\d+) stage=(\S+) rss_bytes=(\d+) shapes=(.*)', raw.decode())]
    stages = ['input', 'backbone', 'coarse.features', 'coarse.matches', 'coarse.released',
              'fine.windows', 'features.released', 'fine.transform', 'fine.matches']
    complete = []
    for call in sorted({r['call'] for r in rows if r['call'] > 0}):
        group = [r for r in rows if r['call'] == call and r['stage'] in stages]
        if len(group) != len(stages):
            break
        assert [r['stage'] for r in group] == stages
        complete.append(group)
    n = len(complete)
    assert n > 0
    captured = T / f'rrrah-loftr-negative-rss-prefix-{n:03d}-20261012.stderr'
    capture_report = T / f'rrrah-loftr-negative-rss-prefix-{n:03d}-20261012.json'
    output = D / f'dedup-loftr-negative-rss-prefix-{n:03d}-audit-20261012.json'
    assert not any(p.exists() for p in [captured, capture_report, output])
    captured.write_bytes(raw)
    capture_report.write_bytes(raw_report)
    pins = dict(report['pins'])
    pins.update({str(p): sha(p) for p in [captured, capture_report, mp, Path(__file__)]})
    first = {r['stage']: r['rss_bytes'] for r in complete[0]}
    output.write_text(json.dumps({
        'status': 'verified_completed_inference_prefix_of_running_stage_rss_diagnostic',
        'complete_inferences': n, 'stage_order': stages, 'rows': complete,
        'first_inference_rss_bytes': first,
        'first_backbone_boundary_increase_bytes': first['backbone'] - first['input'],
        'pins': pins,
        'scope': 'Frozen phase-boundary ps current RSS samples, not kernel peaks, tensor ownership attribution or whole-operation conclusion. Instrumentation-only delta reconstructed; all other frozen source hashes unchanged. Final evidence parity and full profiling completion remain pending. Same historical worst negative pair and fixed caller policy; no root optimization/adoption or hard RSS cap claim.',
    }, indent=2) + '\n')
    print('Verified', n, 'complete inference traces; first backbone boundary increase', first['backbone']-first['input'])

if __name__ == '__main__':
    main()
