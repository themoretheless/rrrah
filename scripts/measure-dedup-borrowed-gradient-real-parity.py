"""Complete native evidence parity after borrowed descriptor changes."""
import hashlib, json, shutil, subprocess, time
from pathlib import Path
D = Path('docs/research')
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
out = D / 'dedup-borrowed-gradient-real-parity.json'
binary = T / 'borrowed-gradient-anchor-files-probe-qualified'
snapshot = T / 'borrowed-gradient-real-parity-source'
assert not out.exists() and not binary.exists() and not snapshot.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
baseline = D / 'dedup-anchor-rank-driven-fallback-real.json'
prior = json.loads(baseline.read_text())
assert len(prior['results']) == 3
shutil.copy2('target/debug/examples/anchor_geometry_rank_fallback_files_probe', binary)
source_pins = {}
for p in Path('crates/rrrah-dedup').rglob('*'):
    if p.is_file():
        dest = snapshot / p
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p, dest)
        source_pins[str(p)] = sha(dest)
files = [baseline, binary, Path(__file__), D / 'dedup-borrowed-gradient-real-parity-build.log']
for row in prior['results']:
    files += [T / 'original-resolution-negative-200201' / row['original'],
              T / 'original-resolution-strong-all' / row['query']]
pins = {str(p): sha(p) for p in files}
r = dict(status='running', results=[], pins=pins, source_snapshot=str(snapshot),
         source_hashes=source_pins, required_pairs=3,
         scope='Exact whole native evidence versus frozen prior automatic fallback on three real positives. No broader recall/precision, collection, timing speedup or independent pixel recomputation; dedup source pins exclude external dependencies.')
def save():
    temporary = out.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(r, indent=2) + '\n')
    temporary.replace(out)
save()
try:
    for old in prior['results']:
        args = [str(T / 'original-resolution-negative-200201' / old['original']),
                str(T / 'original-resolution-strong-all' / old['query']), str(old['source_tolerance'])]
        started = time.monotonic()
        p = subprocess.run([str(binary), *args], capture_output=True, text=True)
        row = dict(original=old['original'], query=old['query'], source_tolerance=old['source_tolerance'],
                   returncode=p.returncode, stdout=p.stdout, stderr=p.stderr,
                   elapsed_seconds=time.monotonic() - started, exact_parity=False)
        r['results'].append(row)
        save()
        assert p.returncode == 0, p.stderr
        evidence = json.loads(p.stdout)
        assert evidence == old['evidence'], old['query']
        row.update(evidence=evidence, exact_parity=True)
        save()
        print(old['query'], 'exact parity', flush=True)
    assert all(sha(p) == digest for p, digest in pins.items())
    assert all(sha(snapshot / p) == digest for p, digest in source_pins.items())
    r['status'] = 'complete_borrowed_gradient_real_parity'
except Exception as error:
    r['status'] = 'failed_borrowed_gradient_real_parity'
    r['error'] = f'{type(error).__name__}: {error}'
    raise
finally:
    save()
