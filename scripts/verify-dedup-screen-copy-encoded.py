"""Independently audit one encoded-sRGB screen-copy experiment, including a miss."""
import argparse,ast,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();report_hash=h(a.report);r=json.loads(a.report.read_text());assert r['status']=='complete_native_experiment' and type(r['returncode']) is int and r['returncode']==0
assert r['original']=='200500.jpg' and r['query']=='200501.jpg' and r['candidate_smoothing_radius']==2 and r['color_space']=='EncodedSrgb' and all(h(k)==v for k,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
m=json.loads(pinned('prepared.json').read_text());dims=[]
for key,split in [('original','original'),('query','strong')]:
 row=next(v for v in m['images'][split] if v['filename']==r[key]);assert row['group_id']==200500 and h(pinned(r[key]))==row['source_sha256'];dims.append(oriented_dimensions(pinned(r[key]),row['source_size']))
baseline_path=Path('docs/research/dedup-fresh-union-original-full-strict-checkpoint-9-next.json');baseline_audit_path=baseline_path.with_name(baseline_path.stem+'-audit.json');baseline_hash=h(baseline_path);assert json.loads(baseline_audit_path.read_text())['input_hashes'][str(baseline_path)]==baseline_hash;baseline=next(x['evidence'] for x in json.loads(baseline_path.read_text())['results'] if x['query']=='200501.jpg')
e=r['evidence'];assert all(e[k]==baseline[k] for k in ['correspondences','matrix','inliers']);canonical=lambda x:json.dumps(x,sort_keys=True,allow_nan=False);assert e['status']=='ok' and canonical(json.loads(r['stdout']))==canonical(e)
for k in ['managed_used','managed_peak','retained_before_drop','region_support_count']:assert type(e[k]) is int and e[k]>=0
assert e['managed_used']==0 and e['retained_before_drop']<=e['managed_peak']<=512*1024*1024 and e['region_support_count']<=128
points=e['correspondences'];assert len(points)<=28000
for point in points:
 assert len(point)==2
 for xy,(w,height) in zip(point,dims):
  assert len(xy)==2 and all(type(v) in [int,float] and math.isfinite(v) for v in xy);assert 0<=xy[0]<w and 0<=xy[1]<height
for i,point in enumerate(points):
 for prior in points[:i]:assert all(math.hypot(x[0]-y[0],x[1]-y[1])>2 for x,y in zip(point,prior))
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');helper_hash=h(helper);s=helper.read_text();nodes={v.name:v for v in ast.parse(s).body if isinstance(v,ast.FunctionDef)};ns={'math':math}
exec(compile(ast.Module(body=[nodes['verify_model']],type_ignores=[]),str(helper),'exec'),ns);region_source=ast.get_source_segment(s,nodes['verify_regions']);assert 'len(regions)<=32' in region_source;exec(compile(region_source.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
if e['matrix'] is None:assert not e['inliers'] and not e['regions'] and not e['region_support_count']
else:assert len(e['inliers'])>=10;ns['verify_model'](e['matrix'],e['inliers'],points);ns['verify_regions'](e,r['query'])
for region in e['regions']:
 for (x,y,w,height),(width,limit_height) in zip(region['domains'],dims):assert x+w<=width and y+height<=limit_height
assert h(baseline_path)==baseline_hash
assert h(a.report)==report_hash and h(helper)==helper_hash and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_native_encoded_screen_experiment','matches':len(points),'inliers':len(e['inliers']),'support_count':e['region_support_count'],'outcome':'local_supported' if e['region_support_count'] else 'miss','input_hashes':{str(a.report):report_hash,str(baseline_path):baseline_hash,str(baseline_audit_path):h(baseline_audit_path),str(helper):helper_hash,str(Path(__file__)):h(__file__),'scripts/dedup_jpeg_domain.py':h('scripts/dedup_jpeg_domain.py')},'scope':'Pinned source JPEGs, typed resources, distinct locations, residuals and regional arithmetic/domain bounds. Single diagnostic, no independent full pixel-resampling oracle, true fold registration, negative precision or default promotion.'},indent=2)+'\n')
