"""Verify the complete current cache integration inventory and pinned library."""
from pathlib import Path
import collections
import hashlib
import json
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
prepared = D / 'dedup-cache-corruption-all-bits-prepared-20261015.json'
plan = json.loads(prepared.read_text())
work = Path(plan['source'])
assert sha(work / 'cache-integration.rs') == plan['source_sha256']
assert sha(ROOT / 'crates/rrrah-dedup/tests/cache.rs') == plan['root_test_sha256'] == plan['source_sha256']
assert sha(ROOT / 'crates/rrrah-dedup/src/cache.rs') == plan['production_cache_sha256']
assert sha(ROOT / plan['previous_qualified_library_report']) == plan['previous_report_sha256']
previous = json.loads((ROOT / plan['previous_qualified_library_report']).read_text())
assert previous['production_cache_sha256'] == plan['production_cache_sha256']
assert sha(plan['library']) == plan['library_sha256'] == previous['library_sha256']
assert json.loads((work / 'command.json').read_text()) == plan['command']
binary = work / 'tests'
inventory = subprocess.run([str(binary), '--list'], capture_output=True, text=True, check=True).stdout
names = re.findall(r'^(.+): test$', inventory, re.M)
assert len(names) == len(set(names)) == 10
assert set(previous['complete_test_inventory']).issubset(names)
assert set(names) - set(previous['complete_test_inventory']) == {
    'every_serialized_bit_corruption_is_rejected',
    'zero_budget_and_owned_values_preserve_previous_entry',
}
log = (work / 'run.log').read_text()
results = re.findall(r'^test (\S+) \.\.\. (ok|FAILED|ignored)$', log, re.M)
assert collections.Counter(name for name, _ in results) == collections.Counter(names)
assert all(status == 'ok' for _, status in results)
assert 'test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;' in log
saved = D / 'dedup-cache-corruption-all-bits-reproducer-20261015'
assert not saved.exists()
saved.mkdir()
for name in ['cache-integration.rs', 'command.json', 'compile.log', 'inventory.log', 'run.log']:
    shutil.copyfile(work / name, saved / name)
output = D / 'dedup-cache-corruption-all-bits-audit-20261015.json'
assert not output.exists()
output.write_text(json.dumps({
    'status': 'verified_complete_ten_cache_integration_tests',
    'passed': 10, 'failed': 0, 'ignored': 0, 'filtered': 0,
    'corruption_oracles': 3264, 'existing_wire_bit_oracles': 2048,
    'inventory': names,
    'prepared_sha256': sha(prepared), 'auditor_sha256': sha(__file__),
    'binary': str(binary), 'binary_sha256': sha(binary),
    'library': plan['library'], 'library_sha256': sha(plan['library']),
    'root_source_pins': {path: sha(ROOT / path) for path in ['crates/rrrah-dedup/src/cache.rs', 'crates/rrrah-dedup/tests/cache.rs']},
    'retained_evidence_pins': {str(path.relative_to(ROOT)): sha(path) for path in saved.iterdir()},
    'scope': 'Current complete cache integration target against unchanged previously qualified actual stack-cache/pixels selected library and real cached dependencies on MacARM. Includes atomic publication and process controls. No fresh full Cargo/library/decoder/platform/corpus/performance or hard-memory-limit qualification.',
}, indent=2) + '\n')
print(output, '10 tests passed')
