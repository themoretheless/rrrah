#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import re

ROOT = Path(__file__).resolve().parents[1]
RESEARCH = ROOT / 'docs/research'
GATE = RESEARCH / 'dedup-complete-collection-baseline-v2-20261015.json'
OUTPUT = RESEARCH / 'dedup-baseline-container-early-retention-20261015.json'
RETAINED = Path('/private/tmp/rrrah-baseline-container-early-retained-20261015')

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

assert not OUTPUT.exists()
gate = json.loads(GATE.read_text())
manifest_path = Path(gate['source_manifest'])
assert sha(manifest_path) == gate['source_manifest_sha256']
manifest = json.loads(manifest_path.read_text())
for row in manifest['source_files']:
    assert sha(Path(manifest['source']) / row['path']) == row['sha256']

copies = [p for p in RETAINED.glob('container-*') if p.is_file() and not p.suffix]
assert len(copies) == 1
copy = copies[0]
original = Path('/private/tmp/rrrah-consensus-collection-target-20261014/debug/deps') / copy.name
assert sha(copy) == sha(original)
inventory = RESEARCH / 'dedup-baseline-container-early-inventory-20261015.log'
names = re.findall(r'^(.+): test$', inventory.read_text(), re.M)
assert names and len(names) == len(set(names))
report = {
    'status': 'compiled_target_retained_inventory_only_not_tests_executed',
    'producer_sha256': sha(__file__),
    'gate_report': str(GATE.relative_to(ROOT)),
    'source_manifest_sha256': sha(manifest_path),
    'crate': 'container',
    'owned_compiler_pid_observed': 76495,
    'parent_cargo_pid': 67374,
    'original': str(original),
    'retained': str(copy),
    'executable_sha256': sha(copy),
    'bytes': copy.stat().st_size,
    'inventory': str(inventory.relative_to(ROOT)),
    'inventory_sha256': sha(inventory),
    'inventory_names': names,
    'recovery_note': 'Original inline retention copied and hash-checked the binary and successfully listed tests, then report emission called st_size integer as function. This producer preserves existing copy/list and verifies all source/binary/list pins before emitting report; no tests or fullgate restarted.',
    'scope': 'Fresh container executable from observed owned baseline compiler retained with inventory. Original immutable3378source manifest rechecked. Inventory only, not executed tests, compiler speedup, currentRoot or complete goal proof.'
}
OUTPUT.write_text(json.dumps(report, indent=2) + '\n')
print(f'Retained container target: {len(names)} listed tests; no execution claim')
