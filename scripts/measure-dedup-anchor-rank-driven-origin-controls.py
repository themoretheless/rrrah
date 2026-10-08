"""Fresh automatic rank-driven fallback search: positive plus all foreign origins."""
import hashlib
import json
import math
import subprocess
import time
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions

D = Path('docs/research')
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe = T / 'anchor-geometry-rank-fallback-files-probe-qualified'
manifest = T / 'prepared.json'
m = json.loads(manifest.read_text())
query = next(x for x in m['images']['strong'] if x['filename'] == '214402.jpg')
originals = sorted(m['images']['original'], key=lambda x: (x['group_id'] != query['group_id'], x['filename']))
assert len(originals) == 157
target = T / 'original-resolution-strong-all' / query['filename']
out = D / 'dedup-anchor-rank-driven-origin-controls.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
files = [exe, manifest, target, Path(__file__), Path('scripts/dedup_jpeg_domain.py'),
         D / 'dedup-anchor-rank-driven-fallback-real-source.json']
files.extend(T / 'original-resolution-negative-200201' / x['filename'] for x in originals)
pins = {str(p): sha(p) for p in files}
assert sha(target) == query['source_sha256']
for row in originals:
    assert sha(T / 'original-resolution-negative-200201' / row['filename']) == row['source_sha256']
r = {'status': 'running', 'required_pairs': 157, 'required_negatives': 156,
     'query': query['filename'], 'query_group': query['group_id'], 'results': [], 'pins': pins,
     'scope': 'Fresh native automatic symmetric/asymmetric candidate extraction and original-pixel rank-driven fallback anchors on occluded eyeglasses query versus all157 publisher origins. Complete union points, target2/source2 diagonal ratio, unchanged rank8/filter8/.005/1000/.3/.9 thresholds. Not borrowed geometry; no semantic/burst, collection, Windows or all-contract precision claim.'}
def save():
    tmp = out.with_suffix('.tmp')
    tmp.write_text(json.dumps(r, indent=2) + '\n')
    tmp.replace(out)
save()
td = oriented_dimensions(target, query['source_size'])
for original in originals:
    source = T / 'original-resolution-negative-200201' / original['filename']
    for p in (source, target, exe):
        assert sha(p) == pins[str(p)]
    sd = oriented_dimensions(source, original['source_size'])
    tolerance = 2 * math.hypot(*sd) / math.hypot(*td)
    start = time.perf_counter()
    process = subprocess.run([str(exe), str(source), str(target), str(tolerance)], capture_output=True, text=True)
    row = {'original': original['filename'], 'original_group': original['group_id'],
           'positive_control': original['group_id'] == query['group_id'], 'source_tolerance': tolerance,
           'seconds': time.perf_counter() - start, 'returncode': process.returncode,
           'stdout': process.stdout, 'stderr': process.stderr}
    if process.returncode == 0:
        row['evidence'] = json.loads(process.stdout)
    for p in (source, target, exe):
        assert sha(p) == pins[str(p)]
    r['results'].append(row)
    save()
assert all(sha(p) == digest for p, digest in pins.items())
r['status'] = 'complete'
save()
print('Completed157 fresh rank-driven fallback origin controls')
