#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 rp=D/'dedup-native-row-ring-control-20261012.json';r=json.loads(rp.read_text());assert r['status']=='complete_native_row_ring_control'and r['returncode']==r['build_returncode']==0
 bp=D/'dedup-native-no-torch-20261012.json';base=json.loads(bp.read_text());ba=D/'dedup-native-no-torch-audit-20261012.json';audit=json.loads(ba.read_text());assert audit['status']=='verified_identical_native_outcome_without_linked_torch' and all(sha(k)==v for k,v in audit['pins'].items())
 for row in [r,base]:assert all(sha(k)==v for k,v in row['pins'].items())and sha(row['build_log'])==row['build_log_sha256'];assert not any('torch'in x.lower()for x in row['dynamic_libraries'].splitlines()[1:])
 assert r['command'][3:]==base['command'][3:]
 old=json.loads(base['stdout']);new=json.loads(r['stdout'])
 for key in ['files','pairs','opposite_trials','managed_retained_before_drop','managed_used']:assert new[key]==old[key],key
 assert new['managed_used']==0 and new['pairs'][0]['support']and not new['pairs'][0]['model_attempted'];assert new['managed_peak']<=old['managed_peak']
 mp=D/'dedup-native-row-ring-control-snapshot-20261012.json';m=json.loads(mp.read_text());parent=json.loads(Path(m['parent_manifest']).read_text());key='crates/rrrah-dedup/src/gradient_scale.rs';assert sha(m['parent_manifest'])==m['parent_sha256'];assert m['deltas']==[key];assert {k for k,v in m['file_hashes'].items()if parent['file_hashes'][k]!=v}=={key};assert sha(R/key)==m['file_hashes'][key]
 oldrss=int(re.search(r'(\d+)\s+maximum resident set size',base['stderr'])[1]);rss=int(re.search(r'(\d+)\s+maximum resident set size',r['stderr'])[1]);out=D/'dedup-native-row-ring-control-audit-20261012.json';assert not out.exists()
 out.write_text(json.dumps({'status':'verified_identical_native_outcome_with_row_ring_smoothing','baseline_rss_bytes':oldrss,'row_ring_rss_bytes':rss,'rss_difference_bytes':rss-oldrss,'baseline_managed_peak_bytes':old['managed_peak'],'row_ring_managed_peak_bytes':new['managed_peak'],'baseline_elapsed_seconds':base['elapsed_seconds'],'row_ring_elapsed_seconds':r['elapsed_seconds'],'pins':{str(p):sha(p)for p in [rp,bp,ba,mp,Path(__file__)]},'scope':'Same frozen positive163 source/policy/requests/weights/no-Torch feature and complete native result, with only smoothing implementation replaced. Managed credit release verified. Single concurrent-host memory/time control, not universal speedup, hard process cap, full corpus/platform/precision qualification.'},indent=2)+'\n');print('Exact native evidence verified, RSS',oldrss,'->',rss,'managed peak',old['managed_peak'],'->',new['managed_peak'])
if __name__=='__main__':main()
