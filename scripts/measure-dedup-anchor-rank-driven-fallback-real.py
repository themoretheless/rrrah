"""Fresh automatic anchor-driven selection; corroborate selected recipe directly."""
import hashlib
import json
import subprocess
import time
from pathlib import Path

D = Path('docs/research')
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe = T / 'anchor-geometry-rank-fallback-files-probe-qualified'
direct = T / 'anchor-geometry-smoothing-files-probe-128m-qualified'
baseline = D / 'dedup-anchor-fallback-recovered-real.json'
manifest = D / 'dedup-anchor-rank-driven-fallback-real-source.json'
prior = json.loads(baseline.read_text())
out = D / 'dedup-anchor-rank-driven-fallback-real.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
files = [exe, direct, baseline, manifest, Path(__file__)]
for row in prior['results']:
    files.extend([T / 'original-resolution-negative-200201' / row['original'],
                  T / 'original-resolution-strong-all' / row['query']])
pins = {str(p): sha(p) for p in files}
r = {'status': 'running', 'required_pairs': 3, 'pins': pins, 'results': [],
     'scope': 'Three prior recovered real pairs through fresh native automatic rank-driven symmetric/asymmetric selection. No prior-selected recipe input. Each selected recipe separately corroborated by frozen direct geometry-only executable, exact complete points/model/anchor counts required. No broader recall/precision, collection, Windows or full-contract claim.'}
def save():
    tmp = out.with_suffix('.tmp')
    tmp.write_text(json.dumps(r, indent=2) + '\n')
    tmp.replace(out)
save()
for old in prior['results']:
    args = [str(T / 'original-resolution-negative-200201' / old['original']),
            str(T / 'original-resolution-strong-all' / old['query']), str(old['source_tolerance'])]
    start = time.perf_counter()
    p = subprocess.run([str(exe), *args], capture_output=True, text=True)
    row = {'original': old['original'], 'query': old['query'], 'source_tolerance': old['source_tolerance'],
           'seconds': time.perf_counter() - start, 'returncode': p.returncode, 'stdout': p.stdout, 'stderr': p.stderr}
    if not p.returncode:
        row['evidence'] = json.loads(p.stdout)
    r['results'].append(row)
    save()
    assert p.returncode == 0, p.stderr
    e = row['evidence']
    assert e['status'] == 'ok' and e['supported'], old['query']
    assert e['smoothing_radii'] == [[2, 2], [4, 0], [0, 4]][e['attempted_recipes'] - 1]
    p = subprocess.run([str(direct), *args, *map(str, e['smoothing_radii'])], capture_output=True, text=True)
    row['direct_returncode'] = p.returncode
    row['direct_stdout'] = p.stdout
    row['direct_stderr'] = p.stderr
    save()
    assert p.returncode == 0, p.stderr
    row['direct_evidence'] = json.loads(p.stdout)
    assert row['direct_evidence'] == {k: v for k, v in e.items() if k not in ('attempted_recipes', 'smoothing_radii')}
    assert all(sha(p) == digest for p, digest in pins.items())
    row['direct_parity_verified'] = True
    save()
r['status'] = 'complete'
save()
print([(x['query'], x['evidence']['attempted_recipes'], x['seconds']) for x in r['results']])
