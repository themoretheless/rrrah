"""Independent negative-grid membership, source/model and typed decision audit."""
import argparse,json,hashlib,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(a.report.read_text());assert r['status'] in ['running','complete'] and r['required_cases']==156 and all(h(k)==v for k,v in r['input_hashes'].items())
reference=next(Path(k) for k in r['input_hashes'] if Path(k).name=='dedup-native-screen-affine-color-negative-gate.json');ref=json.loads(reference.read_text());assert ref['status']=='complete' and len(ref['rows'])==156;prepared=next(Path(k) for k in ref['input_hashes'] if Path(k).name=='prepared.json');assert h(prepared)==ref['input_hashes'][str(prepared)];manifest=json.loads(prepared.read_text());original={v['filename']:v for v in manifest['images']['original']};right=next(Path(k) for k in r['input_hashes'] if Path(k).name=='200501.jpg');v=next(v for v in manifest['images']['strong'] if v['filename']==right.name);assert h(right)==v['source_sha256'];tw,th=oriented_dimensions(right,v['source_size']);supports=errors=refusals=0;seen=set();summaries=[]
def project(m,x,y):
 den=m[2][0]*x+m[2][1]*y+m[2][2];assert math.isfinite(den) and den!=0;return [(m[i][0]*x+m[i][1]*y+m[i][2])/den for i in range(2)]
for row,case in zip(r['rows'],ref['rows']):
 left=Path(row['source']);assert row['source']==case['source'] and left.name not in seen;seen.add(left.name);assert h(left)==row['source_sha256']==case['source_sha256']==original[left.name]['source_sha256'];input=Path(row['input']);assert h(input)==row['input_sha256'];old=Path(case['input']);assert h(old)==case['input_sha256'];values=list(map(float,input.read_text().split()));assert values==list(map(float,old.read_text().split()[:9]));matrix=[values[i:i+3] for i in range(0,9,3)];sw,sh=oriented_dimensions(left,original[left.name]['source_size'])
 if row['returncode']!=0:errors+=1;assert 'evidence' not in row;continue
 e=row['evidence'];assert e==json.loads(row['stdout']) and e['managed_used']==0 and 0<=e['managed_peak']<=512*1024*1024
 if e['status']!='ok':assert e['status'].startswith('refusal:') and not e['regions'] and 'supports' not in row;refusals+=1;continue
 aa,bb,cc=matrix[0];dd,ee,ff=matrix[1];gg,hh,ii=matrix[2]
 adj=[[ee*ii-ff*hh,cc*hh-bb*ii,bb*ff-cc*ee],[ff*gg-dd*ii,aa*ii-cc*gg,cc*dd-aa*ff],[dd*hh-ee*gg,bb*gg-aa*hh,aa*ee-bb*dd]]
 det=aa*adj[0][0]+bb*adj[1][0]+cc*adj[2][0];assert math.isfinite(det) and det!=0
 inverse=[[v/det for v in axis] for axis in adj];expected=[]
 for gy in range(8):
  for gx in range(8):
   x,y=gx*tw//8,gy*th//8;w,ht=(gx+1)*tw//8-x,(gy+1)*th//8-y
   corners=[project(inverse,xx,yy) for xx,yy in [(x,y),(x+w-1,y),(x,y+ht-1),(x+w-1,y+ht-1)]]
   lo=[max(0,min([sw,sh][i],math.floor(min(v[i] for v in corners)))) for i in range(2)]
   hi=[max(0,min([sw,sh][i],math.ceil(max(v[i] for v in corners))+1)) for i in range(2)]
   if hi[0]>lo[0] and hi[1]>lo[1]:expected.append([[*lo,hi[0]-lo[0],hi[1]-lo[1]],[x,y,w,ht]])
 assert [v['domains'] for v in e['regions']]==expected
 assert len(e['regions'])<=64;count=0;targets=set()
 for region in e['regions']:
  source,target=region['domains'];sx,sy,w,ht=source;x,y,ww,hh=target;assert 0<=sx<sw and 0<=sy<sh and 0<w<=sw-sx and 0<ht<=sh-sy;assert ww==tw//8 and hh==th//8 and x%ww==0 and y%hh==0 and x+ww<=tw and y+hh<=th and tuple(target) not in targets;targets.add(tuple(target));assert len(region['directions'])==2
  cx,cy=sx+w/2,sy+ht/2;q=project(matrix,cx,cy);den=matrix[2][0]*cx+matrix[2][1]*cy+matrix[2][2];j=[[(matrix[i][k]-q[i]*matrix[2][k])/den for k in range(2)] for i in range(2)];radius=math.ceil(8/math.sqrt(abs(j[0][0]*j[1][1]-j[0][1]*j[1][0])));assert region['source_radius']==radius and 1<=radius<=16;passes=[]
  for d,domain,rad in zip(region['directions'],[target,source],[8,radius]):
   if 'error' in d:assert d['error'] in ['Color(Bounds)','Color(Uninformative)','Color(IllConditioned)'];passes.append(False);continue
   for key in ['training_samples','samples','matched','pixel_reads']:assert type(d[key]) is int and d[key]>=0
   assert d['training_samples']>=1000 and d['samples']>=1000 and d['matched']<=d['samples'] and d['samples']+d['training_samples']<=domain[2]*domain[3];assert d['pixel_reads']<=domain[2]*domain[3]*(2*rad+1)**2*5;passes.append(d['matched']>=.9*d['samples'])
  count+=all(passes)
 assert count==row['supports'];supports+=count;summaries.append({'source':left.name,'regions':len(e['regions']),'supports':count})
assert len(r['rows'])<=156
if r['status']=='complete':assert len(r['rows'])==156 and seen==set(original)-{'200500.jpg'}
assert all(h(k)==v for k,v in r['input_hashes'].items());a.output.write_text(json.dumps({'status':'verified_terminal' if r['status']=='complete' else 'verified_prefix','report_sha256':h(a.report),'verifier_sha256':h(__file__),'cases':len(r['rows']),'region_supports':supports,'native_errors':errors,'grid_refusals':refusals,'summary':summaries,'scope':'Independent ordered case membership, source/model pins, exact reconstructed grid/source domains, bounded radii and typed decisions. Not pixel resampling, fit oracle or full retrieval/precision qualification.'},indent=2)+'\n')
