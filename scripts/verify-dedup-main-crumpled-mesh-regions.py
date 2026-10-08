"""Complete region selection and explicit nonrigid support predicates."""
import hashlib,json
from pathlib import Path
D=Path('docs/research');p=D/'dedup-main-crumpled-mesh-regions.json';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(p.read_text());assert r['status']=='complete' and len(r['results'])==r['required_regions']==125;assert all(h(k)==v for k,v in r['input_hashes'].items()) and all(h(k)==v for k,v in r['generated_hashes'].items());b=json.loads((D/'dedup-combined-domain-release-original-full.json').read_text());e=next(v['evidence'] for v in b['results'] if v['query']=='204702.jpg');assert r['original_mesh_inlier_indices']==e['inliers'] and len(e['inliers'])==52;summary=[];eligible=refusals=0
for index,row in enumerate(r['results']):
 assert row['region_index']==index and row['domains']==e['regions'][index]['domains'];ids=[]
 for i in e['inliers']:
  if all(rect[0]<=point[0]<rect[0]+rect[2] and rect[1]<=point[1]<rect[1]+rect[3] for point,rect in zip(e['correspondences'][i],row['domains'])):ids.append(i)
 assert ids==row['mesh_witness_indices']
 if len(ids)<10:assert 'returncode' not in row;continue
 eligible+=1
 if row['returncode']:refusals+=1;continue
 assert row['rank']==[json.loads(x) for x in row['stdout'].splitlines()];assert [(x['direction'],x['filter_radius']) for x in row['rank']]==[(d,f) for d in ['forward','reverse'] for f in [0,3,8]]
 for radius in [0,3,8]:
  ds=[x for x in row['rank'] if x['filter_radius']==radius]
  for x in ds:assert 0<=x['valid_sites']<=x['sites'] and 0<=x['agreeing_pairs']<=x['informative_pairs']<=8*x['valid_sites']
  supported=all(x['informative_pairs']>=1000 and x['valid_sites']/x['sites']>=.3 and x['agreeing_pairs']/x['informative_pairs']>=.9 for x in ds)
  summary.append({'region_index':index,'filter_radius':radius,'witnesses':len(ids),'minimum_agreement':min(x['agreeing_pairs']/x['informative_pairs'] for x in ds),'coverage':[x['valid_sites']/x['sites'] for x in ds],'supported':supported})
result={'status':'verified_complete_crumpled_piecewise_local_region_diagnostic','regions':125,'geometry_eligible_regions':eligible,'native_refusals':refusals,'supported_cases':[x for x in summary if x['supported']],'summary':summary,'pins':{str(x):h(x) for x in [p,Path(__file__)]},'scope':'Full52-point prior inlier mesh, every original proposed region included/excluded by both-rectangle witnesses, actual native piecewise counts and unchanged support predicates. Independent real mesh coordinate/pixel oracle, held-out precision and automatic admission pending.'};out=D/'dedup-main-crumpled-mesh-regions-audit.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n');print(result['status'],result['supported_cases'])
