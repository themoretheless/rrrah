"""Fresh explicit asymmetric native searches; legacy-fallback-selected recipes."""
import hashlib
import json
import subprocess
import time
from pathlib import Path

D = Path('docs/research')
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe = T / 'anchor-geometry-smoothing-files-probe-128m-qualified'
baseline = D / 'dedup-anchor-fallback-recovered-real.json'
manifest = D / 'dedup-anchor-geometry-smoothing-real-source-128m.json'
old = json.loads(baseline.read_text())
assert old['status'] == 'complete' and len(old['results']) == 3
out = D / 'dedup-anchor-geometry-smoothing-real-parity-128m.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
files = [exe, baseline, manifest, Path(__file__)]
for row in old['results']:
    files.extend([T / 'original-resolution-negative-200201' / row['original'],
                  T / 'original-resolution-strong-all' / row['query']])
pins = {str(p): sha(p) for p in files}
r = {'status': 'running', 'required_pairs': 3, 'pins': pins, 'results': [],
     'scope': 'Three recovered real pairs; explicit asymmetric radii selected by prior frozen color-driven fallback, then fresh native geometry-only extraction and original-file anchors. Exact complete geometry/point/count parity, no borrowed points/model; no automatic rank-driven fallback, relative timing or broad precision claim.'}
def save():
    tmp = out.with_suffix('.tmp')
    tmp.write_text(json.dumps(r, indent=2) + '\n')
    tmp.replace(out)
save()
for prior in old['results']:
    source = T / 'original-resolution-negative-200201' / prior['original']
    target = T / 'original-resolution-strong-all' / prior['query']
    radii = prior['evidence']['smoothing_radii']
    start = time.perf_counter()
    process = subprocess.run([str(exe), str(source), str(target), str(prior['source_tolerance']),
                              *map(str, radii)], capture_output=True, text=True)
    seconds = time.perf_counter() - start
    assert process.returncode == 0, process.stderr
    e = json.loads(process.stdout)
    expected = {k: v for k, v in prior['evidence'].items() if k not in ('attempted_recipes', 'smoothing_radii')}
    assert e == expected, prior['query']
    assert all(sha(p) == digest for p, digest in pins.items())
    r['results'].append({'original': prior['original'], 'query': prior['query'],
                         'source_tolerance': prior['source_tolerance'], 'smoothing_radii': radii,
                         'seconds': seconds, 'returncode': process.returncode, 'stderr': process.stderr,
                         'evidence': e})
    save()
r['status'] = 'complete'
save()
print([(x['query'], x['seconds']) for x in r['results']])
