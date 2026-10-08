"""Independent directional residual counts for integrated real refusals."""
import hashlib,json
from pathlib import Path
import numpy as np
D=Path('docs/research');p=D/'dedup-main-occlusion-local-rank-resolution.json';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(p.read_text());assert r['status']=='complete' and len(r['results'])==5;assert all(h(k)==v for k,v in r['input_hashes'].items())
b=json.loads((D/'dedup-main-photometric-misses-radius8.json').read_text());summary=[]
for row in r['results']:
 assert row['returncode']==0 and [json.loads(x) for x in row['stdout'].splitlines()]==row['evidence'];e=next(v['evidence'] for v in b['results'] if v['query']==row['query']);assert row['domains']==e['regions'][row['region_index']]['domains'];m=np.array(e['matrix']);inverse=np.linalg.inv(m);forward=both=0;reverse_residuals=[]
 pixelroot=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/rank-normalized-pixel-oracle');dims=[]
 oldrow=next(v for v in b['results'] if v['query']==row['query'])
 for name in [oldrow['original'],row['query']]:
  with (pixelroot/('occlusion-'+Path(name).stem+'.rgba32')).open('rb') as f:
   assert f.read(21)==b'RRRAH-RANK-RGBA32-V1\n';dims.append([int.from_bytes(f.read(4),'little'),int.from_bytes(f.read(4),'little')])
 source_tolerance=2*np.hypot(*dims[0])/np.hypot(*dims[1])
 def inside(point,rect):x,y,w,height=rect;return x<=point[0]<x+w and y<=point[1]<y+height
 for source,target in e['correspondences']:
  if not all(inside(point,rect) for point,rect in zip((source,target),row['domains'])):continue
  q=m@np.array([*source,1.]);z=inverse@np.array([*target,1.]);f=float(np.linalg.norm(q[:2]/q[2]-target));rev=float(np.linalg.norm(z[:2]/z[2]-source))
  if f<=2:forward+=1;reverse_residuals.append(rev);both+=int(rev<=source_tolerance)

 cached=json.loads((D/'dedup-main-occlusion-all-rank-regions.json').read_text());reference=next(v for v in cached['results'] if v['query']==row['query'] and v['region_index']==row['region_index'])
 for evidence in row['evidence']:
  if both<10:assert evidence['status']=='refusal:InsufficientGeometry';continue
  assert evidence['status']=='ok' and evidence['witnesses']==both
  expected=[v for v in reference['rank'] if v['filter_radius']==evidence['filter_radius']]
  for native,previous in zip(evidence['directions'],expected):
   for key in ('sites','valid_sites','informative_pairs','agreeing_pairs'):assert native[key]==previous[key]
  passes=all(v['valid_sites']/v['sites']>=.3 and v['agreeing_pairs']/v['informative_pairs']>=.9 and v['informative_pairs']>=1000 for v in evidence['directions']);assert evidence['supported']==passes
 summary.append({'query':row['query'],'region_index':row['region_index'],'forward_tolerance2_witnesses':forward,'source_tolerance':float(source_tolerance),'normalized_directional_witnesses':both,'supported_filters':[x['filter_radius'] for x in row['evidence'] if x.get('supported')],'source_residuals_of_forward_witnesses':sorted(reverse_residuals)})
result={'status':'verified_integrated_local_rank_resolution_policy','summary':summary,'pins':{str(x):h(x) for x in [p,Path(__file__)]},'scope':'Independent inverse/residual regional counts with predeclared diagonal ratio source tolerance; reached pixel stages exactly match previously measured cached counts. Shared normalized decoding, not broad precision/default promotion.'};out=D/'dedup-main-occlusion-local-rank-resolution-audit.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n');print(result['status'])
