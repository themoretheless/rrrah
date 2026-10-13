"""Automatic native/LoFTR search: real rephoto in model-first order, rotation controls in native-first order.

Five narrow CPU comparisons, not full corpus/collection/format/platform qualification.
Native-first recovery of the original large rephoto is not claimed by these controls.
"""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT = D / 'dedup-search-portfolio-measurement-20261010.json'
BINARY = T / 'loftr-search-portfolio-qualified-20261010'
MANIFEST = D / 'dedup-search-portfolio-snapshot-20261010.json'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
m = json.loads(MANIFEST.read_text())
def verify():
    assert all(sha(ROOT / p) == h and sha(Path(m['snapshot']) / p) == h for p, h in m['file_hashes'].items())
verify()
assert not OUT.exists() and not BINARY.exists()
shutil.copy2(ROOT / 'target/debug/portfolio_files', BINARY)
weights = T / 'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
original = T / 'original-resolution-negative-200201/209400.jpg'
rephoto = T / 'original-resolution-strong-all/209401.jpg'
fixtures = ROOT / 'crates/rrrah-dedup/tests/fixtures/rotation'
cases = [(original, rephoto, side, '5.157305482252365', 'model-first', True, 'model') for side in (320, 640)]
cases += [(fixtures / 'base.png', fixtures / name, 320, '2', 'native-first', expected, terminal) for name, expected, terminal in [('base.png', True, 'native'), ('mirrored.png', True, 'native'), ('unrelated.png', False, 'model')]]
files = [BINARY, MANIFEST, weights, original, rephoto, Path(__file__), D / 'dedup-search-portfolio-native-build-20261010.log'] + [p for case in cases for p in case[:2]]
r = dict(status='running', scope=__doc__, pins={str(p): sha(p) for p in files}, results=[])
def save():
    tmp = OUT.with_suffix('.tmp')
    tmp.write_text(json.dumps(r, indent=2) + '\n')
    tmp.replace(OUT)
save()
try:
    baseline = json.loads((D / 'dedup-loftr-library-f32-measurement-20261010.json').read_text())
    dyld = '/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'
    for source, target, side, tolerance, order, supported, terminal in cases:
        p = subprocess.run(['/usr/bin/time', '-l', '/usr/bin/env', 'DYLD_LIBRARY_PATH=' + dyld, str(BINARY), str(source), str(target), str(weights), str(side), tolerance, order], capture_output=True, text=True)
        row = dict(source=str(source), target=str(target), max_side=side, order=order, returncode=p.returncode, stdout=p.stdout, stderr=p.stderr, expected_supported=supported, expected_terminal=terminal)
        r['results'].append(row)
        save()
        assert p.returncode == 0, p.stderr
        row['evidence'] = json.loads(p.stdout)
        row['process_peak_rss_bytes'] = int(re.search(r'(\d+)\s+maximum resident set size', p.stderr)[1])
        e = row['evidence']
        assert e['supported'] == supported and e['terminal'] == terminal, e
        assert e['managed_used'] == 0 and e['managed_peak'] <= 512*1024*1024
        assert e['attempted_stages'] == (2 if not supported else 1)
        if source == original:
            before = next(x['evidence'] for x in baseline['results'] if x['source'] == str(source) and x['max_side'] == side)
            row['exact_standalone_model_evidence_parity'] = {k:v for k,v in e.items() if k not in ('terminal', 'order', 'attempted_stages', 'first_miss')} == before
            assert row['exact_standalone_model_evidence_parity']
        save()
        print(source.name, target.name, side, order, e['supported'], e['terminal'], flush=True)
    verify()
    assert all(sha(p) == h for p,h in r['pins'].items())
    r['status'] = 'complete_five_pair_automatic_search_cpu_qualification'
except Exception as error:
    r.update(status='failed', error=str(error))
    raise
finally:
    save()
