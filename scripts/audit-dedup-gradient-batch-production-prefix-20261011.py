"""Qualify final-source completed gates and two native prefix controls only."""
import hashlib,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';OUT=D/'dedup-gradient-batch-production-prefix-audit-20261011.json';assert not OUT.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
mp=D/'dedup-gradient-batch-production-snapshot-20261011.json';m=json.loads(mp.read_text());assert len(m['file_hashes'])==3057
assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
assert all(sha(ROOT/p)==h for p,h in m['file_hashes'].items() if p.startswith('crates/rrrah-dedup/'))
mac=D/'dedup-gradient-batch-production-generic-20261011.log';counts=[int(n) for n in re.findall(r'test result: ok\. (\d+) passed; 0 failed;',mac.read_text())];assert counts==[62,11,6,5,9,10,6,8]
opt=D/'dedup-gradient-batch-production-opt3-tests-20261011.log';assert '3 passed; 0 failed; 1 ignored' in opt.read_text()
rp=D/'dedup-gradient-batch-production-real-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_six_real_descriptor_controls_with_exact_parity';assert len(r['samples'])==24;assert all(sha(p)==h for p,h in r['pins'].items())
assert all(x['matches']==1142 for x in r['samples'] if x['pair']=='0:0')
np=D/'dedup-gradient-batch-production-collection-20261011.json';n=json.loads(np.read_text());assert n['status'] in ['running','complete_four_lane_native_mirror_whole_rephoto_and_exact_three_pair_collection']
assert all(sha(p)==h for p,h in n['pins'].items())
prefix=n['results'][:2];assert [x['case'] for x in prefix]==['whole-rephoto','native-mirror-pair'];assert all(x['status']=='complete' and x['evidence']['managed_used']==0 for x in prefix)
assert prefix[0]['exact_whole_proof_parity'] and prefix[1]['exact_native_mirror_proof_parity']
old=json.loads((D/'dedup-portfolio-collection-sticky-measurement-20261010.json').read_text());assert prefix[1]['evidence']['pairs'][0]['evidence']==old['results'][0]['evidence']['pairs'][0]['evidence']
files=[mp,mac,opt,rp,D/'dedup-gradient-batch-production-native-build-20261011.log',D/'dedup-gradient-batch-prototype-audit-20261011.json',Path(__file__)]
a={'status':'complete_final_source_unit_real_and_two_native_prefix_controls','snapshot_files':3057,'macos_generic_tests':117,'optimized_numeric_tests':3,'final_source_real_matcher_samples':24,'final_source_real_configurations':6,'real_summary':r['summary'],'completed_native_prefix':prefix,'native_runtime_and_input_pins':n['pins'],'pins':{str(p):sha(p) for p in files},'in_progress_native_report':str(np),'scope':'Final production four independent ordered accumulators with scalar partial blocks. Real matcher parity and actual Native4 mirror proof plus whole rephoto model proof are qualified. Raw completed native rows are copied; the mutable full collection report is deliberately not pinned as a completed report. Linux final-source gate and full three-photo tiled/native collection are tracked separately. No whole-library speedup, full current corpus, indexed scaling or hard RSS claim.'}
OUT.write_text(json.dumps(a,indent=2)+'\n');print(a['status'])
