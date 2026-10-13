#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-anchor-scale-trace-audit-20261011.json';assert not out.exists();pins={str(Path(__file__)):sha(__file__)};decisions=[];base=json.loads((D/'dedup-model-joint-refit-20261011.json').read_text());original=json.loads(base['results'][1]['stdout'])
 for kind,name,status in [('positive','trace','complete_bounded_anchor_scale_trace'),('unrelated','negative','complete_bounded_anchor_scale_unrelated_control')]:
  rp=D/f'dedup-anchor-scale-{name}-20261011.json';r=json.loads(rp.read_text());assert r['status']==status and len(r['results'])==8;assert [x['filter_radius'] for x in r['results']]==[0,2,4,8,12,16,24,32];assert all(sha(p)==h for p,h in r['pins'].items());pins.update(r['pins']);pins[str(rp)]=sha(rp)
  for row in r['results']:
   assert row['returncode']==0;assert sha(row['log'])==row['log_sha256'];assert Path(row['log']).read_text()==row['stdout']+row['stderr'];pins[row['log']]=row['log_sha256'];v=json.loads(row['stdout']);assert v['filter_radius']==row['filter_radius'];assert v['best']==original['best'] and v['managed_used']==0;assert v['source_tolerance']==original['source_tolerance'] and v['target_tolerance']==2;counts=[]
   for line in row['stderr'].splitlines():
    assert line.startswith('RRRAH_ANCHOR_WINDOW\t');c=dict((k,int(n)) for k,n in re.findall(r'(sites|valid_sites|informative_pairs|agreeing_pairs|pixel_reads): (\d+)',line));assert len(c)==5;counts.append(c)
   e=v['anchor_result'];assert len(counts)==sum(e['anchors']);n=e['anchors'][0];assert e['anchors']==[11,11]
   for windows,agg in zip([counts[:n],counts[n:]],e['directions']):
    assert all(sum(c[k] for c in windows)==value for k,value in agg.items())
   expected=all(x['informative_pairs']>=1000 and x['valid_sites']/x['sites']>=.3 and x['agreeing_pairs']/x['informative_pairs']>=.9 for x in e['directions']);assert expected==e['supported']
   if kind=='positive':assert expected==(row['filter_radius'] in [12,16])
   else:assert not expected
   decisions.append({'kind':kind,'filter_radius':row['filter_radius'],'supported':expected,'directions':e['directions']})
 assert all(sha(p)==h for p,h in pins.items());out.write_text(json.dumps({'status':'verified_bounded_scale_positive_and_unrelated_control','pins':pins,'results':decisions,'scope':'Eight fixed radii, unchanged geometry/point selection and90 percent agreement. All per-window counts included. Positive supports12/16; unrelated target refuses all. Single unrelated control is not precision calibration or automatic fallback qualification; no production adoption.'},indent=2)+'\n');print('Verified16 fixed-radius controls')
if __name__=='__main__':main()
