"""Spatial diagnostic of pinned native correspondences; not acceptance evidence."""
import hashlib,json,math
from pathlib import Path
report=Path('docs/research/dedup-fresh-union-original-full-strict-checkpoint-7.json')
audit=Path('docs/research/dedup-fresh-union-original-full-strict-checkpoint-7-audit.json')
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
a=json.loads(audit.read_text());assert a['input_hashes'][str(report)]==h(report)
r=next(x for x in json.loads(report.read_text())['results'] if x['query']=='200302.jpg')
e=r['evidence'];assert json.loads(r['stdout'])==e and r['returncode']==0
points=e['correspondences']; dims=((2048,1536),(800,600))
spans=[]
for side,(w,t) in enumerate(dims):
 xs=[p[side][0] for p in points];ys=[p[side][1] for p in points]
 spans.append({'x_range':[min(xs),max(xs)],'y_range':[min(ys),max(ys)],'normalized_x_span':(max(xs)-min(xs))/w,'normalized_y_span':(max(ys)-min(ys))/t})
nearest=[]
for i,p in enumerate(points):
 j=min((j for j in range(len(points)) if j!=i),key=lambda j:math.dist(p[0],points[j][0]))
 nearest.append({'source_index':i,'nearest_source_index':j,'source_distance_pixels':math.dist(p[0],points[j][0]),'target_distance_pixels':math.dist(p[1],points[j][1])})
output={'status':'verified_spatial_diagnostic','query':r['query'],'correspondence_count':len(points),'source_target_spans':spans,'nearest_source_neighbors':nearest,'input_hashes':{str(report):h(report),str(audit):h(audit),str(Path(__file__)):h(Path(__file__))},'interpretation':'Source matches occupy a narrow vertical band and several nearby source points map far apart in the target. Descriptor correspondence quality must be investigated before attributing failure solely to the global projective model. No true match labels, fold model, acceptance relaxation or recovery claim.'}
Path('docs/research/dedup-wrinkled-correspondence-spatial.json').write_text(json.dumps(output,indent=2)+'\n')
print(json.dumps({'count':len(points),'spans':spans,'closest_pair':min(nearest,key=lambda x:x['source_distance_pixels'])},indent=2))
