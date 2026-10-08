"""Independent regional witness counts and complete candidate order/rank predicates."""
import hashlib,json
from pathlib import Path
import numpy as np
D=Path('docs/research');p=D/'dedup-main-screen-all-local-rank.json';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(p.read_text());assert r['status']=='complete' and len(r['results'])==r['required_regions']==78;assert all(h(k)==v for k,v in r['input_hashes'].items()) and all(h(k)==v for k,v in r['generated_hashes'].items());b=json.loads((D/'dedup-main-photometric-misses-radius8.json').read_text());e=next(v['evidence'] for v in b['results'] if v['query']=='200501.jpg');m=np.array(e['matrix']);inv=np.linalg.inv(m);summary=[];qualified=0;refusals=0
for index,row in enumerate(r['results']):
 assert row['region_index']==index and row['domains']==e['regions'][index]['domains'];ids=[]
 for i,pair in enumerate(e['correspondences']):
  contained=all(rect[0]<=point[0]<rect[0]+rect[2] and rect[1]<=point[1]<rect[1]+rect[3] for point,rect in zip(pair,row['domains']))
  if not contained:continue
  a=m@np.array([*pair[0],1.]);b=inv@np.array([*pair[1],1.])
  if np.linalg.norm(a[:2]/a[2]-pair[1])<=2 and np.linalg.norm(b[:2]/b[2]-pair[0])<=r['source_tolerance']:ids.append(i)
 assert ids==row['witness_indices']
 if len(ids)<10:assert 'returncode' not in row;continue
 qualified+=1
 if row['returncode']:refusals+=1;continue
 assert row['rank']==[json.loads(x) for x in row['stdout'].splitlines()];assert [(x['direction'],x['filter_radius']) for x in row['rank']]==[(d,f) for d in ['forward','reverse'] for f in [0,3,8]]
 for radius in [0,3,8]:
  ds=[x for x in row['rank'] if x['filter_radius']==radius]
  for x in ds:assert 0<=x['agreeing_pairs']<=x['informative_pairs']<=8*x['valid_sites'] and 0<=x['valid_sites']<=x['sites']
  support=all(x['informative_pairs']>=1000 and x['valid_sites']/x['sites']>=.3 and x['agreeing_pairs']/x['informative_pairs']>=.9 for x in ds)
  summary.append({'region_index':index,'witnesses':len(ids),'filter_radius':radius,'minimum_agreement':min(x['agreeing_pairs']/x['informative_pairs'] if x['informative_pairs'] else 0 for x in ds),'supported_predicate':support})
result={'status':'verified_screen_all_existing_regions_witnesses_and_predicates','regions':78,'geometry_eligible_regions':qualified,'native_rank_refusals':refusals,'supported_cases':[v for v in summary if v['supported_predicate']],'summary':summary,'pins':{str(x):h(x) for x in [p,Path(__file__)]},'scope':'Independent NumPy inverse/residual counts on every proposed region; exact order/native rank bounds and unchanged support predicate. No independent pixel oracle for all these regions or combined API/file copy admission/broad precision.'};out=D/'dedup-main-screen-all-local-rank-audit.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n');print('eligible',qualified,'refusals',refusals,'passing',result['supported_cases'])
