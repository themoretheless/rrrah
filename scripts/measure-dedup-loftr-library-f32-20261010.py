"""Six real pairs through optional library LoFTR API; narrow CPU qualification."""
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT = D / 'dedup-loftr-library-f32-measurement-20261010.json'
BINARY = T / 'loftr-library-f32-qualified-20261010'
PINS = D / 'dedup-loftr-library-f32-source-pins-20261010.json'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
m = json.loads(PINS.read_text())
def verify():
    changed = [p for p, h in m['file_hashes'].items() if sha(ROOT / p) != h]
    assert not changed, changed
verify()
assert not OUT.exists() and not BINARY.exists()
shutil.copy2(ROOT / 'target/debug/compare_files', BINARY)
weights = T / 'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
target = T / 'original-resolution-strong-all/209401.jpg'
sources = [T / 'original-resolution-negative-200201' / n for n in ('209400.jpg', '201400.jpg', '209000.jpg')]
files = [BINARY, PINS, weights, target, *sources, Path(__file__)]
r = dict(status='running', pins={str(p): sha(p) for p in files}, results=[], scope=__doc__, dependency_scope='Current root library/decoder/vendor source; standalone probe lock; optimized dev build, not a controlled latency benchmark.')
def save():
    temp = OUT.with_suffix('.json.tmp')
    temp.write_text(json.dumps(r, indent=2) + '\n')
    temp.replace(OUT)
save()
try:
    baseline = json.loads((D / 'dedup-loftr-confidence-reuse-measurement-20261010.json').read_text())
    dyld = '/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'
    for source in sources:
        for side in (320, 640):
            p = subprocess.run(['/usr/bin/time', '-l', '/usr/bin/env', 'DYLD_LIBRARY_PATH=' + dyld, str(BINARY), str(source), str(target), str(weights), str(side), str(2 * math.hypot(2560, 1920) / math.hypot(1024, 701))], capture_output=True, text=True)
            row = dict(source=str(source), max_side=side, returncode=p.returncode, stdout=p.stdout, stderr=p.stderr)
            r['results'].append(row)
            save()
            assert p.returncode == 0
            row['evidence'] = json.loads(p.stdout)
            row['process_peak_rss_bytes'] = int(re.search(r'(\d+)\s+maximum resident set size', p.stderr)[1])
            row['expected_decision'] = row['evidence']['supported'] == (source.name == '209400.jpg')
            assert row['evidence']['managed_used'] == 0
            before = baseline['results'][len(r['results']) - 1]
            row['exact_evidence_parity'] = row['evidence'] == before['evidence']
            save()
            print(source.name, side, row['evidence']['supported'], flush=True)
    verify()
    assert all(row['expected_decision'] for row in r['results']), 'decision changed'
    assert all(row['exact_evidence_parity'] for row in r['results']), 'decision/payload evidence changed'
    assert all(sha(p) == h for p, h in r['pins'].items())
    r['status'] = 'complete_six_pair_cpu_library_qualification'
except Exception as error:
    r.update(status='failed', error=str(error))
    raise
finally:
    save()
