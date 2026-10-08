"""Verify terminal native Linux input-contract tests and immutable source bytes."""
import argparse
import hashlib
import json
import re
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('returncode', type=int)
parser.add_argument('output', type=Path)
args = parser.parse_args()
assert args.returncode == 0 and not args.output.exists()
base = Path('docs/research')
manifest = base / 'dedup-affine-input-linux-snapshot.json'
log = base / 'dedup-affine-input-linux-native.log'
digest = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
record = json.loads(manifest.read_text())
snapshot = Path(record['snapshot'])
for name, expected in record['file_hashes'].items():
    assert digest(snapshot / name) == expected, name
text = log.read_text()
assert re.findall(r'Running tests/([a-z_]+)\.rs', text) == ['affine_region_input_contract']
assert re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;', text) == [('2', '0', '0', '0', '0')]
assert 'unsupported_alpha_and_scene_range_refuse_in_either_direction_without_clipping ... ok' in text
assert 'overflowed_domains_and_radius_refuse_before_allocation ... ok' in text
paths = [manifest, log, Path(__file__), Path('scripts/run-dedup-linux-affine-input.sh')]
current = {}
for name in ['src/affine_region.rs', 'tests/affine_region_input_contract.rs']:
    path = Path('crates/rrrah-dedup') / name
    assert digest(path) == record['file_hashes'][str(path)]
    current[str(path)] = digest(path)
args.output.write_text(json.dumps({
    'status': 'verified_native_linux_affine_input_contract',
    'returncode': args.returncode,
    'tests': 2,
    'verified_snapshot_files': len(record['file_hashes']),
    'input_hashes': {str(path): digest(path) for path in paths},
    'current_code_test_hashes': current,
    'scope': 'Native Linux aarch64 input refusal and managed-credit contracts only. Unsupported HDR/alpha refusal is not HDR/alpha copy-search coverage.'
}, indent=2) + '\n')
