"""Audit external SIFT proposals and native geometric control evidence."""
import argparse,ast,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={str(a.report):h(a.report),str(Path(__file__)):h(__file__)};r=json.loads(a.report.read_text());assert r['status']=='complete_native_geometry_cases' and r['required_cases']==4 and len(r['results'])==4 and all(h(k)==v for k,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
o=json.loads(pinned('dedup-sift-oracle.json').read_text());assert o['status']=='complete_external_candidate_oracle' and len(o['results'])==4 and all(h(k)==v for k,v in o['input_hashes'].items());m=json.loads(pinned('prepared.json').read_text());helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');pins[str(helper)]=h(helper);node=next(v for v in ast.parse(helper.read_text()).body if isinstance(v,ast.FunctionDef) and v.name=='verify_model');ns={'math':math};exec(compile(ast.Module(body=[node],type_ignores=[]),str(helper),'exec'),ns);summary=[]
for source,row,name in zip(o['results'],r['results'],['exact','wrinkled','unrelated','screen']):
 assert source['case']==row['case']==name;assert type(row['returncode']) is int and row['returncode']==0;points=[[v['source'],v['target']] for v in source['distinct_matches']];dims=[]
 for key in ['left','right']:
  path=Path(source[key]);split='strong' if path.parent.name=='original-resolution-strong-all' else 'original';image=next(v for v in m['images'][split] if v['filename']==path.name);assert h(path)==image['source_sha256'];dims.append(oriented_dimensions(path,image['source_size']))
 assert list(dims[1])==row['target_dimensions'] and [[float(v) for v in line.split()] for line in Path(row['point_file']).read_text().splitlines()]==[s+t for s,t in points]
 for i,match in enumerate(source['distinct_matches']):
  d,second=match['distance'],match['second_distance'];assert all(type(v) in [float,int] and math.isfinite(v) for v in [d,second]) and 0<=d<.8*second
  for point,(w,hh) in zip(points[i],dims):assert len(point)==2 and all(type(v) in [float,int] and math.isfinite(v) for v in point) and 0<=point[0]<w and 0<=point[1]<hh
  for prior in points[:i]:assert all(math.dist(a,b)>2 for a,b in zip(points[i],prior))
 e=row['evidence'];assert json.loads(row['stdout'])==e and e['status']=='ok' and len(e['regions'])==1;region=e['regions'][0];assert region['target_domain']==[0,0,*dims[1]] and region['point_indices']==list(range(len(points)))
 if region['matrix'] is None:assert region['inliers']==[]
 else:assert len(region['inliers'])>=10;ns['verify_model'](region['matrix'],region['inliers'],points)
 if name=='exact':assert source['left']==source['right'] and len(region['inliers'])==len(points)>=10
 if name=='unrelated':assert region['matrix'] is None
 summary.append({'case':name,'distinct_proposals':len(points),'native_model':region['matrix'] is not None,'inliers':len(region['inliers'])})
assert all(h(k)==v for k,v in pins.items()) and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_external_sift_native_geometry_controls','input_hashes':pins,'verified_cases':4,'summary':summary,'scope':'Pinned raw sources, Exif domains, distinct locations, external ratio arithmetic, native matrices and residuals, exact and unrelated control decisions. No independent SIFT descriptor oracle, pixel acceptance or broad precision.'},indent=2)+'\n')
