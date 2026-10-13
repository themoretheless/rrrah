#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-joint-model-bank-broader-controls-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete_bounded_joint_model_gap_controls';assert [x['baseline_index'] for x in r['plan']]==[7,67];assert len(r['results'])==2;pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());results=[]
 for plan,row in zip(r['plan'],r['results']):
  assert row['returncode']==0 and row['group_id']==plan['group_id'];assert row['command'][-5:]==[plan['left'],plan['right'],row['command'][-3],'640',str(plan['source_tolerance'])];assert sha(row['log'])==row['log_sha256'];assert Path(row['log']).read_text()==row['stdout']+row['stderr'];pins[row['log']]=row['log_sha256'];v=json.loads(row['stdout']);assert v['attempts']==25 and v['managed_used']==0
  counts=[tuple(map(int,x)) for x in re.findall(r'sites: (\d+), valid_sites: (\d+), informative_pairs: (\d+), agreeing_pairs: (\d+)',v['pixels'])];assert len(counts)==2;passes=all(info>=1000 and valid/sites>=.3 and agree/info>=.9 for sites,valid,info,agree in counts);assert v['supported']==passes
  results.append({'baseline_index':plan['baseline_index'],'group_id':row['group_id'],'supported':True if passes else None,'selected_points':v['selected_points'],'joint_witnesses':v['geometry_inliers'],'hypothesis_bank':v['hypothesis_bank'],'pixel_counts':counts,'elapsed_seconds':row['elapsed_seconds']})
 assert results[0]['supported'] and results[1]['supported'] is None
 pins.update({str(x):sha(x) for x in [p,Path(__file__)]});out=D/'dedup-model-bank-broader-controls-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_two_real_cpu_bank_controls','pins':pins,'results':results,'scope':'Frozen single-largest-candidate CPU path at filter12/window2,1m.200302 retains support; reverse203500 still lacks1000 informative pairs. No semantic negative or qualification of later pixel-qualified-candidate implementation.'},indent=2)+'\n');print(json.dumps(results))
if __name__=='__main__':main()
