"""Check fixed production source, baseline failure, executed tests and immutable logs."""
import pathlib,json,hashlib,re
R=pathlib.Path(__file__).resolve().parents[1];D=R/'docs/research';W=pathlib.Path('/Users/themoretheless/.codex/tmp/rrrah-cache-interpretation-fix-20261012');S=W/'source'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
b=json.loads((D/'dedup-cache-interpretation-baseline-audit-20261012.json').read_text());assert b['status']=='verified_expected_baseline_failure'
p=D/'dedup-cache-interpretation-expanded-20261012.json';d=json.loads(p.read_text());assert d['status']=='passed' and d['returncode']==0 and d['inputs_unchanged']
assert all(sha(S/'crates/rrrah-dedup'/f)==h for f,h in d['source_hashes'].items())
assert all(sha(R/'crates/rrrah-dedup'/f)==h for f,h in d['source_hashes'].items())
log=pathlib.Path(d['log']);assert sha(log)==d['log_sha256'];text=log.read_text()
counts=[int(x) for x in re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',text)];assert counts==[2,15,1],counts
for name in ['pfm_storage_order_cannot_reuse_the_opposite_interpretation','caller_interpretations_have_separate_repeatable_cache_entries','same_cache_retries_repaired_sources_and_never_retains_old_pair_reports']:assert f'test {name} ... ok' in text
m=json.loads((D/'dedup-current-full-fixtures-source-20261012.json').read_text());changed={'crates/rrrah-dedup/'+f for f in d['source_hashes']};assert all(sha(S/f)==h for f,h in m['source_hashes'].items() if f not in changed)
initial=json.loads((D/'dedup-cache-interpretation-fix-20261012.json').read_text());assert sha(pathlib.Path(d['initial_test_copy']))==initial['changed_inputs']['tests/cache_interpretation.rs']
pins={str(p):sha(p),str(log):sha(log),str(pathlib.Path(__file__)):sha(pathlib.Path(__file__))}
for binary in re.findall(r'Running tests/[^\n]+ \(([^)]+)\)',text):pins[binary]=sha(pathlib.Path(binary))
out={'status':'verified_caller_interpretation_cache_fix','passed':18,'failed':0,'ignored':0,'all_other_frozen_source_files_unchanged':True,'root_dedup_inputs_match_executed_source':True,'pins':pins,'scope':'PFM row-order semantic regression, 21 distinct caller interpretation configurations and 15 decode plus one scan lifecycle test. No universal format/platform/native memory or precision claim.'}
(D/'dedup-cache-interpretation-fix-audit-20261012.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'],out['passed'])
