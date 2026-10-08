"""Audit nine supplied-model pixel experiments, including exact zero-offset parity."""
import argparse,ast,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={str(a.report):h(a.report),str(Path(__file__)):h(__file__)}
r=json.loads(a.report.read_text());assert r['status']=='complete_native_experiment' and type(r['returncode']) is int and r['returncode']==0
assert all(h(p)==v for p,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
baseline_path=pinned('dedup-screen-filter8.json');baseline=json.loads(baseline_path.read_text());audit_path=Path('docs/research/dedup-screen-filter8-audit.json');audit=json.loads(audit_path.read_text());assert audit['status']=='verified_native_linear_filter8_screen_experiment' and audit['input_hashes'][str(baseline_path)]==h(baseline_path)
assert all(h(p)==v for doc in [baseline,audit] for p,v in doc['input_hashes'].items());pins[str(audit_path)]=h(audit_path)
b=baseline['evidence'];matrix=b['matrix'];coefficients=[float(v) for v in pinned('dedup-screen-translation-input.txt').read_text().split()];assert coefficients==[v for row in matrix for v in row]
m=json.loads(pinned('prepared.json').read_text());dimensions=[]
for name,split in [('200500.jpg','original'),('200501.jpg','strong')]:
 row=next(v for v in m['images'][split] if v['filename']==name);assert row['group_id']==200500 and h(pinned(name))==row['source_sha256'];dimensions.append(oriented_dimensions(pinned(name),row['source_size']))
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');pins[str(helper)]=h(helper);nodes={v.name:v for v in ast.parse(helper.read_text()).body if isinstance(v,ast.FunctionDef)};s=ast.get_source_segment(helper.read_text(),nodes['verify_regions']);assert 'len(regions)<=32' in s;ns={'math':math};exec(compile(s.replace('len(regions)<=32','len(regions)<=128'),str(helper),'exec'),ns)
e=r['evidence'];canonical=lambda v:json.dumps(v,sort_keys=True,allow_nan=False);assert e['status']=='ok' and canonical(json.loads(r['stdout']))==canonical(e)
assert type(e['managed_used']) is int and e['managed_used']==0 and type(e['managed_peak']) is int and 0<=e['managed_peak']<=512*1024*1024
assert len(e['experiments'])==9;summary=[]
for row,(dx,dy) in zip(e['experiments'],[(dx,dy) for dy in [-1.,0.,1.] for dx in [-1.,0.,1.]]):
 assert row['offset']==[dx,dy] and all(type(v) in [int,float] and math.isfinite(v) for v in row['offset'])
 expected=[list(v) for v in matrix]
 for c in range(3):expected[0][c]+=dx*matrix[2][c];expected[1][c]+=dy*matrix[2][c]
 assert canonical(row['matrix'])==canonical(expected)
 assert type(row['region_support_count']) is int and 0<=row['region_support_count']<=128;ns['verify_regions'](row,'200501.jpg')
 fractions=[]
 for region in row['regions']:
  for (x,y,w,height),(limit_w,limit_h) in zip(region['domains'],dimensions):assert x+w<=limit_w and y+height<=limit_h
  if region['pixels'] is not None:fractions.append(min(matched/compared if compared else 0 for matched,compared,total in region['pixels']['counts']))
 if dx==dy==0:assert canonical(row['regions'])==canonical(b['regions']) and row['region_support_count']==b['region_support_count']
 count=0
 for point in b['correspondences']:
  x,y=point[0];tx,ty=point[1];z=sum(expected[2][i]*v for i,v in enumerate([x,y,1.]));assert z!=0
  u=sum(expected[0][i]*v for i,v in enumerate([x,y,1.]))/z;v=sum(expected[1][i]*v for i,v in enumerate([x,y,1.]))/z
  count+=(u-tx)**2+(v-ty)**2<=4.0000001
 summary.append({'offset':[dx,dy],'support_count':row['region_support_count'],'best_bidirectional_fraction':max(fractions,default=0),'reported_points_within_geometry_tolerance':count})
assert all(h(p)==v for p,v in pins.items()) and all(h(p)==v for p,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_native_screen_translation_pixel_grid','verified_experiments':9,'summary':summary,'input_hashes':pins,'scope':'Ordered nine offsets, exact matrix composition, zero-offset pixel parity, source/orientation pins and unchanged regional acceptance arithmetic. Translated models are supplied, not refit or independently full-domain qualified; no default, negative precision or full pixel-resampling oracle claim.'},indent=2)+'\n')
