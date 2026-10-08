"""Finite, training-only translation study; never a new copy-admission policy."""
import hashlib
import json
import math
import subprocess
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions

base = Path('docs/research')
root = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
geometry_path = base / 'dedup-folded-201702-witness-geometry.json'
geometry_audit = base / 'dedup-folded-201702-witness-geometry-audit.json'
diagnostic_path = base / 'dedup-folded-training-diagnostic.json'
diagnostic_audit = base / 'dedup-folded-training-diagnostic-audit.json'
geometry = json.loads(geometry_path.read_text())
assert geometry['status'] == 'verified_native_witness_geometry'
diagnostic = json.loads(diagnostic_path.read_text())
qualified = json.loads(diagnostic_audit.read_text())
assert qualified['status'] == 'verified_training_diagnostic_parity'
assert qualified['input_hashes'][str(diagnostic_path)] == digest(diagnostic_path)
exe = root / 'affine-color-training-diagnostic'
assert diagnostic['input_hashes'][str(exe)] == digest(exe)
points_path = base / 'dedup-folded-201702-regional-points.txt'
points = [list(map(float, line.split())) for line in points_path.read_text().splitlines()]
assert len(points) == 289 and all(len(p) == 4 for p in points)
left = root / 'original-resolution-negative-200201/201700.jpg'
right = root / 'original-resolution-strong-all/201702.jpg'
prepared = root / 'prepared.json'
manifest = json.loads(prepared.read_text())
item = next(v for v in manifest['images']['original'] if v['filename'] == left.name)
assert digest(left) == item['source_sha256']
sw, sh = oriented_dimensions(left, item['source_size'])
paths = [geometry_path, geometry_audit, diagnostic_path, diagnostic_audit,
         points_path, exe, left, right, prepared, Path(__file__),
         Path(__file__).with_name('dedup_jpeg_domain.py')]
pins = {str(path): digest(path) for path in paths}
output = base / 'dedup-folded-training-shifts.json'
assert not output.exists()
report = {'status': 'running', 'input_hashes': pins, 'required_models': 6,
          'rows': [], 'selections': [], 'scope': 'Supplied native models translated by at most one query pixel. Fixed target witness and fixed expanded source rectangle per model. Selection uses forward training RMS only among equal-forward-sample-count candidates with at least ten original native correspondences within two pixels. Heldout color scores never choose the shift. Diagnostic only; no automatic search, default changes or broad precision.'}

def save():
    temporary = output.with_suffix('.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    temporary.replace(output)

def apply(h, p):
    denominator = sum(h[2][i] * [*p, 1.][i] for i in range(3))
    assert denominator != 0 and math.isfinite(denominator)
    return [sum(h[j][i] * [*p, 1.][i] for i in range(3)) / denominator for j in range(2)]

save()
models = [row for row in geometry['evidence']['regions'] if row['matrix'] is not None]
assert len(models) == 6
for index, native in enumerate(models):
    input_path = Path(diagnostic['rows'][index + 1]['input'])
    values = list(map(float, input_path.read_text().split()))
    assert values[:9] == [v for axis in native['matrix'] for v in axis]
    sx, sy, w, h = map(int, values[9:13])
    # One query pixel can span several source pixels; expansion is fixed before
    # measurements, and all shift candidates must retain baseline sample counts.
    x0, y0 = max(0, sx - 32), max(0, sy - 32)
    source = [x0, y0, min(sw, sx + w + 32) - x0, min(sh, sy + h + 32) - y0]
    target = list(map(int, values[13:]))
    shifts = [(0, 0), *[(dx, dy) for dy in [-1, 0, 1] for dx in [-1, 0, 1] if (dx, dy) != (0, 0)]]
    baseline_counts = None
    eligible = []
    for dx, dy in shifts:
        matrix = [axis[:] for axis in native['matrix']]
        matrix[0] = [a + dx * b for a, b in zip(matrix[0], matrix[2])]
        matrix[1] = [a + dy * b for a, b in zip(matrix[1], matrix[2])]
        inliers = [i for i in native['point_indices'] if math.dist(apply(matrix, points[i][:2]), points[i][2:]) <= 2.]
        row = {'model_index': index, 'shift': [dx, dy], 'matrix': matrix,
               'source_domain': source, 'target_domain': target, 'inliers': inliers}
        report['rows'].append(row)
        if len(inliers) < 10:
            row['status'] = 'insufficient_geometric_inliers'
            save()
            continue
        path = root / f'fold-training-shift-{index}-{dx}-{dy}.txt'
        assert not path.exists()
        path.write_text(' '.join(map(str, [*[v for axis in matrix for v in axis], *source, *target])) + '\n')
        pin = digest(path)
        result = subprocess.run([str(exe), str(left), str(right), str(path)], capture_output=True, text=True)
        assert digest(path) == pin and all(digest(k) == v for k, v in pins.items())
        row.update(input=str(path), input_sha256=pin, returncode=result.returncode,
                   stdout=result.stdout, stderr=result.stderr)
        save()
        assert result.returncode == 0
        actual = json.loads(result.stdout)
        row.update(status=actual['status'], evidence=actual)
        assert actual['managed_used'] == 0
        if actual['status'] == 'ok':
            forward = actual['directions'][0]
            counts = [forward['training_samples'], forward['samples']]
            if (dx, dy) == (0, 0):
                baseline_counts = counts
            row['same_forward_counts'] = counts == baseline_counts
            if row['same_forward_counts']:
                loss = forward['training_squared_error'] / forward['training_samples']
                assert math.isfinite(loss) and loss >= 0
                row['training_loss'] = loss
                eligible.append((loss, abs(dx) + abs(dy), dx, dy, len(report['rows']) - 1))
        save()
        print(index, dx, dy, row['status'], flush=True)
    if eligible:
        chosen = min(eligible)
        report['selections'].append({'model_index': index, 'row_index': chosen[-1],
                                     'training_loss': chosen[0], 'shift': [chosen[2], chosen[3]]})
    save()
assert len(report['rows']) == 54
report['status'] = 'complete_training_shift_diagnostic'
save()
