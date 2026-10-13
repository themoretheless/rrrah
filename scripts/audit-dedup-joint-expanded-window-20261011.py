#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-joint-expanded-window-fixed-20261011.json';r=json.loads(p.read_text());assert r['status']=='running' and len(r['results'])==2 and all('returncode' in x for x in r['results'])
 pins=dict(r['pins']);pins[str(p)]=sha(p);pins[str(Path(__file__))]=sha(__file__);assert all(sha(k)==v for k,v in pins.items())
 old=json.loads((D/'dedup-joint-model-window-roster-controls-20261011.json').read_text());old=json.loads(old['results'][1]['stdout'])['window_roster'];results=[]
 assert [x['window_radius'] for x in r['results']]==[6,12]
 for row in r['results']:
  if row['window_radius']==12:
   assert row['returncode']==1 and not row['stdout']
   assert row['stderr'].startswith('Error: InsufficientAnchorGeometry(ModelTileEvidence')
   assert row['stderr'].count('ModelTileAttempt {')==25 and 'len: 25, model_digest:' in row['stderr']
   pins[row['log']]=row['log_sha256'];assert sha(row['log'])==row['log_sha256']
   results.append({'window_radius':12,'supported':None,'attempts':25,'status':'typed_insufficient_anchor_geometry'})
   continue
  assert row['returncode']==0;v=json.loads(row['stdout']);assert v['managed_used']==0;assert len(v['window_roster'])==v['attempts'];pins[row['log']]=row['log_sha256'];assert sha(row['log'])==row['log_sha256']
  for i,w in enumerate(v['window_roster']):
   assert w['left_rectangle']==old[i]['left_rectangle'] and w['right_rectangle']==old[i]['right_rectangle']
   # Geometry is independent of center-window coverage and must match exactly.
   geometry=lambda s:s.split(', pixels:')[0]
   assert geometry(w['outcome'])==geometry(old[i]['outcome'])
  if v['supported']:
   assert v['window_roster'][-1]['supported'];s=v['window_roster'][-1]['outcome']
   counts=[tuple(map(int,x)) for x in re.findall(r'sites: (\d+), valid_sites: (\d+), informative_pairs: (\d+), agreeing_pairs: (\d+)',s)];assert len(counts)==2
   assert all(info>=1000 and valid/sites>=.3 and agree/info>=.9 for sites,valid,info,agree in counts)
  else:counts=[]
  results.append({'window_radius':row['window_radius'],'supported':v['supported'],'attempts':v['attempts'],'pixel_counts':counts})
 out=D/'dedup-joint-expanded-window-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_bounded_expanded_window_controls','pins':pins,'results':results,'scope':'Same frozen source/geometry, strict1000-pair/.9/.3 floors unchanged. One positive direction, no automatic window policy/negative precision/full corpus qualification.'},indent=2)+'\n');print(json.dumps(results))
if __name__=='__main__':main()
