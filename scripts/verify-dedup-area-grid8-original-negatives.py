"""Audit full/prefix same-recipe original-resolution156-negative gate."""
import argparse,ast,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
report_hash=h(a.report);r=json.loads(a.report.read_text());assert r['status']=='complete' or (a.checkpoint and r['status']=='running_negatives')
assert all(h(k)==v for k,v in r['input_hashes'].items())
metadata_path=next(Path(k) for k in r['input_hashes'] if Path(k).name=='prepared.json');m=json.loads(metadata_path.read_text())
q=next(v for v in m['images']['strong'] if v['filename']=='200101.jpg');assert r['query']==q['filename'] and r['query_group']==q['group_id']
def source_path(filename):
 paths=[Path(k) for k in r['input_hashes'] if Path(k).name==filename];assert len(paths)==1;return paths[0]
assert h(source_path(q['filename']))==q['source_sha256']
originals=m['images']['original'];assert len(originals)==157
for source in originals:assert h(source_path(source['filename']))==source['source_sha256']
expected=[v for v in originals if v['group_id']!=q['group_id']];assert len(expected)==156 and r['required_negatives']==156
assert len(r['results'])<=156
if not a.checkpoint:assert len(r['results'])==156

def jpeg_dimensions(path):
 data=path.read_bytes();assert data[:2]==b'\xff\xd8';i=2
 while i<len(data):
  assert data[i]==255
  while i<len(data) and data[i]==255:i+=1
  assert i<len(data);marker=data[i];i+=1
  if marker in [1,*range(0xd0,0xd9)]:continue
  assert i+2<=len(data);length=int.from_bytes(data[i:i+2],'big');assert length>=2 and i+length<=len(data)
  if marker in [0xc0,0xc1,0xc2,0xc3,0xc5,0xc6,0xc7,0xc9,0xca,0xcb,0xcd,0xce,0xcf]:
   assert length>=8;width=int.from_bytes(data[i+5:i+7],'big');height=int.from_bytes(data[i+3:i+5],'big');assert width>0 and height>0;return width,height
  assert marker not in [0xda,0xd9];i+=length
 raise AssertionError('Missing JPEG dimensions')

helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');helper_hash=h(helper);tree=ast.parse(helper.read_text())
nodes=[v for v in tree.body if isinstance(v,ast.FunctionDef) and v.name in ['verify_model','verify_regions']];assert len(nodes)==2
ns={'math':math};model=next(v for v in nodes if v.name=='verify_model');regions=next(v for v in nodes if v.name=='verify_regions')
exec(compile(ast.Module(body=[model],type_ignores=[]),str(helper),'exec'),ns)
# This frozen recipe declares128 regions; pixel acceptance stays identical.
region_source=ast.get_source_segment(helper.read_text(),regions);assert 'len(regions)<=32' in region_source
exec(compile(region_source.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
canonical=lambda value:json.dumps(value,sort_keys=True,allow_nan=False)
query_size=jpeg_dimensions(source_path(q['filename']))
def verify_row(row,original):
 assert type(row['returncode']) is int and row['returncode']==0
 e=row['evidence'];assert canonical(json.loads(row['stdout']))==canonical(e)
 assert e['status']=='ok' and type(e['whole_candidate']) is bool
 assert type(e['managed_used']) is int and e['managed_used']==0
 assert type(e['managed_peak']) is int and 0<=e['managed_peak']<=512*1024*1024
 assert type(e['retained_before_drop']) is int and 0<=e['retained_before_drop']<=e['managed_peak']
 points=e['correspondences'];assert all(len(v)==2 and all(len(x)==2 and all(type(y) in [int,float] and math.isfinite(y) for y in x) for x in v) for v in points)
 assert type(e['region_support_count']) is int and 0<=e['region_support_count']<=128
 if e['matrix'] is None:assert not e['inliers'] and not e['regions'] and not e['whole_candidate'] and e['region_support_count']==0
 else:ns['verify_model'](e['matrix'],e['inliers'],points);ns['verify_regions'](e,q['filename'])
 original_size=jpeg_dimensions(source_path(original['filename']))
 for region in e['regions']:
  for (x,y,w,height),(width,limit_height) in zip(region['domains'],[original_size,query_size]):assert x+w<=width and y+height<=limit_height
 return e
positive=next(v for v in originals if v['group_id']==q['group_id'])
control=verify_row(r['positive_control'],positive)
baseline_path=next(Path(k) for k in r['input_hashes'] if Path(k).name=='dedup-original-area-grid8-blur7-200101.json');baseline=json.loads(baseline_path.read_text());assert baseline['status']=='verified_native_output_parity'
assert all(h(k)==v for k,v in baseline['input_hashes'].items());assert canonical(json.loads(baseline['stdout']))==canonical(baseline['evidence'])
keys=['whole_candidate','correspondences','matrix','inliers','regions','region_support_count','retained_before_drop','managed_used']
assert canonical({k:control[k] for k in keys})==canonical({k:baseline['evidence'][k] for k in keys})
assert control['region_support_count']==7 and not control['whole_candidate']
for row,source in zip(r['results'],expected):
 assert row['original']==source['filename'] and row['original_group']==source['group_id']!=q['group_id']
 e=verify_row(row,source);assert not e['whole_candidate'] and e['region_support_count']==0
assert h(a.report)==report_hash and h(helper)==helper_hash and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_area_grid8_negative_prefix' if a.checkpoint else 'verified_area_grid8_complete_negative_gate','verified_pairs':len(r['results']),'required_pairs':156,'positive_exact_parity':True,'input_hashes':{str(a.report):report_hash,str(helper):helper_hash,str(Path(__file__)):h(__file__)},'scope':'Same frozen recipe positive/control parity, all157 original source hashes, ordered different-origin labels, typed native outputs, residuals, regional arithmetic and JPEG domain bounds. Finite one-query gate; no semantic/burst/all-query precision or full pixel-resampling oracle.'},indent=2)+'\n')
