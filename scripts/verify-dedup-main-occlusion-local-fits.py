"""Independent residual, image-domain and count bounds for local-fit diagnostic."""
import hashlib,json,math
from pathlib import Path
D=Path('docs/research');p=D/'dedup-main-occlusion-local-fits.json';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(p.read_text());assert r['status']=='complete' and len(r['results'])==10
assert all(h(k)==v for k,v in r['input_hashes'].items());base=json.loads((D/'dedup-main-photometric-misses-radius8.json').read_text());summary=[]
for row in r['results']:
 e=next(v['evidence'] for v in base['results'] if v['query']==row['query']);assert row['domains']==e['regions'][row['region_index']]['domains'];points=[e['correspondences'][i] for i in row['original_point_indices']];assert 10<=len(points)<=42
 for pair in points:
  for point,rect in zip(pair,row['domains']):x,y,w,height=rect;assert x<=point[0]<x+w and y<=point[1]<y+height
 assert row['geometry_returncode']==0 and json.loads(row['geometry_stdout'])==row['geometry'];fit=row['geometry'];assert fit['points']==len(points)
 item={'query':row['query'],'region_index':row['region_index'],'geometry_status':fit['status']}
 if fit['status']=='model':
  m=fit['matrix'];assert len(fit['inliers'])>=10 and len(set(fit['inliers']))==len(fit['inliers']);error=0
  for i in fit['inliers']:
   source,target=points[i];x,y=source;den=m[2][0]*x+m[2][1]*y+m[2][2];mapped=[(m[k][0]*x+m[k][1]*y+m[k][2])/den for k in (0,1)];distance=math.dist(mapped,target);assert distance<=2;error+=distance**2
  assert math.isclose(error,fit['squared_error'],rel_tol=1e-7,abs_tol=1e-7)
  w,height=row['dimensions'][:2];denoms=[m[2][0]*x+m[2][1]*y+m[2][2] for x,y in [(0,0),(w-1,0),(0,height-1),(w-1,height-1)]];assert all(math.isfinite(x) and x!=0 for x in denoms) and (all(x>0 for x in denoms) or all(x<0 for x in denoms))
  assert row['rank_returncode']==0 and [json.loads(x) for x in row['rank_stdout'].splitlines()]==row['rank'];fractions={}
  for radius in (0,3,8):
   ds=[x for x in row['rank'] if x['filter_radius']==radius];assert len(ds)==2
   for x in ds:assert 0<=x['valid_sites']<=x['sites'] and 0<=x['agreeing_pairs']<=x['informative_pairs']<=8*x['valid_sites']
   fractions[radius]=min(x['agreeing_pairs']/x['informative_pairs'] if x['informative_pairs'] else 0 for x in ds)
  item.update(inliers=len(fit['inliers']),minimum_rank_fractions=fractions)
 summary.append(item)
result={'status':'verified_local_fit_diagnostic_residuals_and_counts','summary':summary,'pins':{str(x):h(x) for x in [p,Path(__file__)]},'scope':'Ten measured local attempts. Independent forward residuals and source-domain horizon check; bounded pixel counts. No independent exhaustive no-model proof, inverse-domain oracle or pixel resampling for refitted models. No threshold/default or copy-admission promotion.'};out=D/'dedup-main-occlusion-local-fits-audit.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n');print(result['status'])
