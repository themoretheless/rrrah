"""Verify a frozen prefix or terminal same-recipe624 different-origin gate.
False local supports and native errors remain explicit outcomes.
"""
import argparse,ast,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();report_hash=h(a.report);r=json.loads(a.report.read_text());assert r['status']=='complete' or a.checkpoint and r['status']=='running_negatives';assert r['required_negatives']==624
assert all(h(k)==v for k,v in r['input_hashes'].items())
by_name={Path(k).name:Path(k) for k in r['input_hashes']};m=json.loads(by_name['prepared.json'].read_text());positive=json.loads(by_name['dedup-main-photometric-misses-radius8-audit.json'].read_text());assert positive['status']=='verified_fixed_geometry_radius8_all_photometric_misses';assert all(h(k)==v for k,v in positive['input_hashes'].items())
queries=[v['query'] for v in positive['summary'] if v['supported_regions']];assert r['queries']==queries and set(queries)=={'205901.jpg','206301.jpg','207002.jpg','207401.jpg'}
strong={v['filename']:v for v in m['images']['strong']};originals=m['images']['original'];assert len(originals)==157
for v in originals:assert h(by_name[v['filename']])==v['source_sha256']
for name in queries:assert h(by_name[name])==strong[name]['source_sha256']
expected=[(q,v) for q in queries for v in originals if v['group_id']!=strong[q]['group_id']];assert len(expected)==624 and len(r['results'])<=624
if not a.checkpoint:assert r['status']=='complete' and len(r['results'])==624
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');s=helper.read_text();nodes={v.name:v for v in ast.parse(s).body if isinstance(v,ast.FunctionDef)};ns={'math':math};exec(compile(ast.Module(body=[nodes['verify_model']],type_ignores=[]),str(helper),'exec'),ns);fn=ast.get_source_segment(s,nodes['verify_regions']);assert 'len(regions)<=32' in fn;exec(compile(fn.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
summary=[]
for row,(query,v) in zip(r['results'],expected):
 assert row['query']==query and row['original']==v['filename'] and row['original_group']==v['group_id'] and row['query_group']==strong[query]['group_id'] and row['original_group']!=row['query_group'];assert type(row['returncode']) is int
 if row['returncode']:
  assert 'evidence' not in row;summary.append({'original':row['original'],'query':query,'outcome':'native_error'});continue
 e=row['evidence'];assert json.loads(row['stdout'])==e and e['status']=='ok';assert 0<=e['retained_before_drop']<=e['managed_peak']<=512*1024*1024 and e['managed_used']==0
 dims=[oriented_dimensions(by_name[row['original']],v['source_size']),oriented_dimensions(by_name[query],strong[query]['source_size'])];points=e['correspondences'];assert len(points)<=28000
 for point in points:
  assert len(point)==2
  for xy,(w,height) in zip(point,dims):assert len(xy)==2 and all(type(n) in [int,float] and math.isfinite(n) for n in xy) and 0<=xy[0]<w and 0<=xy[1]<height
 for i,point in enumerate(points):
  for prior in points[:i]:assert all(math.hypot(x[0]-y[0],x[1]-y[1])>2 for x,y in zip(point,prior))
 if e['matrix'] is None:assert not e['inliers'] and not e['regions'] and e['region_support_count']==0
 else:
  assert len(e['inliers'])>=10;ns['verify_model'](e['matrix'],e['inliers'],points);ns['verify_regions'](e,query)
 for region in e['regions']:
  for (x,y,w,height),(width,limit_height) in zip(region['domains'],dims):assert x+w<=width and y+height<=limit_height
 summary.append({'original':row['original'],'query':query,'outcome':'false_local_support' if e['region_support_count'] else 'rejected','supported_regions':e['region_support_count']})
assert h(a.report)==report_hash and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_same_recipe_radius8_negative_prefix' if a.checkpoint else 'verified_same_recipe_radius8_negative_terminal','verified_rows':len(summary),'required_negatives':624,'counts':{k:sum(v['outcome']==k for v in summary) for k in ['rejected','false_local_support','native_error']},'summary':summary,'pins':{str(p):h(p) for p in [a.report,helper,Path(__file__)]},'scope':'Frozen exact-order publisher-origin negatives, source hashes, oriented dimensions, spatially distinct points, geometry/region predicates and managed payload bounds. No independent pixel resampling, semantic/burst precision or promotion claim; prefix is incomplete.'},indent=2)+'\n');print('verified',len(summary),'rows')
