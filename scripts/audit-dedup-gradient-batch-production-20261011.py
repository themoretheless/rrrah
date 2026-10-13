"""Require final-source platform gates, real matcher parity and native file proofs."""
import hashlib,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';OUT=D/'dedup-gradient-batch-production-audit-20261011.json';assert not OUT.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
mp=D/'dedup-gradient-batch-production-snapshot-20261011.json';m=json.loads(mp.read_text());assert len(m['file_hashes'])==3057
assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
fp=D/'dedup-gradient-index-raster-audit-20261011.json';f=json.loads(fp.read_text())
assert f['status']=='complete_raster_feature_gate_macos_runtime_and_windows_compile_only'
assert f['decode_implies_raster'] and f['decode_enabled_numerical_source_unchanged']
changes=['crates/rrrah-dedup/src/lib.rs','crates/rrrah-dedup/tests/gradient_index.rs']
assert f['source_changes']==changes
for p,h in m['file_hashes'].items():
 if not p.startswith('crates/rrrah-dedup/'): continue
 if p in changes:
  old=(Path(m['snapshot'])/p).read_text();new=(ROOT/p).read_text()
  if p.endswith('lib.rs'): expected=old.replace('#[cfg(feature = "decode")]\npub mod gradient_index;', '#[cfg(feature = "raster")]\npub mod gradient_index;')
  else: expected=old.replace('#![cfg(feature = "decode")]', '#![cfg(feature = "raster")]')
  assert new==expected and new!=old,p
 else: assert sha(ROOT/p)==h,p
logs=[D/'dedup-gradient-batch-production-generic-20261011.log',D/'dedup-gradient-batch-production-linux-20261011.log']
for log in logs:
 counts=[int(n) for n in re.findall(r'test result: ok\. (\d+) passed; 0 failed;',log.read_text())]
 assert counts==[62,11,6,5,9,10,6,8],(log,counts)
opt3=D/'dedup-gradient-batch-production-opt3-tests-20261011.log';assert '3 passed; 0 failed; 1 ignored' in opt3.read_text()
rp=D/'dedup-gradient-batch-production-real-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_six_real_descriptor_controls_with_exact_parity';assert len(r['samples'])==24
assert all(sha(p)==h for p,h in r['pins'].items())
assert all(x['matches']==1142 for x in r['samples'] if x['pair']=='0:0')
np=D/'dedup-gradient-batch-production-collection-20261011.json';n=json.loads(np.read_text());assert n['status']=='complete_four_lane_native_mirror_whole_rephoto_and_exact_three_pair_collection'
assert all(sha(p)==h for p,h in n['pins'].items())
assert [x['case'] for x in n['results']]==['whole-rephoto','native-mirror-pair','three-unique-photo-collection']
assert all(x['status']=='complete' and x['evidence']['managed_used']==0 for x in n['results'])
assert n['results'][0]['exact_whole_proof_parity'] and n['results'][1]['exact_native_mirror_proof_parity'] and n['results'][2]['exact_all_pair_evidence_parity']
files=[fp,mp,*logs,opt3,rp,np,D/'dedup-gradient-batch-production-native-build-20261011.log',D/'dedup-gradient-batch-prototype-audit-20261011.json',Path(__file__)]
a={'status':'complete_bounded_production_four_lane_qualification','snapshot_files':3057,'current_source_delta':changes,'current_source_delta_scope':'Pure index feature exposure only; decode implies raster. Separately qualified macOS raster runtime and Windows compile-only.','macos_generic_tests':117,'linux_decode_generic_tests':117,'opt_level3_numeric_tests':3,'final_source_real_matcher_samples':24,'native_file_controls':3,'exact_complete_collection_pairs':3,'real_matcher_summary':r['summary'],'maximum_native_process_rss_bytes':max(x['process_peak_rss_bytes'] for x in n['results']),'pins':{str(p):sha(p) for p in files},'scope':'Four ordered independent distance accumulators, scalar partial blocks, unchanged logical pairs/work/table order and tie thresholds; per-pair checkpoints precede at most four-pair arithmetic blocks. No new heap scratch. Final actual-production tests retain old scalar oracle, exact distances/correspondences/cancellation counts. Real mirror Native4 proof, whole rephoto proof and all three tiled/native pair evidences match prior reports.','limits':'Bounded CPU/macOS native corpus; Linux generic decode only, no Linux Torch. Shared-host opt-level1 matcher timings do not establish whole-library or collection speedup. Full current corpus, indexed pair scaling, all formats and hard tensor/workspace RSS admission remain open.'}
OUT.write_text(json.dumps(a,indent=2)+'\n');print(a['status'])
