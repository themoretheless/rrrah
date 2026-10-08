"""Inspect regional geometric witnesses for all passing rank-only cases."""
import hashlib,json,math
from pathlib import Path
D=Path('docs/research');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
paths=[D/'dedup-main-occlusion-all-rank-regions.json',D/'dedup-main-photometric-misses-radius8.json',D/'dedup-main-occlusion-best-rank-pixel-audit.json'];r,b,pixels=[json.loads(p.read_text()) for p in paths];assert r['status']=='complete' and pixels['comparisons']==12
summary=[]
def inside(point,rect):
 x,y,w,height=rect;return x<=point[0]<x+w and y<=point[1]<y+height
for row in r['results']:
 if row['returncode']:continue
 e=next(x['evidence'] for x in b['results'] if x['query']==row['query'])
 for radius in (0,3,8):
  ds=[x for x in row['rank'] if x['filter_radius']==radius]
  if not all(x['informative_pairs']>=1000 and x['valid_sites']/x['sites']>=.3 and x['agreeing_pairs']/x['informative_pairs']>=.9 for x in ds):continue
  indices=[i for i in e['inliers'] if all(inside(point,rect) for point,rect in zip(e['correspondences'][i],row['domains']))]
  residuals=[];matrix=e['matrix']
  for i in indices:
   source,target=e['correspondences'][i];x,y=source;den=matrix[2][0]*x+matrix[2][1]*y+matrix[2][2];assert math.isfinite(den) and den!=0
   mapped=[(matrix[k][0]*x+matrix[k][1]*y+matrix[k][2])/den for k in (0,1)];distance=math.dist(mapped,target);assert distance<=2;residuals.append(distance)
  spans=[]
  for side,rect in enumerate(row['domains']):
   points=[e['correspondences'][i][side] for i in indices]
   spans.append([(max(p[k] for p in points)-min(p[k] for p in points))/rect[k+2] for k in (0,1)] if points else [0,0])
  summary.append({'query':row['query'],'region_index':row['region_index'],'filter_radius':radius,'regional_inlier_indices':indices,'regional_inliers':len(indices),'maximum_forward_residual':max(residuals,default=None),'axis_spans_relative_to_region':spans,'at_least_ten_regional_inliers':len(indices)>=10})
assert len(summary)==4
out=D/'dedup-main-occlusion-regional-inlier-audit.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_regional_geometric_witnesses_for_rank_only_candidates','summary':summary,'pins':{str(p):h(p) for p in paths+[Path(__file__)]},'scope':'Global-model forward residual<=2 for original spatially distinct inliers contained in both region rectangles. Three candidates retain>=10 regional witnesses. No new local fit, independent selected holdout or negative precision/copy admission.'},indent=2)+'\n');print(json.dumps(summary))
