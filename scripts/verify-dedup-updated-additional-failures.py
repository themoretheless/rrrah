"""Audit all-source union output; native errors remain unresolved denominator rows."""
import argparse,ast,hashlib,json,math
from dedup_jpeg_domain import oriented_dimensions
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
report_hash=h(a.report);r=json.loads(a.report.read_text());assert r['status']=='complete' or a.checkpoint and r['status']=='running_pairs'
assert r['max_combined_domain_pixels']==12800000 and r['required_pairs']==2 and all(h(k)==v for k,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
m=json.loads(pinned('prepared.json').read_text());sm=json.loads(pinned('dedup-original-strong-inputs.json').read_text());assert sm['status']=='verified_original_strong_archive_bytes' and sm['count']==229 and all(h(k)==v for k,v in sm['input_hashes'].items())
strong={v['name']:v for v in sm['records']};assert len(strong)==229
for image in m['images']['original']:assert h(pinned(image['filename']))==image['source_sha256']
for image in m['images']['strong']:assert strong[image['filename']]['sha256']==image['source_sha256']==h(strong[image['filename']]['path'])
positive=json.loads(pinned('dedup-combined-domain-release.json').read_text());audit=json.loads(pinned('dedup-combined-domain-release-terminal-audit.json').read_text());assert positive['status']=='verified_native_release_parity' and audit['status']=='verified_repeat' and len(positive['results'])==5 and len(audit['summary'])==5
assert all(h(k)==v for doc in [positive,audit] for k,v in doc['input_hashes'].items());exe=pinned('gradient-scales-probe-combined-domain-release');assert h(exe)==positive['input_hashes'][str(exe)]
canonical=lambda e:json.dumps(e,sort_keys=True,allow_nan=False)
keys=['correspondences','matrix','inliers','regions','region_support_count','retained_before_drop','managed_used']
control=r['positive_control'];assert type(control['returncode']) is int and control['returncode']==0 and canonical(json.loads(control['stdout']))==canonical(control['evidence'])
assert canonical({k:control['evidence'][k] for k in keys})==canonical({k:positive['results'][0]['evidence'][k] for k in keys})
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');helper_hash=h(helper);source=helper.read_text();nodes={v.name:v for v in ast.parse(source).body if isinstance(v,ast.FunctionDef)};ns={'math':math}
exec(compile(ast.Module(body=[nodes['verify_model']],type_ignores=[]),str(helper),'exec'),ns)
region_source=ast.get_source_segment(source,nodes['verify_regions']);assert 'len(regions)<=32' in region_source;exec(compile(region_source.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
expected=[v for v in m['positive_pairs'] if v['right']['filename'] in ['201303.jpg','201401.jpg']];assert len(expected)==2 and len(r['results'])<=2
if r['status']=='complete' or not a.checkpoint:assert len(r['results'])==2
summary=[]
for row,pair in zip(r['results'],expected):
 assert row['original']==pair['left']['filename'] and row['query']==pair['right']['filename'] and type(row['group_id']) is int and row['group_id']==pair['left']['group_id']==pair['right']['group_id']
 assert type(row['returncode']) is int and type(row['stdout']) is str and type(row['stderr']) is str
 if row['returncode']:
  assert 'evidence' not in row;summary.append({'query':row['query'],'outcome':'native_error','returncode':row['returncode']});continue
 e=row['evidence'];assert e['status']=='ok' and canonical(json.loads(row['stdout']))==canonical(e)
 for k in ['managed_used','managed_peak','retained_before_drop','region_support_count']:assert type(e[k]) is int and e[k]>=0
 assert e['managed_used']==0 and e['retained_before_drop']<=e['managed_peak']<=512*1024*1024 and e['region_support_count']<=128
 dimensions=[oriented_dimensions(pinned(pair['left']['filename']),pair['left']['source_size']),oriented_dimensions(strong[pair['right']['filename']]['path'],pair['right']['source_size'])];points=e['correspondences'];assert len(points)<=28000
 for point in points:
  assert len(point)==2
  for xy,(w,height) in zip(point,dimensions):
   assert len(xy)==2 and all(type(v) in [int,float] and math.isfinite(v) for v in xy);assert 0<=xy[0]<w and 0<=xy[1]<height
 for pos,point in enumerate(points):
  for prior in points[:pos]:assert all(math.hypot(x[0]-y[0],x[1]-y[1])>2 for x,y in zip(point,prior))
 if e['matrix'] is None:assert not e['inliers'] and not e['regions'] and e['region_support_count']==0
 else:assert len(e['inliers'])>=10;ns['verify_model'](e['matrix'],e['inliers'],points);ns['verify_regions'](e,row['query'])
 for region in e['regions']:
  for (x,y,w,height),(width,limit_height) in zip(region['domains'],dimensions):assert x+w<=width and y+height<=limit_height
 summary.append({'query':row['query'],'outcome':'local_supported' if e['region_support_count'] else 'miss','matches':len(points),'inliers':len(e['inliers']),'support_count':e['region_support_count']})
assert h(a.report)==report_hash and h(helper)==helper_hash and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_updated_additional_failure_prefix' if a.checkpoint else 'verified_updated_additional_failure_evidence','verified_rows':len(summary),'required_pairs':2,'counts':{outcome:sum(v['outcome']==outcome for v in summary) for outcome in ['local_supported','miss','native_error']},'summary':summary,'input_hashes':{str(a.report):report_hash,str(helper):helper_hash,'scripts/dedup_jpeg_domain.py':h('scripts/dedup_jpeg_domain.py'),str(Path(__file__)):h(__file__)},'scope':'Source/archive hashes, ordered origin pairs, same-recipe positive parity, typed stdout, distinct locations, geometry residuals and regional arithmetic/domain bounds. Errors remain unresolved denominator rows. No whole identity, semantic precision, full-pixel resampling oracle or indexed full-collection proof.'},indent=2)+'\n')
