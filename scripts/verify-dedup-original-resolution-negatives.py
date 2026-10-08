"""Independently check original-resolution negative output and region admission."""
import argparse,ast,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
r=json.loads(a.report.read_text());assert r['status']=='complete' or (a.checkpoint and r['status']=='running_negatives')
assert all(h(k)==v for k,v in r['input_hashes'].items())
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');tree=ast.parse(helper.read_text());definitions=[v for v in tree.body if isinstance(v,ast.FunctionDef) and v.name in ['verify_model','verify_regions']];assert len(definitions)==2
namespace={'math':math};exec(compile(ast.Module(body=definitions,type_ignores=[]),str(helper),'exec'),namespace)
metadata_path=next(Path(k) for k in r['input_hashes'] if k.endswith('/prepared.json'));m=json.loads(metadata_path.read_text());q=next(v for v in m['images']['strong'] if v['filename']=='200201.jpg');assert q['group_id']==200200
query_paths=[k for k in r['input_hashes'] if Path(k).name==q['filename']];assert len(query_paths)==1 and h(query_paths[0])==q['source_sha256']
expected=[v for v in m['images']['original'] if v['group_id']!=q['group_id']];assert len(expected)==156 and r['required_negatives']==[v['filename'] for v in expected]
assert len(r['results'])<=156
if not a.checkpoint:assert len(r['results'])==156
baseline_path=next(Path(k) for k in r['input_hashes'] if k.endswith('/dedup-original-resolution-three-crops.json'));baseline=next(v['evidence'] for v in json.loads(baseline_path.read_text())['results'] if v['query_id']=='200201.jpg')
canonical=lambda e:json.dumps({k:v for k,v in e.items() if k!='managed_peak'},sort_keys=True,allow_nan=False)
def verify_row(row):
 assert type(row['returncode']) is int and row['returncode']==0 and row['query']=='200201.jpg'
 e=row['evidence'];assert json.dumps(json.loads(row['stdout']),sort_keys=True,allow_nan=False)==json.dumps(e,sort_keys=True,allow_nan=False)
 assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0
 assert type(e['managed_peak']) is int and 0<=e['managed_peak']<=536870912
 assert [v['levels'] for v in e['stats']]==[4,7]
 positives=0
 for v in e['stats']:
  d=v['distinct'];points=d['correspondences'];assert all(len(p)==2 and all(len(x)==2 and all(type(y) in [int,float] and math.isfinite(y) for y in x) for x in p) for p in points)
  if d['matrix'] is not None:namespace['verify_model'](d['matrix'],d['inliers'],points)
  else:assert not d['inliers']
  assert type(d['region_support_count']) is int and len(d['similarity_candidates'])<=2
  namespace['verify_regions'](d,row['query']);positives+=d['region_support_count']
  for s in d['similarity_candidates']:
   namespace['verify_model'](s['matrix'],s['inliers'],points);assert s['matrix'][2]==[0.,0.,1.] and s['matrix'][0][0]==s['matrix'][1][1] and s['matrix'][0][1]==-s['matrix'][1][0]
   assert type(s['region_support_count']) is int;namespace['verify_regions'](s,row['query']);positives+=s['region_support_count']
 return positives
control=r['positive_control'];verify_row(control);assert canonical(control['evidence'])==canonical(baseline)
assert Path(control['left']).name=='200200.jpg'
original_control=next(v for v in m['images']['original'] if v['filename']=='200200.jpg');assert h(control['left'])==original_control['source_sha256']
for row,source in zip(r['results'],expected):
 assert Path(row['left']).name==source['filename'] and row['publisher_group_id']==source['group_id']!=q['group_id'];assert h(row['left'])==source['source_sha256']
 assert verify_row(row)==0
 assert all(v['distinct']['similarity_error'] is None for v in row['evidence']['stats']), 'Refused alternative is inconclusive, not a qualified negative'
assert all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_negative_prefix' if a.checkpoint else 'verified_complete_negative_gate','verified_pairs':len(r['results']),'required_pairs':156,'positive_exact_parity':True,'input_hashes':{str(a.report):h(a.report),str(helper):h(helper),str(Path(__file__)):h(__file__)},'scope':'Pinned original bytes, ordered different-origin labels, typed outputs, geometry residuals, regional acceptance arithmetic and positive control parity; finite one-query gate, not semantics/burst or general precision.'},indent=2)+'\n')
