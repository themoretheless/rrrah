"""Audit execution and source identity without treating zero selected tests as coverage."""
import pathlib,json,hashlib,re,shutil
R=pathlib.Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-model-miss-diagnostics-20261012.json';d=json.loads(P.read_text());S=pathlib.Path(d['source']);W=S.parent
assert d['status']=='passed' and d['inputs_unchanged']
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
for f,h in d['inputs'].items():assert sha(S/'crates/rrrah-dedup'/f)==h==sha(R/'crates/rrrah-dedup'/f)
prior=json.loads((D/'dedup-cache-interpretation-expanded-20261012.json').read_text())
for f,h in prior['source_hashes'].items():assert sha(S/'crates/rrrah-dedup'/f)==h
original=json.loads((D/'dedup-current-full-fixtures-source-20261012.json').read_text());changes={'crates/rrrah-dedup/'+f for f in [*d['inputs'],*prior['source_hashes']]}
assert all(sha(S/f)==h for f,h in original['source_hashes'].items() if f not in changes)
counts=[];pins={str(P):sha(P),str(pathlib.Path(__file__)):sha(__file__)};preserved=W/'qualified-executables';assert not preserved.exists();preserved.mkdir()
for g in d['gates']:
 assert g['returncode']==0
 log=pathlib.Path(g['log']);assert sha(log)==g['log_sha256'];text=log.read_text();n=re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',text);assert len(n)==1;counts.append(int(n[0]));pins[str(log)]=sha(log)
 binary=pathlib.Path(re.search(r'Running tests/[^\n]+ \(([^)]+)\)',text)[1]);saved=preserved/g['test'];shutil.copy2(binary,saved);assert sha(saved)==sha(binary);pins[str(saved)]=sha(saved)
assert counts==[47,0,6],counts
q=D/'dedup-native-family-diagnostics-supplement-v2-20261012.json';a=json.loads(q.read_text());assert a['status']=='passed' and a['returncode']==0 and a['listing'].count(': test')==4
assert sha(a['binary'])==a['binary_sha256']==sha(preserved/'search_portfolio');assert sha(a['log'])==a['log_sha256'];pins[str(q)]=sha(q);pins[a['log']]=a['log_sha256']
text=(W/'model_compare.log').read_text();assert 'file_bank_coverage_keeps_flat_and_inverted_pixels_inconclusive ... ok' in text and 'file_bank_empty_is_inconclusive_and_cancelled_provider_discards_retained_prefix ... ok' in text
text=(W/'portfolio_collection.log').read_text();assert 'native_bank_collection_all_unknown_pairs_preserve_stage_trace ... ok' in text
out={'status':'verified_compact_model_miss_diagnostics','passed':57,'failed':0,'ignored':0,'zero_selected_stage':'Original search_portfolio native_bank filter selected0; not coverage. Supplement independently ran all4 native_family_first tests from same newly compiled binary.','counts':{'model_compare':47,'native_family_first':4,'native_bank_collection':6},'source_inputs_match_root':True,'unchanged_other_original_frozen_files':True,'pins':pins,'scope':'Generic supplied-provider and real original-pixel integration tests; no actual Torch inference, new negative classifier, universal resource bound or native458 corpus qualification.'}
(D/'dedup-model-miss-diagnostics-audit-20261012.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'],out['passed'])
