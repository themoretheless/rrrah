"""Independent target-grid geometry, membership and typed-result audit."""
import argparse,json,hashlib,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
from dedup_distinct_points import verify_distinct_points
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(a.report.read_text());assert r['status']=='terminal' and r['returncode']==0 and all(h(k)==v for k,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
m=json.loads(pinned('prepared.json').read_text());dims=[]
for name,split in [('200500.jpg','original'),('200501.jpg','strong')]:
 v=next(v for v in m['images'][split] if v['filename']==name);assert h(pinned(name))==v['source_sha256'];dims.append(oriented_dimensions(pinned(name),v['source_size']))
raw=r['evidence'];assert raw==json.loads(r['stdout']);matrix=raw['matrix'];assert len(matrix)==3 and all(len(row)==3 and all(math.isfinite(v) for v in row) for row in matrix);aa,bb,cc=matrix[0];dd,ee,ff=matrix[1];gg,hh,ii=matrix[2];adj=[[ee*ii-ff*hh,cc*hh-bb*ii,bb*ff-cc*ee],[ff*gg-dd*ii,aa*ii-cc*gg,cc*dd-aa*ff],[dd*hh-ee*gg,bb*gg-aa*hh,aa*ee-bb*dd]];det=aa*adj[0][0]+bb*adj[1][0]+cc*adj[2][0];assert det!=0;inverse=[[v/det for v in row] for row in adj]
def project(m,x,y):
 d=m[2][0]*x+m[2][1]*y+m[2][2];assert math.isfinite(d) and d!=0;return [(m[i][0]*x+m[i][1]*y+m[i][2])/d for i in range(2)]
points=raw['detail']['correspondences'];ids=raw['detail']['inlier_ids'];assert len(points)==raw['points']<=28000 and len(ids)==raw['inliers']>=10 and all(type(i) is int for i in ids) and ids==sorted(set(ids))
expected_ids=[]
for index,point in enumerate(points):
 for key,size in [('source',dims[0]),('target',dims[1])]:assert len(point[key])==2 and all(type(v) in [int,float] and math.isfinite(v) and 0<=v<size[i] for i,v in enumerate(point[key]))
 predicted=project(matrix,*point['source']);residual=sum((predicted[i]-point['target'][i])**2 for i in range(2))
 if residual<=4:expected_ids.append(index)
assert ids==expected_ids
verify_distinct_points([[point['source'],point['target']] for point in points])
e={'status':raw['status'],'managed_used':raw['managed_used'],'managed_peak':raw['managed_peak'],'regions':raw['detail']['regions']};assert e['status']=='ok' and e['managed_used']==0 and 0<=e['managed_peak']<=512*1024*1024;expected=[];sw,sh=dims[0];tw,th=dims[1]
for gy in range(8):
 for gx in range(8):
  x,y=gx*tw//8,gy*th//8;w,ht=(gx+1)*tw//8-x,(gy+1)*th//8-y;corners=[project(inverse,xx,yy) for xx,yy in [(x,y),(x+w-1,y),(x,y+ht-1),(x+w-1,y+ht-1)]];lo=[max(0,min(dims[0][i],math.floor(min(v[i] for v in corners)))) for i in range(2)];hi=[max(0,min(dims[0][i],math.ceil(max(v[i] for v in corners))+1)) for i in range(2)]
  if hi[0]>lo[0] and hi[1]>lo[1]:expected.append([[*lo,hi[0]-lo[0],hi[1]-lo[1]],[x,y,w,ht]])
assert len(e['regions'])==len(expected);supports=refusals=0
for row,domains in zip(e['regions'],expected):
 assert row['domains']==domains;sx,sy,w,ht=domains[0];cx,cy=sx+w/2,sy+ht/2;q=project(matrix,cx,cy);den=matrix[2][0]*cx+matrix[2][1]*cy+matrix[2][2];j=[[(matrix[i][k]-q[i]*matrix[2][k])/den for k in range(2)] for i in range(2)];radius=math.ceil(8/math.sqrt(abs(j[0][0]*j[1][1]-j[0][1]*j[1][0])));assert row['source_radius']==radius and 1<=radius<=16;passes=[]
 for d,domain,opposite,mapping,size,other,rad in zip(row['directions'],[domains[1],domains[0]],[domains[0],domains[1]],[inverse,matrix],[dims[1],dims[0]],[dims[0],dims[1]],[8,radius]):
  if 'error' in d:
   assert d['error'] in ['Color(Bounds)','Color(Uninformative)','Color(IllConditioned)'];refusals+=1;passes.append(False);continue
  assert len(d['matrix'])==3 and all(len(v)==3 and all(math.isfinite(x) and abs(x)<=5 for x in v) for v in d['matrix']) and len(d['offset'])==3 and all(math.isfinite(v) and abs(v)<=.1 for v in d['offset']) and math.isfinite(d['squared_error']) and d['squared_error']>=0
  x,y,w,ht=domain;ox,oy,ow,oh=opposite;counts=[0,0]
  for py in range(y,y+ht):
   for px in range(x,x+w):
    cx,cy=project(mapping,px,py)
    if not (ox<=cx<ox+ow and oy<=cy<oy+oh):continue
    if px<rad or py<rad or px+rad>=size[0] or py+rad>=size[1]:continue
    corners=[project(mapping,px+dx,py+dy) for dx,dy in [(-rad,-rad),(-rad,rad),(rad,-rad),(rad,rad)]]
    if any(not (0<=v[0] and v[0]+1<other[0] and 0<=v[1] and v[1]+1<other[1]) for v in corners):continue
    counts[((px-x)//8+(py-y)//8)%2]+=1
  assert d['training_samples']==counts[0] and d['samples']==counts[1]>=1000;assert type(d['matched']) is int and 0<=d['matched']<=d['samples'];assert 0<=d['pixel_reads']<=w*ht*(2*rad+1)**2*5;passes.append(d['matched']>=.9*d['samples'])
 supports+=all(passes)
assert supports==raw['supports'] and len(expected)==raw['regions'];assert all(h(k)==v for k,v in r['input_hashes'].items());a.output.write_text(json.dumps({'status':'verified_native_search_points_regions_counts','report_sha256':h(a.report),'verifier_sha256':h(__file__),'distinct_points_helper_sha256':h(Path(__file__).with_name('dedup_distinct_points.py')),'points':len(points),'inliers':len(ids),'regions':len(expected),'supports':supports,'direction_refusals':refusals,'scope':'Independent all-point residual membership at tolerance2, source/target distinct distances greater than2, grid bounds, radii, center membership, heldout counts and color parameter bounds. No independent pixel resampling, color-fit oracle or broad precision proof.'},indent=2)+'\n')
