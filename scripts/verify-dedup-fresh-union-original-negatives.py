"""Audit same-recipe full one-query different-origin gate for new union recovery."""
import argparse,ast,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();report_hash=h(a.report);r=json.loads(a.report.read_text());assert r['status']=='complete' or a.checkpoint and r['status']=='running_negatives'
assert all(h(k)==v for k,v in r['input_hashes'].items());assert r['query']=='200301.jpg' and type(r['query_group']) is int and r['query_group']==200300 and r['required_negatives']==156
assert len(r['results'])<=156
if not a.checkpoint:assert len(r['results'])==156
manifest=Path('docs/research/dedup-original-resolution-hard-crops.json');m=json.loads(manifest.read_text());assert m['status']=='verified_selected_archive_bytes' and all(h(k)==v for k,v in m['input_hashes'].items())
for n in ['200100.jpg','200200.jpg','200301.jpg']:
 path=next(k for k in r['input_hashes'] if Path(k).name==n);assert r['input_hashes'][path]==next(v for k,v in m['input_hashes'].items() if Path(k).name==n)
positive_path=next(Path(k) for k in r['input_hashes'] if Path(k).name=='dedup-fresh-candidate-union-200301.json');positive=json.loads(positive_path.read_text());assert positive['status']=='verified_fresh_native_diagnostic_parity' and all(h(k)==v for k,v in positive['input_hashes'].items())
exe=next(k for k in r['input_hashes'] if Path(k).name=='gradient-scales-probe-fresh-candidate-union');assert r['input_hashes'][exe]==positive['input_hashes'][exe]
audit_path=next(Path(k) for k in r['input_hashes'] if Path(k).name=='dedup-fresh-candidate-union-200301-audit.json');audit=json.loads(audit_path.read_text());assert audit['status']=='verified_fresh_union_file_parity' and all(h(k)==v for k,v in audit['input_hashes'].items())
canonical=lambda v:json.dumps(v,sort_keys=True,allow_nan=False);assert canonical(json.loads(positive['stdout']))==canonical(positive['evidence']);assert positive['evidence']['region_support_count']==47
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');helper_hash=h(helper);source=helper.read_text();nodes={v.name:v for v in ast.parse(source).body if isinstance(v,ast.FunctionDef)};ns={'math':math}
exec(compile(ast.Module(body=[nodes['verify_model']],type_ignores=[]),str(helper),'exec'),ns);region_source=ast.get_source_segment(source,nodes['verify_regions']);assert 'len(regions)<=32' in region_source;exec(compile(region_source.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
prepared_path=next(Path(k) for k in r['input_hashes'] if Path(k).name=='prepared.json');prepared=json.loads(prepared_path.read_text())
originals=prepared['images']['original'];assert len(originals)==157
query_row=next(v for v in prepared['images']['strong'] if v['filename']==r['query']);assert query_row['group_id']==r['query_group']
query_path=next(k for k in r['input_hashes'] if Path(k).name==r['query']);assert r['input_hashes'][query_path]==query_row['source_sha256']
for original in originals:
 path=next(k for k in r['input_hashes'] if Path(k).name==original['filename']);assert r['input_hashes'][path]==original['source_sha256']
 assert original['source_archive_sha256']=='8b85b7914fd0145bc258bde3440f6a622243ca91ee085fe2025ce7bdeb0bfb8e'
negative_rows=[v for v in originals if v['group_id']!=r['query_group']];assert len(negative_rows)==156
keys=['correspondences','matrix','inliers','regions','region_support_count','retained_before_drop','managed_used']
control=r['positive_control'];assert type(control['returncode']) is int and control['returncode']==0
assert canonical(json.loads(control['stdout']))==canonical(control['evidence'])
assert canonical({k:control['evidence'][k] for k in keys})==canonical({k:positive['evidence'][k] for k in keys})
assert control['evidence']['managed_peak']<=512*1024*1024
for row,original in zip(r['results'],negative_rows):
 assert row['original']==original['filename'] and type(row['original_group']) is int and row['original_group']==original['group_id']!=r['query_group']
 dimensions=[original['source_size'],query_row['source_size']]
 assert type(row['returncode']) is int and row['returncode']==0;e=row['evidence'];assert canonical(json.loads(row['stdout']))==canonical(e) and e['status']=='ok'
 for k in ['managed_used','managed_peak','retained_before_drop','region_support_count']:assert type(e[k]) is int and e[k]>=0
 assert e['managed_used']==0 and e['retained_before_drop']<=e['managed_peak']<=512*1024*1024 and e['region_support_count']==0
 points=e['correspondences'];assert len(points)<=28000 and all(len(v)==2 and all(len(x)==2 and all(type(y) in [int,float] and math.isfinite(y) for y in x) for x in v) for v in points)
 for point in points:
  for (x,y),(w,height) in zip(point,dimensions):assert 0<=x<w and 0<=y<height
 for i,point in enumerate(points):
  for prior in points[:i]:assert all(math.hypot(a[0]-b[0],a[1]-b[1])>2 for a,b in zip(point,prior))
 if e['matrix'] is None:assert not e['inliers'] and not e['regions']
 else:assert len(e['inliers'])>=10;ns['verify_model'](e['matrix'],e['inliers'],points);ns['verify_regions'](e,'200301.jpg')
 for region in e['regions']:
  for (x,y,w,height),(width,limit_height) in zip(region['domains'],dimensions):assert x+w<=width and y+height<=limit_height
assert h(a.report)==report_hash and h(helper)==helper_hash and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_fresh_union_negative_prefix' if a.checkpoint else 'verified_fresh_union_original_negatives','verified_pairs':len(r['results']),'required_pairs':156,'positive_support_count':47,'input_hashes':{str(a.report):report_hash,str(helper):helper_hash,str(manifest):h(manifest),str(Path(__file__)):h(__file__)},'scope':'Full one-query156 different publisher-origin comparisons. Same frozen native recipe and qualified known positive; source/archive pins, labels, raw typed evidence, memory lifecycle, independent locations, geometric residuals and regional counts/domains. Finite controls, no semantic/all-query precision or full pixel-resampling oracle.'},indent=2)+'\n')
