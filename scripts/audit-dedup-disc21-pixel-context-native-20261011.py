#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-disc21-pixel-context-native-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete' and r['returncode']==r['build_returncode']==0
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());assert sha(r['build_log'])==r['build_log_sha256']
 original=D/'dedup-disc21-bank-diagnostic-native-20261011.json';runtime=json.loads(original.read_text());assert all(sha(k)==v for k,v in runtime['pins'].items());pins.update(runtime['pins']);pins[str(original)]=sha(original)
 parent=D/'dedup-model-bank-pixel-contexts-snapshot-20261011.json';m=json.loads(parent.read_text());s=Path(next(k for k in pins if k.endswith('/crates/rrrah-dedup/src/model_banks.rs'))).parents[3]
 changed=[f for f,v in m['file_hashes'].items() if sha(s/f)!=v];assert changed==['crates/rrrah-dedup/src/model_loftr.rs']
 cpu_parent=D/'dedup-native-bank-collection-native-snapshot-20261011.json';cm=json.loads(cpu_parent.read_text());assert all(sha(Path(cm['snapshot'])/k)==v for k,v in cm['file_hashes'].items());old=(Path(cm['snapshot'])/changed[0]).read_text();new=(s/changed[0]).read_text();start=new.index('/// Cached CPU LoFTR with explicit original-pixel window/rank contexts.');end=new.index('/// Native/learned portfolio with cached CPU LoFTR bank aggregation and coverage.',start);assert new[:start]+new[end:]==old
 pins[str(cpu_parent)]=sha(cpu_parent)
 verify=D/'dedup-model-bank-pixel-contexts-verification-20261011.json';a=json.loads(verify.read_text());assert a['status']=='verified_150_tests_model_bank_pixel_contexts' and all(sha(k)==v for k,v in a['pins'].items())
 baseline=D/'dedup-disc21-rank2-candidate-20261011.json';b=json.loads(baseline.read_text());assert r['command'][5:]==b['command'][5:];v=json.loads(r['stdout']);e=json.loads(b['stdout']);assert v['supported'] is True and v['attempts']==25 and v['managed_used']==0 and v['pixel_rank_radius']==2 and v['pixel_window_radius']==2
 for key in ['raw_proposals','selected_points','geometry_inliers','hypothesis_bank','transform','pixels','original_dimensions']:assert v[key]==e[key],key
 assert v['pixel_attempts']>e['pixel_attempts']
 probe=Path(next(k for k in pins if k.endswith('/src/bin/disc21_pixel_context_native_files.rs'))).read_text();assert probe.count('ModelBankPixelRecipe{')==4 and 'work.point_checks*=4;work.selection_checks*=4;work.rank_sites*=4;work.pixel_reads*=4' in probe
 pins.update({str(x):sha(x) for x in [p,parent,verify,baseline,Path(__file__),Path(r['build_log'])]});out=D/'dedup-disc21-pixel-context-native-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_actual_cpu_four_context_positive_recovery','pins':pins,'evidence':v,'scope':'New150-test generic context API plus cached CPU wrapper. Legacy rank8 contexts precede rank2 recovery; same geometry and pixel evidence as independent candidate setting. One development positive only; no full cohort/negative precision or all-case qualification.'},indent=2)+'\n');print(v)
if __name__=='__main__':main()
