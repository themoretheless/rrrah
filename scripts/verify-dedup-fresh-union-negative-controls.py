"""Audit same-recipe finite different-origin controls for new union recovery."""
import argparse,ast,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();report_hash=h(a.report);r=json.loads(a.report.read_text());assert r['status']=='complete' or a.checkpoint and r['status']=='running_negatives'
assert all(h(k)==v for k,v in r['input_hashes'].items());assert r['query']=='200301.jpg' and type(r['query_group']) is int and r['query_group']==200300 and r['required_negatives']==2
assert len(r['results'])<=2
if not a.checkpoint:assert len(r['results'])==2
manifest=Path('docs/research/dedup-original-resolution-hard-crops.json');m=json.loads(manifest.read_text());assert m['status']=='verified_selected_archive_bytes' and all(h(k)==v for k,v in m['input_hashes'].items())
for n in ['200100.jpg','200200.jpg','200301.jpg']:
 path=next(k for k in r['input_hashes'] if Path(k).name==n);assert r['input_hashes'][path]==m['input_hashes'][path]
positive_path=next(Path(k) for k in r['input_hashes'] if Path(k).name=='dedup-fresh-candidate-union-200301.json');positive=json.loads(positive_path.read_text());assert positive['status']=='verified_fresh_native_diagnostic_parity' and all(h(k)==v for k,v in positive['input_hashes'].items())
exe=next(k for k in r['input_hashes'] if Path(k).name=='gradient-scales-probe-fresh-candidate-union');assert r['input_hashes'][exe]==positive['input_hashes'][exe]
audit_path=next(Path(k) for k in r['input_hashes'] if Path(k).name=='dedup-fresh-candidate-union-200301-audit.json');audit=json.loads(audit_path.read_text());assert audit['status']=='verified_fresh_union_file_parity' and all(h(k)==v for k,v in audit['input_hashes'].items())
canonical=lambda v:json.dumps(v,sort_keys=True,allow_nan=False);assert canonical(json.loads(positive['stdout']))==canonical(positive['evidence']);assert positive['evidence']['region_support_count']==47
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');helper_hash=h(helper);source=helper.read_text();nodes={v.name:v for v in ast.parse(source).body if isinstance(v,ast.FunctionDef)};ns={'math':math}
exec(compile(ast.Module(body=[nodes['verify_model']],type_ignores=[]),str(helper),'exec'),ns);region_source=ast.get_source_segment(source,nodes['verify_regions']);assert 'len(regions)<=32' in region_source;exec(compile(region_source.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
for row,name,group in zip(r['results'],['200100.jpg','200200.jpg'],[200100,200200]):
 assert row['original']==name and type(row['original_group']) is int and row['original_group']==group!=r['query_group']
 assert type(row['returncode']) is int and row['returncode']==0;e=row['evidence'];assert canonical(json.loads(row['stdout']))==canonical(e) and e['status']=='ok'
 for k in ['managed_used','managed_peak','retained_before_drop','region_support_count']:assert type(e[k]) is int and e[k]>=0
 assert e['managed_used']==0 and e['retained_before_drop']<=e['managed_peak']<=512*1024*1024 and e['region_support_count']==0
 points=e['correspondences'];assert len(points)<=28000 and all(len(v)==2 and all(len(x)==2 and all(type(y) in [int,float] and math.isfinite(y) for y in x) for x in v) for v in points)
 for point in points:
  for (x,y),(w,height) in zip(point,[(2048,1536),(692,349)]):assert 0<=x<w and 0<=y<height
 for i,point in enumerate(points):
  for prior in points[:i]:assert all(math.hypot(a[0]-b[0],a[1]-b[1])>2 for a,b in zip(point,prior))
 if e['matrix'] is None:assert not e['inliers'] and not e['regions']
 else:assert len(e['inliers'])>=10;ns['verify_model'](e['matrix'],e['inliers'],points);ns['verify_regions'](e,'200301.jpg')
 for region in e['regions']:
  for (x,y,w,height),(width,limit_height) in zip(region['domains'],[(2048,1536),(692,349)]):assert x+w<=width and y+height<=limit_height
assert h(a.report)==report_hash and h(helper)==helper_hash and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_fresh_union_negative_prefix' if a.checkpoint else 'verified_fresh_union_negative_controls','verified_pairs':len(r['results']),'required_pairs':2,'positive_support_count':47,'input_hashes':{str(a.report):report_hash,str(helper):helper_hash,str(manifest):h(manifest),str(Path(__file__)):h(__file__)},'scope':'Same frozen native recipe and qualified known positive; source/archive pins, labels, raw typed evidence, memory lifecycle, independent locations, geometric residuals and regional counts/domains. Finite controls, no semantic/all-query precision or full pixel-resampling oracle.'},indent=2)+'\n')
