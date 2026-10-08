"""Retain all automatic-search outcomes on independent rotation fixtures."""
import hashlib, json, math, shutil, struct, subprocess, time
from pathlib import Path
D = Path('docs/research')
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
root = Path('crates/rrrah-dedup/tests/fixtures/rotation')
out = D / 'dedup-anchor-rotation-current.json'
binary = T / 'anchor-rank-current-rotation-probe-qualified'
snapshot = T / 'anchor-rotation-current-source'
assert not out.exists() and not binary.exists() and not snapshot.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
shutil.copy2('target/debug/examples/anchor_geometry_rank_fallback_files_probe', binary)
source_hashes = {}
for p in Path('crates/rrrah-dedup').rglob('*'):
    if p.is_file():
        dest = snapshot / p
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p, dest)
        source_hashes[str(p)] = sha(dest)
names = ['base', 'angle-17', 'angle--37', 'angle-63', 'scale-117-angle-0',
         'scale-216-angle-0', 'scale-216-angle-17', 'mirrored', 'unrelated']
files = [root / (name + '.png') for name in names]
pins = {str(p): sha(p) for p in files + [binary, root / 'manifest.json', Path(__file__),
                                        D / 'dedup-anchor-rotation-current-build.log']}
def dimensions(p):
    data = p.read_bytes()
    assert data[:8] == b'\x89PNG\r\n\x1a\n' and data[12:16] == b'IHDR'
    return struct.unpack('>II', data[16:24])
r = dict(status='running', required_pairs=len(files), results=[], pins=pins,
         source_snapshot=str(snapshot), source_hashes=source_hashes,
         scope='Independent fixed opaque rotation/scale/reflection/control fixtures under current automatic rank-driven policy; all errors/refusals/misses retained. No real corpus, independent pixel recomputation, collection or general angular coverage claim.')
def save():
    temporary = out.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(r, indent=2) + '\n')
    temporary.replace(out)
save()
try:
    for target in files:
        tolerance = 2 * math.hypot(*dimensions(files[0])) / math.hypot(*dimensions(target))
        started = time.monotonic()
        p = subprocess.run([str(binary), str(files[0]), str(target), str(tolerance)], capture_output=True, text=True)
        row = dict(query=target.name, source_tolerance=tolerance, returncode=p.returncode,
                   stdout=p.stdout, stderr=p.stderr, elapsed_seconds=time.monotonic() - started)
        if p.returncode == 0:
            row['evidence'] = json.loads(p.stdout)
        r['results'].append(row)
        save()
        print(target.name, row.get('evidence', {}).get('status', 'native_error'),
              row.get('evidence', {}).get('supported', False), flush=True)
    assert all(sha(p) == digest for p, digest in pins.items())
    assert all(sha(snapshot / p) == digest for p, digest in source_hashes.items())
    r['status'] = 'complete_rotation_fixture_diagnostic'
except Exception as error:
    r.update(status='failed_rotation_fixture_diagnostic', error=f'{type(error).__name__}: {error}')
    raise
finally:
    save()
