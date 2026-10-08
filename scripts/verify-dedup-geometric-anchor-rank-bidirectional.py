"""Audit pre-scoring anchor selection, disjoint centers, geometry and support predicates."""
import hashlib,json,math
from pathlib import Path
D=Path('docs/research');p=D/'dedup-geometric-anchor-rank-bidirectional.json';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(p.read_text());assert r['status']=='complete_bidirectional_geometric_anchor_rank_diagnostic' and len(r['summary'])==24;assert all(h(k)==v for k,v in r['pins'].items());b=json.loads((D/'dedup-combined-domain-release-original-full.json').read_text());summary=[]
for query in ['202502.jpg','212602.jpg','204702.jpg','200501.jpg']:
 e=next(v['evidence'] for v in b['results'] if v['query']==query)
 for direction,side in [('forward',1),('reverse',0)]:
  rows=[v for v in r['summary'] if v['query']==query and v['direction']==direction];assert [v['filter_radius'] for v in rows]==[0,3,8];first=rows[0];assert len(first['anchor_indices'])>=10 and len(set(first['anchor_indices']))==len(first['anchor_indices'])
  assert set(first['anchor_indices'])<=set(first['compatible_indices'])<=set(e['inliers']);windows=[];indices=[]
  # Existing cases all coordinates admit full5x5 windows; reconstruct exact deterministic overlap exclusion.
  for index in first['compatible_indices']:
   center=e['correspondences'][index][side];x=math.floor(center[0])-2;y=math.floor(center[1])-2;assert x>=0 and y>=0
   if any(x<px+5 and px<x+5 and y<py+5 and py<y+5 for px,py,_,_ in windows):continue
   windows.append([x,y,5,5]);indices.append(index)
  assert windows==first['windows'] and indices==first['anchor_indices']
  for row in rows:
   assert row['windows']==windows and row['anchor_indices']==indices and row['sites']==25*len(windows);assert 0<=row['valid_sites']<=row['sites'] and 0<=row['agreeing_pairs']<=row['informative_pairs']<=8*row['valid_sites'];assert row['agreement']==row['agreeing_pairs']/row['informative_pairs']
   summary.append({'query':query,'direction':direction,'filter_radius':row['filter_radius'],'anchors':len(indices),'coverage':row['valid_sites']/row['sites'],'agreement':row['agreement'],'passes_unchanged_counts':row['informative_pairs']>=1000 and row['valid_sites']/row['sites']>=.3 and row['agreement']>=.9})
result={'status':'verified_anchor_selection_and_ordinal_diagnostic_predicates','summary':summary,'pins':{str(x):h(x) for x in [p,Path(__file__)]},'scope':'Deterministic coordinate-only disjoint5x5 centers before scoring; same selection across filter levels, bounded aggregated counts. Prior independent Python pixel oracle is reused, not actual native anchor API, held-out geometry or unrelated precision.'};out=D/'dedup-geometric-anchor-rank-bidirectional-audit.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n');print(result['status'])
