"""Frozen real three-file indexed versus exhaustive fresh-file parity."""
import hashlib, json, math, os, shutil, subprocess, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
os.chdir(ROOT)
D = Path('docs/research')
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
output = D / 'dedup-anchor-rank-driven-real-collection-current.json'
snapshot = T / 'anchor-rank-driven-real-collection-current-source'
binary = T / 'anchor-rank-driven-real-collection-current-probe-qualified'
assert not output.exists() and not snapshot.exists() and not binary.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
files = [T / 'original-resolution-negative-200201/201300.jpg',
         T / 'original-resolution-strong-all/201303.jpg',
         T / 'original-resolution-negative-200201/201400.jpg']
source_files = sorted(p for p in Path('crates/rrrah-dedup').rglob('*') if p.is_file())
source_files += [Path('Cargo.lock'), Path('Cargo.toml'), Path(__file__),
                 D / 'dedup-anchor-rank-driven-real-collection-current-build.log']
source_pins = {}
for p in source_files:
    relative = p.resolve().relative_to(ROOT)
    dest = snapshot / relative
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(p, dest)
    source_pins[str(relative)] = sha(dest)
shutil.copy2('target/debug/examples/anchor_rank_driven_real_collection_probe', binary)
# One explicit collection tolerance, also used for all direct comparisons.
# It is scaled for the designated original/query; other pairs do not silently
# acquire a different recipe. No mixed-resolution calibration is claimed.
from dedup_jpeg_domain import oriented_dimensions
manifest = json.loads((T / 'prepared.json').read_text())
records = {x['filename']: x for group in manifest['images'].values() for x in group}
dimensions = [oriented_dimensions(p, records[p.name]['source_size']) for p in files]
tolerance = 2 * math.hypot(*dimensions[0]) / math.hypot(*dimensions[1])
pins = {str(p): sha(p) for p in files + [binary]}
report = dict(status='running', pins=pins, source_snapshot=str(snapshot),
              source_hashes=source_pins, dimensions=dimensions,
              source_tolerance=tolerance, expected_positive=[1, 2],
              scope='One real original/copy/foreign collection and all three fresh direct pairs. Exact retained anchors, original points, geometry and recipe metadata parity; missing indexed pairs must be direct negatives. One explicit collection tolerance. Frozen dedup and lock files, not a full dependency-source or platform/precision/recall qualification.')
def save():
    temporary = output.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    os.replace(temporary, output)
save()
started = time.monotonic()
result = subprocess.run([str(binary), *map(str, files), str(tolerance)],
                        capture_output=True, text=True)
report.update(returncode=result.returncode, stdout=result.stdout,
              stderr=result.stderr, elapsed_seconds=time.monotonic() - started)
try:
    assert all(sha(p) == digest for p, digest in pins.items())
    assert all(sha(snapshot / p) == digest for p, digest in source_pins.items())
    assert result.returncode == 0, result.stderr
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    assert [(v['left'], v['right']) for v in rows[:-1]] == [(1, 2), (1, 3), (2, 3)]
    assert all(v['direct_parity'] for v in rows[:-1])
    assert [v['supported'] for v in rows[:-1]] == [True, False, False]
    assert rows[0]['indexed'] and rows[-1]['memory_used'] == 0
    report['results'] = rows
    report['status'] = 'complete_real_three_file_indexed_parity'
except Exception as error:
    report['status'] = 'failed_real_three_file_indexed_parity'
    report['error'] = f'{type(error).__name__}: {error}'
    raise
finally:
    save()
print(report['status'], flush=True)
