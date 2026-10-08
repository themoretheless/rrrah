"""Independent directional residual counts for integrated real refusals."""
import hashlib,json
from pathlib import Path
import numpy as np
D=Path('docs/research');p=D/'dedup-main-occlusion-local-rank-api.json';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(p.read_text());assert r['status']=='complete' and len(r['results'])==5;assert all(h(k)==v for k,v in r['input_hashes'].items())
b=json.loads((D/'dedup-main-photometric-misses-radius8.json').read_text());summary=[]
for row in r['results']:
 assert row['returncode']==0 and [json.loads(x) for x in row['stdout'].splitlines()]==row['evidence'];e=next(v['evidence'] for v in b['results'] if v['query']==row['query']);assert row['domains']==e['regions'][row['region_index']]['domains'];m=np.array(e['matrix']);inverse=np.linalg.inv(m);forward=both=0;reverse_residuals=[]
 def inside(point,rect):x,y,w,height=rect;return x<=point[0]<x+w and y<=point[1]<y+height
 for source,target in e['correspondences']:
  if not all(inside(point,rect) for point,rect in zip((source,target),row['domains'])):continue
  q=m@np.array([*source,1.]);z=inverse@np.array([*target,1.]);f=float(np.linalg.norm(q[:2]/q[2]-target));rev=float(np.linalg.norm(z[:2]/z[2]-source))
  if f<=2:forward+=1;reverse_residuals.append(rev);both+=int(rev<=2)
 assert both<10 and all(x['status']=='refusal:InsufficientGeometry' for x in row['evidence'])
 summary.append({'query':row['query'],'region_index':row['region_index'],'forward_tolerance2_witnesses':forward,'both_tolerance2_witnesses':both,'source_residuals_of_forward_witnesses':sorted(reverse_residuals)})
result={'status':'verified_integrated_local_rank_real_geometry_refusals','summary':summary,'pins':{str(x):h(x) for x in [p,Path(__file__)]},'scope':'Independent inverse/residual regional counts prove all15 explicit min10/tolerance2 refusals. Pixel stage not reached. Source/target resolution difference requires explicit unit-aware policy evaluation; no threshold promotion.'};out=D/'dedup-main-occlusion-local-rank-api-audit.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n');print([(x['query'],x['region_index'],x['forward_tolerance2_witnesses'],x['both_tolerance2_witnesses']) for x in summary])
