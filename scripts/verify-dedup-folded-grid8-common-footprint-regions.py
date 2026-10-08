"""Independent fixed native-model ROI geometry/count audit; no pixel oracle."""
import argparse,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists();digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(a.report.read_text());assert r['status']=='complete_native_experiment' and r['required_cases']==len(r['rows'])==6 and all(digest(k)==v for k,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
reference=json.loads(pinned('dedup-folded-201702-grid8-geometry.json').read_text());models=[v for v in reference['results'][0]['evidence']['regions'] if v['matrix'] is not None];assert len(models)==6
manifest=json.loads(pinned('prepared.json').read_text());dims=[]
for name,split in [('201700.jpg','original'),('201702.jpg','strong')]:
 item=next(v for v in manifest['images'][split] if v['filename']==name);assert digest(pinned(name))==item['source_sha256'];dims.append(oriented_dimensions(pinned(name),item['source_size']))
def project(h,x,y):
 den=h[2][0]*x+h[2][1]*y+h[2][2];assert den!=0 and math.isfinite(den);return [(h[i][0]*x+h[i][1]*y+h[i][2])/den for i in range(2)]
summaries=[]
for index,(row,model) in enumerate(zip(r['rows'],models)):
 assert row['model_index']==index and row['cell']==[0,0] and row['matrix']==model['matrix'] and row['fit_target_domain']==model['target_domain'] and len(model['inliers'])>=10
 h=row['matrix'];aa,bb,cc=h[0];dd,ee,ff=h[1];gg,hh,ii=h[2];adj=[[ee*ii-ff*hh,cc*hh-bb*ii,bb*ff-cc*ee],[ff*gg-dd*ii,aa*ii-cc*gg,cc*dd-aa*ff],[dd*hh-ee*gg,bb*gg-aa*hh,aa*ee-bb*dd]];det=aa*adj[0][0]+bb*adj[1][0]+cc*adj[2][0];assert det!=0;inverse=[[v/det for v in axis] for axis in adj]
 target=list(map(int,model['target_domain']));x,y,w,ht=target;corners=[project(inverse,xx,yy) for xx,yy in [(x,y),(x+w-1,y),(x,y+ht-1),(x+w-1,y+ht-1)]];lo=[max(0,min(dims[0][i],math.floor(min(v[i] for v in corners)))) for i in range(2)];hi=[max(0,min(dims[0][i],math.ceil(max(v[i] for v in corners))+1)) for i in range(2)];source=[*lo,hi[0]-lo[0],hi[1]-lo[1]];assert row['domains']==[source,target]
 path=Path(row['input']);assert digest(path)==row['input_sha256'];assert list(map(float,path.read_text().split()))==[v for axis in h for v in axis]+source+target
 sx,sy,sw,sh=source;cx,cy=sx+sw/2,sy+sh/2;q=project(h,cx,cy);den=h[2][0]*cx+h[2][1]*cy+h[2][2];j=[[(h[i][k]-q[i]*h[2][k])/den for k in range(2)] for i in range(2)];radius=8
 assert row['returncode']==0;e=row['evidence'];assert e['footprints']==['target','source'] and e==json.loads(row['stdout']) and e['managed_used']==0 and 0<=e['managed_peak']<=512*1024*1024
 if e['status']=='ok':
  assert len(e['directions'])==2
  for direction_index,(direction,domain,opposite,mapping,size,other,rad) in enumerate(zip(e['directions'],[target,source],[source,target],[inverse,h],[dims[1],dims[0]],[dims[0],dims[1]],[8,8])):
   counts=[0,0];x,y,w,ht=domain;ox,oy,ow,oh=opposite
   for py in range(y,y+ht):
    for px in range(x,x+w):
     mx,my=project(mapping,px,py)
     if not (ox<=mx<ox+ow and oy<=my<oy+oh):continue
     if direction_index==0:
      if px<rad or py<rad or px+rad>=size[0] or py+rad>=size[1]:continue
      taps=[project(mapping,px+dx,py+dy) for dx,dy in [(-rad,-rad),(-rad,rad),(rad,-rad),(rad,rad)]]
      if any(not (0<=v[0] and v[0]+1<other[0] and 0<=v[1] and v[1]+1<other[1]) for v in taps):continue
     else:
      source_taps=[[mx+dx,my+dy] for dx,dy in [(-rad,-rad),(-rad,rad),(rad,-rad),(rad,rad)]]
      if any(not (0<=v[0] and v[0]+1<other[0] and 0<=v[1] and v[1]+1<other[1]) for v in source_taps):continue
      denominators=[inverse[2][0]*v[0]+inverse[2][1]*v[1]+inverse[2][2] for v in source_taps]
      assert min(denominators)>0 or max(denominators)<0,'Projective horizon crosses source footprint; corner oracle not applicable.'
      taps=[project(inverse,*v) for v in source_taps]
      if any(not (0<=v[0] and v[0]+1<size[0] and 0<=v[1] and v[1]+1<size[1]) for v in taps):continue
     counts[((px-x)//8+(py-y)//8)%2]+=1
   assert direction['radius']==rad and direction['training_samples']==counts[0]>=1000 and direction['samples']==counts[1]>=1000 and 0<=direction['matched']<=direction['samples'];assert direction['pixel_reads']<=w*ht*(2*rad+1)**2*(5 if direction_index==0 else 8)
  assert row['supported']==all(v['matched']>=.9*v['samples'] for v in e['directions'])
 else:
  assert e['status'] in ['refusal:Region(Color(Bounds))','refusal:Region(Color(Uninformative))','refusal:Region(Color(IllConditioned))','refusal:Budget'] and e['directions']==[] and row['supported'] is False
 summaries.append({'model_index':index,'status':e['status'],'supported':row['supported'],'kernel_radii':[8,8]})
assert all(digest(k)==v for k,v in r['input_hashes'].items());a.output.write_text(json.dumps({'status':'verified_folded_grid8_common_footprint_regions','report_sha256':digest(a.report),'verifier_sha256':digest(__file__),'cases':6,'supports':sum(v['supported'] for v in summaries),'summary':summaries,'scope':'Exact model/domain/input membership, reconstructed mapped bounds/radii and successful sample counts in explicit common query axes. No pixel resampling, color fit oracle or broad copy precision.'},indent=2)+'\n')
