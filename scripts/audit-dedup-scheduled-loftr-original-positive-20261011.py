#!/usr/bin/env python3
import hashlib,json,math,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 rp=D/'dedup-scheduled-loftr-original-positive-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_bounded_original_positive_scheduled_loftr_controls';assert all(sha(p)==h for p,h in r['pins'].items());assert [x['case'] for x in r['results']]==['forward','reverse']
 m=json.loads(Path(r['source_manifest']).read_text());assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items());rows=[]
 for row in r['results']:
  assert row['status']=='passed_publisher_positive' and row['returncode']==0 and sha(row['log'])==row['log_sha256'];assert Path(row['log']).read_text()==row['stdout']+row['stderr']
  value=json.loads(row['stdout']);assert value['files']==[0,1] and value['proposal_pairs']==[] and len(value['pairs'])==1;pair=value['pairs'][0];assert (pair['left'],pair['right'])==(0,1);e=pair['evidence'];assert e['supported'] is True and e['terminal']=='model_tiles' and e['attempts'];model=e['attempts'][-1]['evidence'];assert model['supported'] is True and model['geometry_inliers']>=10;pixels=model['pixels'];assert pixels['supported'] is True and len(pixels['anchors'])==2 and all(n>=10 for n in pixels['anchors']) and len(pixels['directions'])==2
  for direction in pixels['directions']:
   assert direction['sites']>0 and direction['informative_pairs']>=1000 and direction['valid_sites']/direction['sites']>=0.3 and direction['agreeing_pairs']/direction['informative_pairs']>=0.9
  assert value['managed_used']==0 and value['managed_peak']<=512*1024*1024;rss=re.findall(r'^\s*(\d+)\s+maximum resident set size$',row['stderr'],re.M);rows.append({'direction':row['case'],'elapsed_seconds':row['elapsed_seconds'],'managed_peak':value['managed_peak'],'observed_maximum_resident_bytes':int(rss[0]) if len(rss)==1 else None,'model_anchors':pixels['anchors'],'geometry_inliers':model['geometry_inliers'],'tile_attempts':len(e['attempts'])})
 assert math.isclose(r['results'][0]['source_tolerance']*r['results'][1]['source_tolerance'],4.,rel_tol=1e-12)
 report={'status':'complete_bounded_frozen_original_publisher_positive_audit','controls':2,'publisher_pair':'200000/200001','directions':['forward','reverse'],'empty_proposals_force_fallback':True,'results':rows,'pins':{str(p):sha(p) for p in [rp,Path(r['source_manifest']),Path(__file__)]},'limits':'One published original-resolution positive, model side640, diagnostic untagged-linear assumption. Local geometry and direct pixel-rank evidence; not whole-file identity, full corpus recall/precision, hard RSS or other platforms.'}
 (D/'dedup-scheduled-loftr-original-positive-audit-20261011.json').write_text(json.dumps(report,indent=2)+'\n');print(report['status'],report['controls'])
if __name__=='__main__':main()
