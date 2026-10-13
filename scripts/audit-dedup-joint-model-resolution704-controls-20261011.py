#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-joint-model-resolution704-controls-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete_bounded_joint_model_gap_controls';assert len(r['results'])==len(r['plan'])==1
 h=D/'dedup-joint-window-roster-harness-20261011.json';m=json.loads(h.read_text());pins={**r['pins'],**m['pins'],str(p):sha(p),str(h):sha(h),str(Path(__file__)):sha(__file__)};assert all(sha(k)==v for k,v in pins.items())
 source=Path(m['production_parent']);source=D/source;f=json.loads(source.read_text());assert all(sha(Path(f['snapshot'])/k)==v for k,v in f['file_hashes'].items());pins[str(source)]=sha(source)
 row=r['results'][0];plan=r['plan'][0];assert plan['baseline_index']==255 and plan['direction']=='reverse';assert row['command'][-5:]==[plan['left'],plan['right'],row['command'][-3],'704',str(plan['source_tolerance'])];assert row['returncode']==0
 v=json.loads(row['stdout']);assert v['managed_used']==0 and len(v['window_roster'])==v['attempts'];pins[row['log']]=row['log_sha256'];assert sha(row['log'])==row['log_sha256'];assert Path(row['log']).read_text()==row['stdout']+row['stderr']
 if v['supported']:
  for kind in ['forward','reverse']:
   match=re.search(kind+r': RankRegionEvidence \{ ([^}]+) \}',v['pixels']);assert match;c={k:int(n) for k,n in re.findall(r'(sites|valid_sites|informative_pairs|agreeing_pairs): (\d+)',match[1])};assert c['informative_pairs']>=1000 and c['valid_sites']/c['sites']>=.3 and c['agreeing_pairs']/c['informative_pairs']>=.9
 out=D/'dedup-joint-model-resolution704-audit-20261011.json';assert not out.exists();result={'supported':True if v['supported'] else None,'attempts':v['attempts'],'windows_with_geometry':sum(x['has_geometry'] for x in v['window_roster']),'elapsed_seconds':row['elapsed_seconds']}
 out.write_text(json.dumps({'status':'verified_bounded_resolution704_control','pins':pins,'result':result,'scope':'Reverse200301→200300 only, original tolerance/filter/pixel floors retained; max_side704. Frozen production, no automatic resolution policy or full corpus qualification.'},indent=2)+'\n');print(json.dumps(result))
if __name__=='__main__':main()
