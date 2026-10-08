"""Typed coordinate/work/threshold audit of dense proposals; no acceptance oracle."""
import argparse,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={str(a.report):h(a.report),str(Path(__file__)):h(__file__)}
r=json.loads(a.report.read_text());assert r['status']=='complete_native_proposals' and type(r['returncode']) is int and r['returncode']==0 and all(h(k)==v for k,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
m=json.loads(pinned('prepared.json').read_text());dims=[]
for name,split in [('200300.jpg','original'),('200302.jpg','strong')]:
 row=next(v for v in m['images'][split] if v['filename']==name);assert row['group_id']==200300 and h(pinned(name))==row['source_sha256'];dims.append(oriented_dimensions(pinned(name),row['source_size']))
e=r['evidence'];assert e['status']=='ok' and json.loads(r['stdout'])==e
assert len(e['feature_counts'])==2 and all(type(v) is int and 0<=v<=20000 for v in e['feature_counts']) and math.prod(e['feature_counts'])<=400000000
assert type(e['radius_pass']) is int and len(e['matches'])<=e['radius_pass']<=e['feature_counts'][0]
for count,(width,height),stride in zip(e['feature_counts'],dims,[32,16]):
 sites=len(range(16,max(16,height-16),stride))*len(range(16,max(16,width-16),stride));assert count<=3*sites and 1024*sites<=16000000
for match in e['matches']:
 for key,(w,hh),stride in zip(['source','target'],dims,[32,16]):
  v=match[key];assert len(v)==2 and all(type(x) in [float,int] and math.isfinite(x) for x in v)
  assert 16<=v[0]<w-16 and 16<=v[1]<hh-16 and all((x-16)%stride==0 for x in v)
 distance=match['squared_distance'];second=match['second_distinct_squared_distance'];assert all(type(v) in [float,int] and math.isfinite(v) for v in [distance,second]);assert 0<=distance<=.5 and distance<=second and distance<.64*second
assert all(h(k)==v for k,v in pins.items()) and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_dense_patch_proposal_arithmetic','input_hashes':pins,'feature_counts':e['feature_counts'],'radius_pass':e['radius_pass'],'proposals':len(e['matches']),'source_unique_positions':len({tuple(v['source']) for v in e['matches']}),'target_unique_positions':len({tuple(v['target']) for v in e['matches']}),'scope':'Pinned original images, Exif dimensions, grid coordinates, finite descriptor distances and unchanged ratio/radius thresholds. Multiple scale proposals may share positions. No independent descriptor oracle, geometric or pixel proof, negatives or full allocation qualification.'},indent=2)+'\n')
