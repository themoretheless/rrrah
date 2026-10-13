"""Library scored-proposal API and Rust geometry and original-pixel confirmation.

Reuses all four archived Rust LoFTR outputs. Python only supplies declared fixture
dimensions and serializes native output; no OpenCV fit or point selection.
"""
import hashlib
import json
import math
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
WORK = T / 'native-loftr-inputs-20261010'
OUT = D / 'dedup-loftr-scored-proposals-managed-20261010.json'
GEOMETRY = WORK / 'geometry-scored-proposals-managed-qualified'
assert not OUT.exists() and not GEOMETRY.exists()
shutil.copy2('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-target-20261010/release/geometry', GEOMETRY)
CONFIRM = T / 'supplied-anchor-files-probe-qualified-20261010'
MANIFEST = WORK / 'manifest.json'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior_path = D / 'dedup-loftr-native-20261010.json'
prior = json.loads(prior_path.read_text())
rows = json.loads(MANIFEST.read_text())
files = [GEOMETRY, CONFIRM, MANIFEST, prior_path, Path(__file__),
         ROOT / 'scripts/dedup-loftr-native-probe/Cargo.lock',
         ROOT / 'scripts/dedup-loftr-native-probe/Cargo.toml',
         ROOT / 'scripts/dedup-loftr-native-probe/src/bin/geometry.rs',
         ROOT / 'Cargo.toml', ROOT / 'crates/rrrah-dedup/Cargo.toml',
         ROOT / 'crates/rrrah-dedup/build.rs', ROOT / 'crates/rrrah-dedup/build_context.rs', D / 'dedup-scored-proposals-managed-native-build-retry-20261010.log']
files.extend((ROOT / 'crates/rrrah-dedup/src').rglob('*.rs'))
for index, row in enumerate(rows):
    proposal = WORK / f'{index}-native-proposals.json'
    assert sha(proposal) == prior['pins'][str(proposal)]
    files.append(proposal)
    files.extend(Path(x['source']) for x in row['inputs'])
pins = {str(p):sha(p) for p in files}
r = dict(status='running', pins=pins, results=[], scope=__doc__)
def save():
    temp = OUT.with_suffix('.json.tmp')
    temp.write_text(json.dumps(r, indent=2)+'\n')
    temp.replace(OUT)
save()
try:
    for index, row in enumerate(rows):
        started = time.monotonic()
        dimensions = [x[a] for x in row['inputs'] for a in ('width','height')]
        dimensions += [2560,1920,1024,701]
        proposal = WORK / f'{index}-native-proposals.json'
        p = subprocess.run([str(GEOMETRY), str(proposal), *map(str,dimensions)], capture_output=True, text=True)
        result = dict(side=row['side'], clahe=row['clahe'], geometry_returncode=p.returncode, geometry_stderr=p.stderr)
        r['results'].append(result)
        save()
        if p.returncode:
            raise RuntimeError('native geometry failure')
        geometry = json.loads(p.stdout)
        artifact = WORK / f'{index}-scored-proposals-managed-qualified.json'
        assert not artifact.exists()
        artifact.write_text(p.stdout)
        pins[str(artifact)] = sha(artifact)
        result.update(status=geometry['status'], selected_points=len(geometry['points']))
        if geometry['status'] == 'geometry':
            model = artifact.with_suffix('.model.txt')
            points = artifact.with_suffix('.points.txt')
            model.write_text(' '.join(str(x) for row in geometry['matrix'] for x in row)+'\n')
            points.write_text(''.join(' '.join(map(str,[*p,*q]))+'\n' for p,q in geometry['points']))
            pins[str(model)] = sha(model)
            pins[str(points)] = sha(points)
            result.update(hypotheses=geometry['hypotheses'], forward_inliers=len(geometry['inliers']))
            q = subprocess.run([str(CONFIRM), *[x['source'] for x in row['inputs']], str(model), str(points), str(2*math.hypot(2560,1920)/math.hypot(1024,701))], capture_output=True, text=True)
            result.update(confirmation_returncode=q.returncode, confirmation_stderr=q.stderr, confirmation_stdout=q.stdout)
            if q.returncode:
                raise RuntimeError('native original-pixel confirmation failure')
            result['evidence'] = json.loads(q.stdout)
        result['elapsed_seconds'] = time.monotonic()-started
        save()
        print(row['side'],row['clahe'],result.get('evidence',result['status']),flush=True)
    assert all(sha(p)==h for p,h in pins.items())
    r['status'] = 'complete_rust_geometry_diagnostic'
except Exception as error:
    r.update(status='failed_rust_geometry_diagnostic',error=str(error))
    raise
finally:
    save()
