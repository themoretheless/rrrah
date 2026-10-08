"""Training-vs-heldout diagnostic on unchanged, prequalified native witnesses."""
import hashlib
import json
import math
import shutil
import subprocess
from pathlib import Path

base = Path('docs/research')
root = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
reference = base / 'dedup-folded-witness-common-footprint-regions.json'
audit = base / 'dedup-folded-witness-common-footprint-regions-audit.json'
old = json.loads(reference.read_text())
qualified = json.loads(audit.read_text())
assert old['status'] == 'complete_native_experiment'
assert qualified['status'] == 'verified_folded_witness_common_footprint_regions'
assert qualified['report_sha256'] == digest(reference)
assert len(old['rows']) == qualified['cases'] == 6
control = base / 'dedup-common-footprint-screen-control.json'
control_old = json.loads(control.read_text())
assert control_old['status'] == 'verified_common_footprint_control'
exe = root / 'affine-color-training-diagnostic'
assert not exe.exists()
shutil.copy2('target/debug/examples/affine_color_training_diagnostic', exe)
left = root / 'original-resolution-negative-200201/201700.jpg'
right = root / 'original-resolution-strong-all/201702.jpg'
screen_left = root / 'original-resolution-negative-200201/200500.jpg'
screen_right = root / 'original-resolution-strong-all/200501.jpg'
screen_input = root / 'guarded-roi-screen-control.txt'
cases = [('screen_control', screen_left, screen_right, screen_input, control_old['evidence'])]
for index, row in enumerate(old['rows']):
    path = Path(row['input'])
    assert digest(path) == row['input_sha256']
    cases.append((f'folded_{index}', left, right, path, row['evidence']))
paths = [reference, audit, control, exe, Path(__file__),
         Path('crates/rrrah-dedup/examples/affine_color_training_diagnostic.rs'),
         *[Path('crates/rrrah-dedup/src') / name for name in
           ['affine_region.rs', 'affine_region_file.rs', 'affine_color.rs', 'lib.rs']]]
paths += [path for _, a, b, p, _ in cases for path in [a, b, p]]
pins = {str(path): digest(path) for path in paths}
output = base / 'dedup-folded-training-diagnostic.json'
assert not output.exists()
report = {'status': 'running', 'required_cases': 7, 'input_hashes': pins, 'rows': [],
          'scope': 'One screen control and six unchanged supplied witness geometries. Training scores are diagnostic only; heldout thresholds and all original policies remain unchanged. Not automatic fold recovery or broad precision.'}

def save():
    temporary = output.with_suffix('.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    temporary.replace(output)

save()
for name, a, b, path, expected in cases:
    result = subprocess.run([str(exe), str(a), str(b), str(path)], capture_output=True, text=True)
    assert all(digest(k) == v for k, v in pins.items())
    row = {'case': name, 'input': str(path), 'returncode': result.returncode,
           'stdout': result.stdout, 'stderr': result.stderr}
    report['rows'].append(row)
    save()
    assert result.returncode == 0
    actual = json.loads(result.stdout)
    row['evidence'] = actual
    assert actual['status'] == expected['status']
    assert actual['managed_used'] == 0
    assert actual['managed_peak'] == expected['managed_peak']
    assert actual['footprints'] == expected['footprints'] == ['target', 'source']
    assert len(actual['directions']) == len(expected['directions'])
    for measured, prior in zip(actual['directions'], expected['directions']):
        for key, value in prior.items():
            assert measured[key] == value, (name, key)
        assert 0 <= measured['training_matched'] <= measured['training_samples']
        for key in ['training_squared_error', 'heldout_squared_error']:
            assert math.isfinite(measured[key]) and measured[key] >= 0
    row['original_evidence_parity'] = True
    save()
    print(name, actual['status'], flush=True)
report['status'] = 'complete_native_diagnostic'
save()
