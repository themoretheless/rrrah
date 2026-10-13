#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 rp=D/'dedup-joint-model-variant-controls-20261011.json';raw=rp.read_bytes();r=json.loads(raw);closed=[]
 for row in r['results']:
  if row['status']=='running':break
  closed.append(row)
 assert closed;A=Path(closed[0]['log']).parent;capture=A/f'prefix-{len(closed):04d}.json';out=D/f'dedup-joint-model-variant-controls-prefix-{len(closed):04d}-audit-20261011.json';assert not capture.exists() and not out.exists();capture.write_bytes(raw);assert all(sha(p)==h for p,h in r['pins'].items());pins=dict(r['pins']);pins.update({str(capture):sha(capture),str(Path(__file__)):sha(__file__)});gp=D/'dedup-joint-model-tiles-unknown-audit-20261011.json';g=json.loads(gp.read_text());assert g['status']=='complete_bounded_frozen_joint_model_tests' and g['tests']==36;assert all(sha(p)==h for p,h in g['pins'].items());pins[str(gp)]=sha(gp)
 gm=json.loads(Path(g['source_manifest']).read_text());mp=D/'dedup-joint-model-tiles-native-fixed-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());assert all(gm['file_hashes'][f]==m['file_hashes'][f] for f in ['crates/rrrah-dedup/src/model_compare.rs','crates/rrrah-dedup/src/model_tiles.rs']);pins[str(mp)]=sha(mp);decisions=[]
 for plan,row in zip(r['plan'],closed):
  assert row['group_id']==plan['group_id'];assert row['command'][-5:]==[plan['left'],plan['right'],row['command'][-3],'640',str(plan['source_tolerance'])];assert sha(row['log'])==row['log_sha256'];assert Path(row['log']).read_text()==row['stdout']+row['stderr'];pins[row['log']]=row['log_sha256'];support=None
  if row['returncode']==0:
   value=json.loads(row['stdout']);assert value['managed_used']==0 and 1<=value['attempts']<=25
   if value['supported']:
    support=True;assert value['geometry_inliers']>=10
    for kind in ['forward','reverse']:
     match=re.search(kind+r': RankRegionEvidence \{ ([^}]+) \}',value['pixels']);assert match;c={k:int(n) for k,n in re.findall(r'(sites|valid_sites|informative_pairs|agreeing_pairs): (\d+)',match[1])};assert c['informative_pairs']>=1000 and c['valid_sites']/c['sites']>=.3 and c['agreeing_pairs']/c['informative_pairs']>=.9
  else:
   assert row['returncode']==1 and row['status']=='typed_inconclusive';assert row['stderr'].startswith('Error: InsufficientAnchorGeometry(ModelTileEvidence');assert 'len: 25, model_digest:' in row['stderr'];assert row['stderr'].count('ModelTileAttempt {')==25
  rss=re.search(r'(\d+)\s+maximum resident set size',row['stderr']);decisions.append({'group_id':row['group_id'],'baseline_index':plan['baseline_index'],'direction':plan['direction'],'supported':support,'attempts':row['attempts'],'elapsed_seconds':row['elapsed_seconds'],'max_rss_bytes':int(rss[1])})
 assert all(sha(p)==h for p,h in pins.items());out.write_text(json.dumps({'status':'verified_bounded_real_cpu_gap_control_prefix','pins':pins,'results':decisions,'closed':len(closed),'required':2,'scope':'Actual CPU joint fixed-filter12 tile search on publisher positives.36 generic tests share source; positive original-pixel floors checked. Typed absence of support remains inconclusive, not negative. No universal precision/scale/portfolio/corpus completion.'},indent=2)+'\n');print('Verified',len(closed),'gap controls')
if __name__=='__main__':main()
