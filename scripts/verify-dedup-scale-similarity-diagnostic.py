"""Check diagnostic provenance and reported geometric residuals, not crop truth."""
import argparse,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');p.add_argument('--single',action='store_true');p.add_argument('--pixels',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
r=json.loads(a.report.read_text());assert r['status']=='complete' or (a.checkpoint and r['status']=='running');assert all(h(k)==v for k,v in r['input_hashes'].items())
expected=['200101.jpg'] if a.single else ['200101.jpg','200201.jpg','200301.jpg'];assert 0<len(r['results'])<=len(expected)
assert [v['query_id'] for v in r['results']]==expected[:len(r['results'])]
if not a.checkpoint:assert len(r['results'])==len(expected)
summary=[]
def verify_model(model,indices,points):
 assert len(model)==3 and all(len(row)==3 and all(type(x) in [int,float] and math.isfinite(x) for x in row) for row in model)
 assert len(set(indices))==len(indices)
 for i in indices:
  assert type(i) is int and 0<=i<len(points)
  x,y=points[i][0];tx,ty=points[i][1];z=model[2][0]*x+model[2][1]*y+model[2][2];assert z!=0
  u=(model[0][0]*x+model[0][1]*y+model[0][2])/z;v=(model[1][0]*x+model[1][1]*y+model[1][2])/z
  assert math.isfinite(u) and math.isfinite(v) and (u-tx)**2+(v-ty)**2<=4.0000001
def verify_regions(model,query):
 regions=model['regions'];assert len(regions)<=32
 for v in regions:
  assert type(v['accepted']) is bool and len(v['domains'])==2
  for rect in v['domains']:
   assert len(rect)==4 and all(type(n) is int for n in rect)
   x,y,w,h=rect;assert x>=0 and y>=0 and w>0 and h>0
  pixels=v['pixels'];accepted=False
  if pixels is not None:
   assert v['fit_failure'] is None
   counts=pixels['counts'];assert len(counts)==2
   for rect,c in zip(v['domains'],counts):
    assert len(c)==3 and all(type(n) is int for n in c)
    matched,compared,total=c;assert 0<=matched<=compared<=total and total==rect[2]*rect[3]
   for gains,offsets in zip(pixels['gains'],pixels['offsets']):
    assert len(gains)==len(offsets)==3
    assert all(type(n) in [int,float] and math.isfinite(n) and .2-1e-9<=n<=5+1e-9 for n in gains)
    assert all(type(n) in [int,float] and math.isfinite(n) and abs(n)<=.1+1e-9 for n in offsets)
   accepted=all(c>=1000 and c/t>=.3 and m/c>=.9 for m,c,t in counts)
  assert v['accepted']==accepted
 assert sum(v['accepted'] for v in regions)==model['region_support_count']
for row in r['results']:
 assert type(row['returncode']) is int and row['returncode']==0
 raw=json.loads(row['stdout']);e=row['evidence'];assert json.dumps(raw,sort_keys=True,allow_nan=False)==json.dumps(e,sort_keys=True,allow_nan=False)
 assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0
 assert [v['levels'] for v in e['stats']]==[4,7]
 for v in e['stats']:
  d=v['distinct'];points=d['correspondences'];assert all(len(p)==2 and all(len(x)==2 and all(type(y) in [int,float] and math.isfinite(y) for y in x) for x in p) for p in points)
  if d['matrix'] is not None:verify_model(d['matrix'],d['inliers'],points)
  else:assert not d['inliers']
  assert type(d['region_support_count']) is int and 0<=d['region_support_count']<=32
  if a.pixels:verify_regions(d,row['query_id'])
  assert len(d['similarity_candidates'])<=2
  for s in d['similarity_candidates']:
   m=s['matrix'];verify_model(m,s['inliers'],points)
   assert m[2]==[0.,0.,1.] and m[0][0]==m[1][1] and m[0][1]==-m[1][0]
   assert type(s['region_support_count']) is int and 0<=s['region_support_count']<=32
   if a.pixels:verify_regions(s,row['query_id'])
  summary.append({'query':row['query_id'],'levels':v['levels'],'matches':len(points),'projective_support':d['region_support_count'],'similarity_error':d['similarity_error'],'similarity_support':[s['region_support_count'] for s in d['similarity_candidates']]})
a.output.write_text(json.dumps({'status':'verified_diagnostic_geometry' if r['status']=='complete' and len(r['results'])==len(expected) else 'verified_diagnostic_prefix','summary':summary,'input_hashes':{str(a.report):h(a.report),str(Path(__file__)):h(__file__)},'pixel_arithmetic_verified':a.pixels,'scope':'Raw typed output, ordered positive labels, source pins and reported inlier residuals; local count alone not independently pixel audited, not true crop transform or negative precision.'},indent=2)+'\n')
