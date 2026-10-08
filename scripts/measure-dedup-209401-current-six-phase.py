"""Current frozen six-phase diagnostic of a verified old three-phase miss."""
import hashlib, json, math, shutil, subprocess, time
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
D = Path('docs/research')
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
out = D / 'dedup-209401-current-six-phase.json'
binary = T / '209401-current-six-phase-probe-qualified'
assert not out.exists() and not binary.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest = D / 'dedup-collection-late-token-snapshot.json'
m = json.loads(manifest.read_text())
assert all(sha(Path(m['snapshot']) / p) == h for p, h in m['file_hashes'].items())
shutil.copy2('target/debug/examples/anchor_reflection_fallback_files_probe', binary)
files = [T / 'original-resolution-negative-200201/209400.jpg', T / 'original-resolution-strong-all/209401.jpg']
prepared = T / 'prepared.json'
records = {x['filename']: x for group in json.loads(prepared.read_text())['images'].values() for x in group}
dimensions = [oriented_dimensions(p, records[p.name]['source_size']) for p in files]
tolerance = 2 * math.hypot(*dimensions[0]) / math.hypot(*dimensions[1])
pins = {str(p):sha(p) for p in files + [binary, manifest, prepared, Path(__file__), D / 'dedup-209401-current-six-phase-build.log']}
r = dict(status='running', pins=pins, source_manifest=str(manifest), dimensions=dimensions, source_tolerance=tolerance, memory_ceiling=536870912, scope='One real crop/rephoto positive pair, current frozen native six-phase file fallback; all outcomes retained. Same original-pixel acceptance as old corpus. No recall/precision or independent pixel qualification.')
def save():
    temp = out.with_suffix('.json.tmp')
    temp.write_text(json.dumps(r, indent=2) + '\n')
    temp.replace(out)
save()
started = time.monotonic()
try:
    p = subprocess.run([str(binary), *map(str, files), str(tolerance)], capture_output=True, text=True)
    r.update(returncode=p.returncode, stdout=p.stdout, stderr=p.stderr, elapsed_seconds=time.monotonic()-started)
    if p.returncode == 0: r['evidence'] = json.loads(p.stdout)
    assert all(sha(p) == h for p,h in pins.items())
    assert all(sha(Path(m['snapshot']) / p) == h for p,h in m['file_hashes'].items())
    r['status'] = 'complete_current_six_phase_diagnostic'
finally:
    save()
print(r['status'], r.get('evidence'), flush=True)
