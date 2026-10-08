"""Audit supplied local models, explicit refusals and pixels inside fitting domains."""
import argparse,ast,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={str(a.report):h(a.report),str(Path(__file__)):h(__file__)}
r=json.loads(a.report.read_text());assert r['status']=='complete_native_experiment' and type(r['returncode']) is int and r['returncode']==0 and all(h(p)==v for p,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
g_path=pinned('dedup-sift-lowcontrast-oracle-geometry.json');g=json.loads(g_path.read_text());ga_path=Path('docs/research/dedup-sift-lowcontrast-oracle-geometry-audit.json');pins[str(ga_path)]=h(ga_path);ga=json.loads(ga_path.read_text());assert ga['status']=='verified_external_sift_native_geometry_controls' and ga['input_hashes'][str(g_path)]==h(g_path) and all(h(p)==v for p,v in ga['input_hashes'].items())
assert all(h(p)==v for p,v in g['input_hashes'].items());screen=next(v for v in g['results'] if v['case']=='screen');models=[v for v in screen['evidence']['regions'] if v['matrix'] is not None];assert len(models)==1
coefficients=[[float(v) for v in line.split()] for line in pinned('dedup-screen-sift-lowcontrast-model-input.txt').read_text().splitlines()];assert coefficients==[v['target_domain']+[x for row in v['matrix'] for x in row] for v in models]
m=json.loads(pinned('prepared.json').read_text());dimensions=[]
for name,split in [('200500.jpg','original'),('200501.jpg','strong')]:
 image=next(v for v in m['images'][split] if v['filename']==name);assert image['group_id']==200500 and h(pinned(name))==image['source_sha256'];dimensions.append(oriented_dimensions(pinned(name),image['source_size']))
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');pins[str(helper)]=h(helper);node=next(v for v in ast.parse(helper.read_text()).body if isinstance(v,ast.FunctionDef) and v.name=='verify_regions');ns={'math':math};exec(compile(ast.Module(body=[node],type_ignores=[]),str(helper),'exec'),ns)
e=r['evidence'];canonical=lambda v:json.dumps(v,sort_keys=True,allow_nan=False);assert e['status']=='ok' and canonical(json.loads(r['stdout']))==canonical(e)
assert type(e['managed_used']) is int and e['managed_used']==0 and type(e['managed_peak']) is int and 0<=e['managed_peak']<=512*1024*1024 and len(e['experiments'])==1
summary=[]
for row,model in zip(e['experiments'],models):
 assert canonical(row['matrix'])==canonical(model['matrix']) and row['fit_target_domain']==model['target_domain']
 if 'error' in row:
  assert type(row['error']) is str and row['error'] and all(k not in row for k in ['regions','region_support_count','local_supported_count']);summary.append({'target_domain':row['fit_target_domain'],'outcome':'native_model_refusal','error':row['error']});continue
 for k in ['region_support_count','local_supported_count']:assert type(row[k]) is int and 0<=row[k]<=32
 ns['verify_regions'](row,'200501.jpg');domain=model['target_domain'];accepted=0;fractions=[]
 for region in row['regions']:
  for (x,y,w,height),(limit_w,limit_h) in zip(region['domains'],dimensions):assert x+w<=limit_w and y+height<=limit_h
  x,y,w,height=region['domains'][1];inside=x>=domain[0] and y>=domain[1] and x+w<=domain[0]+domain[2] and y+height<=domain[1]+domain[3]
  if inside:
   accepted+=region['accepted'];pixels=region['pixels']
   if pixels is not None and all(c>=1000 and c/t>=.3 for matched,c,t in pixels['counts']):fractions.append(min(matched/c for matched,c,t in pixels['counts']))
 assert accepted==row['local_supported_count']
 summary.append({'target_domain':domain,'outcome':'local_supported' if accepted else 'miss','local_support_count':accepted,'all_region_support_count':row['region_support_count'],'best_eligible_inside_fraction':max(fractions,default=None)})
assert all(h(p)==v for p,v in pins.items()) and all(h(p)==v for p,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_native_screen_local_model_pixel_experiments','verified_models':1,'summary':summary,'input_hashes':pins,'scope':'Exact one native SIFT-proposed full-domain model, input/source/Exif pins, regional original-pixel arithmetic and counts restricted to fitting target rectangles. Refusals retained. No independent resampling, true local geometry, negative precision or production integration proof.'},indent=2)+'\n')
