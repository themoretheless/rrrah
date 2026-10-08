"""Audit all three bounded-memory repeats, preserving errors separately."""
import argparse,ast,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();rh=h(a.report);r=json.loads(a.report.read_text());assert r['status']=='complete' or (a.checkpoint and r['status']=='running_pairs');assert all(h(k)==v for k,v in r['input_hashes'].items());assert r['required_pairs']==['200301.jpg','200701.jpg','200801.jpg']
if r['status']=='complete' or not a.checkpoint:assert len(r['results'])==3
assert len(r['results'])<=3
pinned=lambda name:next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
m=json.loads(pinned('prepared.json').read_text());baseline=pinned('dedup-fresh-union-original-full-strict-checkpoint-12-screen-run.json');ba=baseline.with_name(baseline.stem+'-audit.json');assert json.loads(ba.read_text())['input_hashes'][str(baseline)]==h(baseline);control=next(x['evidence'] for x in json.loads(baseline.read_text())['results'] if x['query']=='200301.jpg')
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');src=helper.read_text();nodes={v.name:v for v in ast.parse(src).body if isinstance(v,ast.FunctionDef)};ns={'math':math};exec(compile(ast.Module(body=[nodes['verify_model']],type_ignores=[]),str(helper),'exec'),ns);rs=ast.get_source_segment(src,nodes['verify_regions']);assert 'len(regions)<=32' in rs;exec(compile(rs.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
summary=[]
for i,row in enumerate(r['results']):
 assert row['query']==r['required_pairs'][i] and type(row['returncode']) is int
 if row['returncode']:
  assert row['stderr'];summary.append({'query':row['query'],'outcome':'native_error','returncode':row['returncode']});continue
 expected=next(x for x in json.loads(baseline.read_text())['results'] if x['query']==row['query']);assert row['original']==expected['original']
 e=row['evidence'];assert e==json.loads(row['stdout']) and e['status']=='ok';dims=[]
 for key,split in [('original','original'),('query','strong')]:
  v=next(x for x in m['images'][split] if x['filename']==row[key]);assert h(pinned(row[key]))==v['source_sha256'];dims.append(oriented_dimensions(pinned(row[key]),v['source_size']))
 for k in ['managed_used','managed_peak','retained_before_drop','region_support_count']:assert type(e[k]) is int and e[k]>=0
 assert e['managed_used']==0 and e['retained_before_drop']<=e['managed_peak']<=512*1024*1024
 points=e['correspondences'];assert len(points)<=28000
 for j,point in enumerate(points):
  assert len(point)==2
  for xy,(w,t) in zip(point,dims):assert len(xy)==2 and all(type(v) in [int,float] and math.isfinite(v) for v in xy) and 0<=xy[0]<w and 0<=xy[1]<t
  for prior in points[:j]:assert all(math.dist(x,y)>2 for x,y in zip(point,prior))
 if e['matrix'] is None:assert not e['inliers'] and not e['regions'] and not e['region_support_count']
 else:assert len(e['inliers'])>=10;ns['verify_model'](e['matrix'],e['inliers'],points);ns['verify_regions'](e,row['query'])
 for region in e['regions']:
  for (x,y,w,t),(dw,dh) in zip(region['domains'],dims):assert x+w<=dw and y+t<=dh
 if i==0:assert {k:v for k,v in e.items() if k!='managed_peak'}=={k:v for k,v in control.items() if k!='managed_peak'}
 summary.append({'query':row['query'],'outcome':'local_supported' if e['region_support_count'] else 'miss','matches':len(points),'inliers':len(e['inliers']),'regions':e['region_support_count'],'managed_peak':e['managed_peak']})
assert h(a.report)==rh and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_repeat' if r['status']=='complete' else 'verified_repeat_prefix','summary':summary,'input_hashes':{str(a.report):rh,str(Path(__file__)):h(__file__),str(helper):h(helper),str(ba):h(ba),'scripts/dedup_jpeg_domain.py':h('scripts/dedup_jpeg_domain.py')},'scope':'Three explicit repeats; original input pins, canonical positive parity, model/region arithmetic, oriented bounds and managed memory. Errors retained. No whole identity, full229 recall, full-RSS or independent pixel-resampling proof.'},indent=2)+'\n')
