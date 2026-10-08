"""Test bounded affine-color grids on eight independently qualified local models."""
import hashlib
import json
import subprocess
from pathlib import Path

base = Path('docs/research')
root = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
reference = base / 'dedup-folded-201702-local-models-pixels.json'
previous = json.loads(reference.read_text())
assert previous['status'] == 'complete_native_experiment'
assert all(digest(path) == value for path, value in previous['input_hashes'].items())
experiments = json.loads(previous['stdout'])['experiments']
assert len(experiments) == 8
parity = base / 'dedup-affine-grid-release-parity.json'
qualified = json.loads(parity.read_text())
assert qualified['status'] == 'verified_three_case_semantic_parity'
assert all(digest(path) == value for path, value in qualified['input_hashes'].items())
exe = root / 'affine-color-grid-image-probe-screen-meanfix-release'
left = root / 'original-resolution-negative-200201/201700.jpg'
right = root / 'original-resolution-strong-all/201702.jpg'
assert digest(exe) == qualified['input_hashes'][str(exe)]
pins = {str(path): digest(path) for path in [reference, parity, exe, left, right, Path(__file__)]}
output = base / 'dedup-folded-affine-grid.json'
assert not output.exists()
report = {
    'status': 'running', 'required_models': 8, 'input_hashes': pins, 'rows': [],
    'scope': 'Eight native local geometric models followed by bounded automatic affine-color grid. Count supports only for target cells wholly inside the fitting domain. No fresh integrated piecewise search, nonrigid coverage or precision proof.'
}
def save():
    temporary = output.with_suffix('.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    temporary.replace(output)
save()
for index, model in enumerate(experiments):
    path = root / f'folded-affine-grid-model-{index}.txt'
    assert not path.exists()
    path.write_text(' '.join(str(value) for row in model['matrix'] for value in row) + '\n')
    before = digest(path)
    result = subprocess.run([str(exe), str(left), str(right), str(path)], capture_output=True, text=True)
    assert digest(path) == before and all(digest(path) == value for path, value in pins.items())
    row = {'index': index, 'fit_target_domain': model['fit_target_domain'], 'matrix': model['matrix'], 'input': str(path), 'input_sha256': before, 'returncode': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr}
    if result.returncode == 0:
        evidence = json.loads(result.stdout)
        assert evidence['managed_used'] == 0
        row['evidence'] = evidence
        x, y, width, height = model['fit_target_domain']
        supports = []
        for region in evidence['regions']:
            tx, ty, tw, th = region['domains'][1]
            if x <= tx and y <= ty and tx + tw <= x + width and ty + th <= y + height:
                if all('matched' in direction and direction['matched'] >= .9 * direction['samples'] for direction in region['directions']):
                    supports.append(region)
        row['local_supports'] = supports
    report['rows'].append(row)
    save()
    print(index, row.get('evidence', {}).get('status'), len(row.get('local_supports', [])), flush=True)
report['status'] = 'complete_native_experiment'
save()
