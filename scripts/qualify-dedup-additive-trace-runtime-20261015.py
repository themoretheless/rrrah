"""Build the complete captured candidate library and complete portfolio target."""
from pathlib import Path
import hashlib
import json
import os
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WORK = Path('/private/tmp/rrrah-additive-trace-full-production-metadata-20261015')
plan = json.loads((WORK / 'plan.json').read_text())
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
report = ROOT / 'docs/research/dedup-additive-trace-runtime-20261015.json'
assert not report.exists()
source = ROOT / 'crates/rrrah-dedup/tests/search_portfolio.rs'
original = source.read_text()
start = original.index('#[test]\nfn consensus_unknown_runs_reverse_and_provider_failure_stays_error')
opening = original.index('{\n', start)
depth, index = 1, opening + 1
while depth:
    index += 1
    depth += (original[index] == '{') - (original[index] == '}')
extra = original[start:index + 1]
extra = extra.replace('fn consensus_unknown_runs_reverse_and_provider_failure_stays_error', 'fn traced_consensus_preserves_counters_errors_and_source_guards', 1)
extra = extra.replace('let result = search_native_bank_consensus_files_with(', 'let result = rrrah_dedup::search_portfolio::search_native_bank_consensus_files_with_trace(', 1)
needle = '                let e = result?;'
assert extra.count(needle) == 1
extra = extra.replace(needle, '''                let trace = result?;
                assert_eq!(trace.forward.model_attempts, Some(25));
                assert_eq!(trace.reverse.model_attempts, Some(25));
                assert!(trace.forward.model_diagnostics.is_some());
                assert_eq!(trace.reverse.model_diagnostics,
                    Some(rrrah_dedup::model_banks::ModelBankDiagnostics::default()));
                let e = trace.summary();''', 1)
integration = WORK / 'search_portfolio-integration.rs'
integration.write_text(original + '\n' + extra + '\n')
library = WORK / 'librrrah_dedup.rlib'
command = list(plan['command'])
command[command.index('--emit=metadata')] = '--emit=link'
command[command.index('-o') + 1] = str(library)
test_command = json.loads((ROOT / 'docs/research/dedup-projected-integration-cached-library-20261015.json').read_text())['commands'][0]['command']
test_command[test_command.index('--crate-name') + 1] = 'search_portfolio_trace'
test_command[test_command.index(str(ROOT / 'crates/rrrah-dedup/tests/projected_portfolio.rs'))] = str(integration)
test_command[test_command.index('-o') + 1] = str(WORK / 'integration-tests')
for index, argument in enumerate(test_command):
    if argument.startswith('rrrah_dedup='):
        test_command[index] = 'rrrah_dedup=' + str(library)
    elif argument.startswith('rrrah_decode='):
        test_command[index] = 'rrrah_decode=/private/tmp/rrrah-consensus-collection-target-20261014/debug/deps/librrrah_decode-74c55a6b7ffff907.rlib'
env = os.environ.copy()
env['CARGO_MANIFEST_DIR'] = str(ROOT / 'crates/rrrah-dedup')
env['RRRAH_BUILT_RECIPE_ID'] = plan['recipe_environment_value']
data = dict(status='starting', producer_pid=os.getpid(), producer_sha256=sha(__file__),
            metadata_plan_sha256=sha(WORK / 'plan.json'), original_integration_sha256=sha(source),
            candidate_integration_sha256=sha(integration), commands=[command, test_command],
            scope=plan['scope'] + ' Actual candidate machine code and complete portfolio integration target, plus one traced-provider/cancellation/mutation regression. No full Cargo/all-target/native model/current Root decoder/Core/recipe parity or performance qualification.')


def save():
    report.write_text(json.dumps(data, indent=2) + '\n')


def verify_inputs():
    for path, digest in plan['source_pins'].items():
        assert sha(WORK / path) == digest, path
    for path, digest in plan['dependency_pins'].items():
        assert sha(path) == digest, path


save()
for phase, args, log in [
    ('building_candidate_library', command, 'library-build.log'),
    ('building_complete_integration', test_command, 'integration-build.log'),
    ('listing_complete_integration', [str(WORK / 'integration-tests'), '--list'], 'integration-inventory.log'),
    ('running_complete_integration', [str(WORK / 'integration-tests'), '--test-threads=2'], 'integration-run.log'),
]:
    verify_inputs()
    data['status'] = phase
    save()
    with (WORK / log).open('w') as output:
        child = subprocess.Popen(args, cwd=ROOT, env=env, stdout=output, stderr=subprocess.STDOUT)
        data['child_pid'] = child.pid
        save()
        code = child.wait()
    data.setdefault('results', []).append(dict(phase=phase, returncode=code, log=str(WORK / log), log_sha256=sha(WORK / log)))
    save()
    if code:
        data['status'] = 'terminal_failure'
        save()
        raise SystemExit(code)
    verify_inputs()
data['status'] = 'terminal_pending_inventory_audit'
data['library_sha256'] = sha(library)
data['binary_sha256'] = sha(WORK / 'integration-tests')
save()
print(report)
