"""Real-weight cooperative checkpoint cancellation; native-prepared gray inputs.

Two positive input sizes; nine one-shot cancellation points each and exact retry.
This does not prove kernel preemption, cancellation latency or a total RSS bound.
"""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT = D / 'dedup-loftr-strip-padding-cancel-forward-20261010.json'
BINARY = T / 'loftr-strip-padding-cancel-forward-qualified-20261010'
MANIFEST = D / 'dedup-loftr-strip-padding-snapshot-20261010.json'
PREPARATION = D / 'dedup-native-file-model-input-20261010.json'
RUNTIME = D / 'dedup-loftr-native-20261010-runtime-pins.json'
WEIGHTS = T / 'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert not OUT.exists() and not BINARY.exists()
m = json.loads(MANIFEST.read_text())
prepared = json.loads(PREPARATION.read_text())
runtime = json.loads(RUNTIME.read_text())
assert all(sha(Path(m['snapshot']) / p) == h for p, h in m['file_hashes'].items())
assert all(sha(p) == h for p, h in runtime['pins'].items())
gray_pins = {p: h for p, h in prepared['pins'].items() if p.endswith('.gray')}
assert len(gray_pins) == 4 and all(sha(p) == h for p, h in gray_pins.items())
shutil.copy2('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-target-20261010/release/cancel_forward', BINARY)
pins = {str(p): sha(p) for p in [BINARY, MANIFEST, PREPARATION, RUNTIME, WEIGHTS, Path(__file__), D / 'dedup-loftr-strip-padding-native-build-20261010.log']}
pins.update(gray_pins)
report = dict(status='running', pins=pins, results=[], scope=__doc__)

def save():
    temp = OUT.with_suffix('.json.tmp')
    temp.write_text(json.dumps(report, indent=2) + '\n')
    temp.replace(OUT)

save()
try:
    env = dict(os.environ)
    env['DYLD_LIBRARY_PATH'] = '/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'
    for row in prepared['results']:
        side = row['side']
        args = [str(BINARY)]
        for index in (0, 1):
            path = T / 'native-file-model-input-20261010' / (str(side) + '-' + str(index) + '.gray')
            args += [str(path), *map(str, row['preparation'][index]['evidence']['dimensions'])]
        args += [str(WEIGHTS)]
        p = subprocess.run(args, capture_output=True, text=True, env=env)
        result = dict(side=side, returncode=p.returncode, stdout=p.stdout, stderr=p.stderr)
        report['results'].append(result)
        save()
        assert p.returncode == 0
        evidence = json.loads(p.stdout)
        assert evidence['status'] == 'ok' and evidence['retry_exact']
        assert evidence['raw_proposals'] == {320: 263, 640: 925}[side]
        assert len(evidence['cancelled_at']) == 9
        result['evidence'] = evidence
        save()
        print(side, evidence['checkpoints'], evidence['cancelled_at'], flush=True)
    assert all(sha(p) == h for p, h in pins.items())
    assert all(sha(Path(m['snapshot']) / p) == h for p, h in m['file_hashes'].items())
    assert all(sha(p) == h for p, h in runtime['pins'].items())
    report['status'] = 'complete_real_weight_cooperative_cancellation'
except Exception as error:
    report.update(status='failed_real_weight_cooperative_cancellation', error=str(error))
    raise
finally:
    save()
