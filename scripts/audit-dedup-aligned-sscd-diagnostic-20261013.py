#!/usr/bin/env python3
import json,hashlib,math
from pathlib import Path
from PIL import Image
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();P=D/'dedup-aligned-sscd-diagnostic-plan-20261013.json';O=D/'dedup-aligned-sscd-diagnostic-20261013.json';p=json.loads(P.read_text());d=json.loads(O.read_text());assert d['status']=='complete_fixed_aligned_diagnostic' and d['pins_unchanged'];assert sha(P)==d['plan_sha256'] and sha(R/'scripts/run-dedup-aligned-sscd-diagnostic-20261013.py')==p['producer_sha256'];assert all(sha(k)==v for k,v in p['pins'].items());a=json.loads(Path(p['acquisition']).read_text());assert sha(a['weights'])==a['weights_sha256'] and all(sha(Path(a['source'])/k)==v for k,v in a['source_sha256'].items());assert len(d['rows'])==2*len(p['cases'])==28

def inverse(M):
 a=[list(map(float,r))+[float(i==j) for j in range(3)] for i,r in enumerate(M)]
 for col in range(3):
  best=max(range(col,3),key=lambda i:abs(a[i][col]));a[col],a[best]=a[best],a[col];v=a[col][col];assert abs(v)>1e-15;a[col]=[x/v for x in a[col]]
  for j in range(3):
   if j!=col:
    v=a[j][col];a[j]=[x-v*y for x,y in zip(a[j],a[col])]
 return [r[3:] for r in a]
summary=[]
for case in p['cases']:
 dims=[]
 for k in ['left','right']:
  with Image.open(case[k]) as im:assert im.getexif().get(274,1)==1;dims.append(list(im.size))
 for j,direction in enumerate(['forward','reverse']):
  row=d['rows'][2*p['cases'].index(case)+j];assert row['label']==case['label'] and row['direction']==direction;A=dims[j];B=dims[1-j];assert row['source_dimensions']==A and row['target_dimensions']==B and row['region']==case['regions'][j];M=case['matrix'] if j==0 else inverse(case['matrix']);x,y,w,h=row['region'];common=0
  for v in range(320):
   sy=y+(v+.5)*h/320-.5
   for u in range(320):
    sx=x+(u+.5)*w/320-.5;den=math.fsum([M[2][0]*sx,M[2][1]*sy,M[2][2]]);assert abs(den)>1e-12;tx=math.fsum([M[0][0]*sx,M[0][1]*sy,M[0][2]])/den;ty=math.fsum([M[1][0]*sx,M[1][1]*sy,M[1][2]])/den
    if 0<=sx<A[0]-1 and 0<=sy<A[1]-1 and all(0<=tx+dx<B[0]-1 and 0<=ty+dy<B[1]-1 for dx,dy in [[0,0],*p['offsets']]):common+=1
  assert common==row['common_sites'] and row['total_sites']==102400 and row['common_fraction']==common/102400
  if common==0:assert row['status']=='no_common_support';continue
  assert row['status']=='scored' and row['offsets']==[[0,0],*p['offsets']];vectors=row['embeddings'];assert len(vectors)==len(row['pixel_tensor_sha256'])==7
  for vec in vectors:assert len(vec)==512 and all(math.isfinite(z) for z in vec) and abs(math.sqrt(math.fsum(z*z for z in vec))-1)<1e-5
  dot=lambda v:math.fsum(x*y for x,y in zip(vectors[0],v));scores=[dot(v) for v in vectors[1:6]];assert scores[0]==row['baseline_cosine'] and scores[1:]==row['competitor_cosines'] and max(scores[1:])==row['best_competitor_cosine'] and scores[0]-max(scores[1:])==row['margin'];assert dot(vectors[6])==row['source_mask_cosine'];assert math.fsum(x*y for x,y in zip(vectors[1],vectors[6]))==row['baseline_mask_cosine']
  if case['label']=='self_control':assert row['pixel_tensor_sha256'][0]==row['pixel_tensor_sha256'][1] and vectors[0]==vectors[1] and abs(scores[0]-1)<1e-5
  summary.append({k:row[k] for k in ['label','direction','baseline_cosine','best_competitor_cosine','margin','common_fraction','source_mask_cosine','baseline_mask_cosine']})
F=D/'dedup-aligned-sscd-diagnostic-audit-20261013.json';assert not F.exists();F.write_text(json.dumps(dict(status='verified_fixed_plan_common_geometry_and_embedding_arithmetic',pins={str(v):sha(v) for v in [P,O,Path(__file__)]},required_directions=28,scored=len(summary),summary=summary,scope='Independent scalar Gaussian inverse, per-site common-support count and embedding dot/norm arithmetic; identity control requires exact source/baseline pixel-tensor hashes and vectors. Does not independently regenerate natural-image pixel tensors or reproduce neural inference. Shared mask, training-selected geometry/ROIs and alternative preprocessing remain diagnostic limitations; no threshold or acceptance change.'),indent=2)+'\n');print(F)
