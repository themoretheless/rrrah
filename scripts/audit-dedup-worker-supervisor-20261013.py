"""Audit actual subprocess supervision and native deadline execution with explicit limits."""
import pathlib,json,hashlib,re,shutil
R=pathlib.Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-worker-supervisor-20261013.json';d=json.loads(P.read_text());S=pathlib.Path(d['source']);W=S.parent
assert d['status']=='passed' and d['returncode']==0 and d['inputs_unchanged']
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
assert all(sha(S/'crates/rrrah-dedup'/f)==h==sha(R/'crates/rrrah-dedup'/f) for f,h in d['inputs'].items())
m=json.loads((D/'dedup-real-model-miss-preparation-20261013.json').read_text());assert all(sha(S/p)==h for p,h in m['source_hashes'].items() if p!='crates/rrrah-dedup/src/lib.rs')
log=pathlib.Path(d['log']);assert sha(log)==d['log_sha256'];text=log.read_text();assert 'test result: ok. 7 passed; 0 failed; 0 ignored;' in text
names=['binary_input_and_output_survive_private_stdio_files','invalid_policy_input_and_initial_cancellation_never_spawn','deadline_terminates_and_reaps_a_blocking_worker','live_cancellation_terminates_and_reaps_then_a_retry_succeeds','stdout_and_stderr_floods_stop_without_pipe_deadlock','nonzero_exit_preserves_bounded_binary_diagnostics']
for name in names:assert f'test {name} ... ok' in text
binary=pathlib.Path(re.search(r'Running tests/[^\n]+ \(([^)]+)\)',text)[1]);saved=W/'worker-supervisor-test';assert not saved.exists();shutil.copy2(binary,saved);assert sha(saved)==sha(binary)
Q=D/'dedup-native-worker-deadline-v2-20261013.json';n=json.loads(Q.read_text());assert n['status']=='passed' and n['returncode']==0;assert all(sha(p)==h for p,h in n['pins'].items());assert 'native_worker_deadline=terminated_and_reaped' in pathlib.Path(n['log']).read_text()
probe=pathlib.Path('/Users/themoretheless/.codex/tmp/rrrah-cache-interpretation-fix-20261012/target/debug/dedup-native-worker-deadline');copy=W/'native-worker-deadline-probe';assert not copy.exists();shutil.copy2(probe,copy)
pins={str(p):sha(p) for p in [P,log,saved,Q,copy,pathlib.Path(__file__)]}
out={'status':'verified_single_process_worker_supervision','passed_test_functions':7,'behavioral_tests':6,'helper_tests':1,'failed':0,'ignored':0,'real_native_deadline_control':'Observed200ms deadline killed and waited for actual native comparison executable; no claim that inference kernel had begun.','pins':pins,'scope':'Actual macOS child-process IO/exit/deadline/cancellation/capture tests. No RSS enforcement, hard real-time/disk cap, descendant containment, automatic model provider integration or other-platform qualification. Existing in-process LoFTR entrypoints remain cooperative.'};(D/'dedup-worker-supervisor-audit-20261013.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'])
