"""Record rejection of exact descriptor bounds after mixed real-photo timings."""
import hashlib,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';OUT=D/'dedup-gradient-bounds-audit-20261011.json';assert not OUT.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
rp=D/'dedup-gradient-bounds-real-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_six_real_descriptor_controls_with_exact_parity';assert all(sha(p)==h for p,h in r['pins'].items());assert len(r['samples'])==24
assert all(s['matches']==1142 for s in r['samples'] if s['pair']=='0:0')
assert any(s['full_to_bounded_ratio']<0.8 for s in r['summary'])
manifest=D/'dedup-gradient-bounds-real-snapshot-20261011.json';m=json.loads(manifest.read_text());assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
current=ROOT/'crates/rrrah-dedup/src/gradient.rs';base=Path('/Users/themoretheless/.codex/tmp/rrrah-prepared-reuse-snapshot-20261011/crates/rrrah-dedup/src/gradient.rs')
def matcher(text):
 start=text.index('fn match_gradients_selected(');return text[start:text.index('\n/// Describe a rotated',start)]
assert matcher(current.read_text())==matcher(base.read_text())
tests=D/'dedup-gradient-bounds-rejected-tests-20261011.log';assert '3 passed; 0 failed; 1 ignored' in tests.read_text()
final_lib=D/'dedup-gradient-bounds-final-lib-20261011.log';assert '59 passed; 0 failed' in final_lib.read_text()
log=D/'dedup-gradient-bounds-generic-20261011.log';counts=[int(n) for n in re.findall(r'test result: ok\. (\d+) passed; 0 failed;',log.read_text())];assert counts==[59,11,6,5,9,10,6,8]
files=[rp,manifest,current,ROOT/'crates/rrrah-dedup/src/gradient_matching_tests.rs',tests,final_lib,log,D/'dedup-gradient-bounds-timing-20261011.log',Path(__file__)]
a={'status':'complete_experiment_rejected_production_matcher_restored','decision':'No adoption: exact API parity but mixed timing evidence, including substantial regression on real crumpled and foreign photo descriptors. No general speedup established. Candidate and baseline retained only under cfg(test).','snapshot_files':3056,'frozen_candidate_macos_generic_tests':114,'real_same_process_alternating_samples':24,'real_descriptor_configurations':6,'self_positive_matches':1142,'real_summary':r['summary'],'production_matcher_identical_to_prepared_reuse_revision':True,'test_only_relocation_focused_tests':3,'pins':{str(p):sha(p) for p in files},'limits':'Opt-level1 same-process shared-host measurements, three real photos and bounded feature extraction. Not full Native6 collection throughput, current corpus recall, hard RSS, or release-profile performance qualification. Linux candidate gate tracked separately.'}
OUT.write_text(json.dumps(a,indent=2)+'\n');print(a['status'])
