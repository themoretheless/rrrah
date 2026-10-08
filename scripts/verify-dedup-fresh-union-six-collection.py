"""Independently audit six-source collection/fresh pair checkpoints."""
import argparse,ast,hashlib,itertools,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
report_hash=h(a.report);r=json.loads(a.report.read_text())
assert r['status']=='complete_native_pair_parity' or (a.checkpoint and r['status']=='running_fresh_pairs')
assert all(h(k)==v for k,v in r['input_hashes'].items())
names=['200100.jpg','200101.jpg','200200.jpg','200201.jpg','200300.jpg','200301.jpg'];assert r['files']==names
manifest=next(Path(k) for k in r['input_hashes'] if Path(k).name=='dedup-original-resolution-hard-crops.json');m=json.loads(manifest.read_text());assert m['status']=='verified_selected_archive_bytes'
assert all(h(k)==v for k,v in m['input_hashes'].items())
dimensions={v['name']:v['dimensions'] for v in m['records']}
for n in names:
 path=next(k for k in r['input_hashes'] if Path(k).name==n);assert r['input_hashes'][path]==m['input_hashes'][path]
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');helper_hash=h(helper);source=helper.read_text();nodes={v.name:v for v in ast.parse(source).body if isinstance(v,ast.FunctionDef)};ns={'math':math}
exec(compile(ast.Module(body=[nodes['verify_model']],type_ignores=[]),str(helper),'exec'),ns)
region_source=ast.get_source_segment(source,nodes['verify_regions']);assert 'len(regions)<=32' in region_source
exec(compile(region_source.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
def integer(v):assert type(v) is int and v>=0

def evidence(e,i,j,managed=False):
 integer(e['region_support_count']);assert e['region_support_count']<=128
 points=e['correspondences'];assert all(len(v)==2 and all(len(x)==2 and all(type(y) in [int,float] and math.isfinite(y) for y in x) for x in v) for v in points)
 assert len(points)<=28000
 for point in points:
  for (x,y),(w,height) in zip(point,[dimensions[names[i-1]],dimensions[names[j-1]]]):assert 0<=x<w and 0<=y<height
 for pos,point in enumerate(points):
  for prior in points[:pos]:assert all(math.hypot(a[0]-b[0],a[1]-b[1])>2 for a,b in zip(point,prior))
 if e['matrix'] is None:assert not e['inliers'] and not e['regions'] and e['region_support_count']==0
 else:assert len(e['inliers'])>=10;ns['verify_model'](e['matrix'],e['inliers'],points);ns['verify_regions'](e,names[j-1])
 for region in e['regions']:
  for (x,y,w,height),(width,limit_height) in zip(region['domains'],[dimensions[names[i-1]],dimensions[names[j-1]]]):assert x+w<=width and y+height<=limit_height
 if managed:
  assert e['status']=='ok'
  for k in ['managed_used','managed_peak','retained_before_drop']:integer(e[k])
  assert e['managed_used']==0 and e['retained_before_drop']<=e['managed_peak']<=512*1024*1024
 return e['region_support_count']>0
c=r['collection'];assert c['status']=='ok' and c['analysed']==list(range(1,7)) and c['insufficient_features']==[]
for k in ['indexed_features','descriptor_hits','proposed_pairs','pixel_verification_pairs','managed_used','managed_peak','retained_before_drop']:integer(c[k])
assert c['managed_used']==0 and c['retained_before_drop']<=c['managed_peak']<=512*1024*1024
assert len(c['pairs'])==c['pixel_verification_pairs']==c['proposed_pairs']<=15
indexed={}
for e in c['pairs']:
 i,j=e['left'],e['right'];integer(i);integer(j);assert 1<=i<j<=6 and (i,j) not in indexed;indexed[i,j]=e
 accepted=evidence(e,i,j);assert not accepted or (i-1)//2==(j-1)//2
keys=['correspondences','matrix','inliers','regions','region_support_count'];canonical=lambda e:json.dumps({k:e[k] for k in keys},sort_keys=True,allow_nan=False)
expected=list(itertools.combinations(range(1,7),2));assert len(r['results'])<=15
if not a.checkpoint:assert len(r['results'])==15
for row,(i,j) in zip(r['results'],expected):
 assert row['left']==i and row['right']==j and type(row['same_origin']) is bool and row['same_origin']==((i-1)//2==(j-1)//2)
 assert type(row['retrieved']) is bool and row['retrieved']==((i,j) in indexed)
 accepted=evidence(row['evidence'],i,j,True);assert not accepted or row['same_origin'] and row['retrieved']
 if row['retrieved']:assert canonical(row['evidence'])==canonical(indexed[i,j])
outputs=r['native_outputs'];assert len(outputs)==len(r['results'])+1
assert outputs[0]['args'][0]=='original-union-collection-six'
assert json.dumps(json.loads(outputs[0]['stdout']),sort_keys=True,allow_nan=False)==json.dumps(c,sort_keys=True,allow_nan=False)
for raw,row in zip(outputs[1:],r['results']):
 assert raw['args'][0]=='original-managed-candidate-union'
 assert [Path(v).name for v in raw['args'][1:]]==[names[row['left']-1],names[row['right']-1]]
 assert json.dumps(json.loads(raw['stdout']),sort_keys=True,allow_nan=False)==json.dumps(row['evidence'],sort_keys=True,allow_nan=False)
for raw in outputs:assert type(raw['returncode']) is int and raw['returncode']==0
assert h(a.report)==report_hash and h(helper)==helper_hash and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_union_six_collection_prefix' if a.checkpoint else 'verified_union_six_collection_pair_parity','verified_fresh_pairs':len(r['results']),'required_pairs':15,'retrieved_pairs':len(indexed),'input_hashes':{str(a.report):report_hash,str(helper):helper_hash,str(Path(__file__)):h(__file__)},'scope':'Source/archive pins, typed counters, geometry residuals, regional arithmetic/domain bounds, distinct origin refusal and exact fresh/collection evidence parity. Finite six-source check; no full corpus recall or complete RSS bound.'},indent=2)+'\n')
