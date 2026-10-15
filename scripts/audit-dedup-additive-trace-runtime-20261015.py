"""Account for the complete original portfolio inventory and two added controls."""
from pathlib import Path
import hashlib
import json
import re
import runpy
import subprocess

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
report = D / 'dedup-additive-trace-runtime-20261015.json'
data = json.loads(report.read_text())
assert data['status'] in ('terminal_pending_inventory_audit', 'terminal_failure')
assert sha(ROOT / 'scripts/qualify-dedup-additive-trace-runtime-20261015.py') == data['producer_sha256']
work = Path('/private/tmp/rrrah-additive-trace-full-production-metadata-20261015')
assert sha(work / 'plan.json') == data['metadata_plan_sha256']
plan = json.loads((work / 'plan.json').read_text())
for path, digest in plan['source_pins'].items():
    assert sha(work / path) == digest, path
for path, digest in plan['dependency_pins'].items():
    assert sha(path) == digest, path
source = ROOT / 'crates/rrrah-dedup/tests/search_portfolio.rs'
assert sha(source) == data['original_integration_sha256']
integration = work / 'search_portfolio-integration.rs'
assert sha(integration) == data['candidate_integration_sha256']
assert integration.read_bytes().startswith(source.read_bytes() + b'\n')
assert [row['phase'] for row in data['results']] == [
    'building_candidate_library', 'building_complete_integration',
    'listing_complete_integration', 'running_complete_integration',
]
assert all(row['returncode'] == 0 for row in data['results'][:-1])
for row in data['results']:
    assert sha(row['log']) == row['log_sha256']

# The complete prior target supplies an independent executable inventory.
original_audit_path = D / 'dedup-complete-collection-terminal-audit-20261015.json'
original_audit = json.loads(original_audit_path.read_text())
assert sha(original_audit['producer_report']) == original_audit['producer_report_sha256']
original = json.loads(Path(original_audit['producer_report']).read_text())
old_target = next(row for row in original['retained_binaries']
                  if Path(row['original']).name.startswith('search_portfolio-'))
assert sha(old_target['copy']) == old_target['sha256']
assert sha(old_target['inventory']) == old_target['inventory_sha256']
old_inventory = subprocess.run([old_target['copy'], '--list'], capture_output=True, text=True, check=True).stdout
old_names = set(re.findall(r'^(.+): test$', old_inventory, re.M))
assert old_names == set(re.findall(r'^(.+): test$', Path(old_target['inventory']).read_text(), re.M))
old_source = Path(original['source']) / 'crates/rrrah-dedup/tests/search_portfolio.rs'
manifest = json.loads((D / 'dedup-committed-build-source-415443172-v2-20261015.json').read_text())
assert sha(old_source) == next(row['sha256'] for row in manifest['source_files']
                               if row['path'] == 'crates/rrrah-dedup/tests/search_portfolio.rs')
current = source.read_text()
start = current.index('#[test]\nfn consensus_all_directional_outcomes_preserve_unknown')
opening = current.index('{\n', start)
depth, index = 1, opening + 1
while depth:
    index += 1
    depth += (current[index] == '{') - (current[index] == '}')
assert current[index + 1:index + 3] == '\n\n'
assert current[:start] + current[index + 3:] == old_source.read_text(), 'unaccounted original integration changes'
binary = work / 'integration-tests'
inventory = subprocess.run([str(binary), '--list'], capture_output=True, text=True, check=True).stdout
names = set(re.findall(r'^(.+): test$', inventory, re.M))
assert names == old_names | {
    'consensus_all_directional_outcomes_preserve_unknown',
    'traced_consensus_preserves_counters_errors_and_source_guards',
}
assert len(old_names) == 38 and len(names) == 40
account = runpy.run_path(str(ROOT / 'scripts/audit-dedup-complete-collection-terminal-20261015.py'))['account']
counts = account((work / 'integration-run.log').read_text(), names)
assert bool(data['results'][-1]['returncode']) == bool(counts['failed'])
output = D / 'dedup-additive-trace-runtime-audit-20261015.json'
assert not output.exists()
output.write_text(json.dumps(dict(
    status='verified_failed_complete_candidate_portfolio' if counts['failed'] else 'verified_passing_complete_candidate_portfolio',
    gate_passed=not counts['failed'], **counts,
    producer_report_sha256=sha(report), auditor_sha256=sha(__file__),
    inventory_parser_sha256=sha(ROOT / 'scripts/audit-dedup-complete-collection-terminal-20261015.py'),
    original_terminal_audit_sha256=sha(original_audit_path),
    library_sha256=sha(work / 'librrrah_dedup.rlib'), binary_sha256=sha(binary),
    metadata_plan_sha256=sha(work / 'plan.json'),
    scope=data['scope'] + ' Complete40-test target verified against prior38-test executable inventory and two explicit additions. Not whole-library/all-target Cargo, current native corpus or performance proof.',
), indent=2) + '\n')
print(output, counts['passed'], counts['failed'], counts['ignored'])
