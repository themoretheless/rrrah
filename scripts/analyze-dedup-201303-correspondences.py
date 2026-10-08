"""Spatial diagnostic of pinned native correspondences; not acceptance evidence."""
import hashlib,json,math
from dedup_jpeg_domain import oriented_dimensions
from pathlib import Path
report=Path('docs/research/dedup-updated-additional-failures-checkpoint-1.json')
audit=Path('docs/research/dedup-updated-additional-failures-checkpoint-1-audit.json')
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
a=json.loads(audit.read_text());assert a['input_hashes'][str(report)]==h(report)
r=next(x for x in json.loads(report.read_text())['results'] if x['query']=='201303.jpg')
e=r['evidence'];assert json.loads(r['stdout'])==e and r['returncode']==0
points=e['correspondences']
raw=json.loads(report.read_text());assert all(h(Path(k))==v for k,v in raw['input_hashes'].items())
prepared=next(Path(k) for k in raw['input_hashes'] if Path(k).name=='prepared.json');metadata=json.loads(prepared.read_text());dims=[]
for name,split in [(r['original'],'original'),(r['query'],'strong')]:
 row=next(v for v in metadata['images'][split] if v['filename']==name);path=next(Path(k) for k in raw['input_hashes'] if Path(k).name==name);assert h(path)==row['source_sha256'];dims.append(oriented_dimensions(path,row['source_size']))
spans=[]
for side,(w,t) in enumerate(dims):
 xs=[p[side][0] for p in points];ys=[p[side][1] for p in points]
 spans.append({'x_range':[min(xs),max(xs)],'y_range':[min(ys),max(ys)],'normalized_x_span':(max(xs)-min(xs))/w,'normalized_y_span':(max(ys)-min(ys))/t})
nearest=[]
for i,p in enumerate(points):
 j=min((j for j in range(len(points)) if j!=i),key=lambda j:math.dist(p[0],points[j][0]))
 nearest.append({'source_index':i,'nearest_source_index':j,'source_distance_pixels':math.dist(p[0],points[j][0]),'target_distance_pixels':math.dist(p[1],points[j][1])})
output={'status':'verified_spatial_diagnostic','query':r['query'],'correspondence_count':len(points),'source_target_spans':spans,'nearest_source_neighbors':nearest,'input_hashes':{str(report):h(report),str(audit):h(audit),str(Path(__file__)):h(Path(__file__))},'interpretation':'Spatial statistics describe proposed matches, with no correctness oracle. Use to investigate correspondence quality and model support; bounding-box span alone does not prove correct registration. No true match labels, fold model, acceptance relaxation or recovery claim.'}
Path('docs/research/dedup-201303-correspondence-spatial.json').write_text(json.dumps(output,indent=2)+'\n')
print(json.dumps({'count':len(points),'spans':spans,'closest_pair':min(nearest,key=lambda x:x['source_distance_pixels'])},indent=2))
