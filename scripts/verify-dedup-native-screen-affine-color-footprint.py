"""Typed native color diagnostic audit; no independent resampling oracle."""
import argparse,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(a.report.read_text());pins={str(a.report):h(a.report),str(Path(__file__)):h(__file__)};assert r['status']=='complete_native_color_diagnostic' and type(r['returncode']) is int and r['returncode']==0 and r['required_directions']==2 and all(h(k)==v for k,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
m=json.loads(pinned('prepared.json').read_text());dimensions={}
for name,split in [('200500.jpg','original'),('200501.jpg','strong')]:
 v=next(v for v in m['images'][split] if v['filename']==name);assert h(pinned(name))==v['source_sha256'];dimensions[name]=oriented_dimensions(pinned(name),v['source_size'])
baseline=json.loads(pinned('dedup-screen-filter8.json').read_text());assert all(h(k)==v for k,v in baseline['input_hashes'].items());assert [float(v) for v in pinned('dedup-screen-global-translation-anchor.txt').read_text().split()]==[v for row in baseline['evidence']['matrix'] for v in row]
matrix=baseline['evidence']['matrix']
aa,bb,cc=matrix[0];dd,ee,ff=matrix[1];gg,hh,ii=matrix[2]
adj=[[ee*ii-ff*hh,cc*hh-bb*ii,bb*ff-cc*ee],[ff*gg-dd*ii,aa*ii-cc*gg,cc*dd-aa*ff],[dd*hh-ee*gg,bb*gg-aa*hh,aa*ee-bb*dd]]
det=aa*adj[0][0]+bb*adj[1][0]+cc*adj[2][0];assert det!=0
inverse=[[v/det for v in row] for row in adj]
def project(m,x,y):
 den=m[2][0]*x+m[2][1]*y+m[2][2];assert math.isfinite(den) and abs(den)>1e-12
 return [(m[j][0]*x+m[j][1]*y+m[j][2])/den for j in range(2)]
cx=r['reverse_roi'][0]+r['reverse_roi'][2]/2;cy=r['reverse_roi'][1]+r['reverse_roi'][3]/2
q=project(matrix,cx,cy);den=matrix[2][0]*cx+matrix[2][1]*cy+matrix[2][2]
j=[[(matrix[i][k]-q[i]*matrix[2][k])/den for k in range(2)] for i in range(2)]
scale=math.sqrt(abs(j[0][0]*j[1][1]-j[0][1]*j[1][0]));reverse_radius=math.ceil(8/scale)
assert r['forward_filter_radius']==8 and r['reverse_filter_radius']==reverse_radius and 1<=reverse_radius<=16 and r['maximum_pixel_reads']==64000000
assert r['tolerance']==.03 and r['minimum_variance']==1e-5 and r['maximum_coefficient']==5 and r['maximum_offset']==.1
e=r['evidence'];assert json.loads(r['stdout'])==e and e['status']=='ok' and len(e['directions'])==2;assert type(e['managed_used']) is int and e['managed_used']==0 and type(e['managed_peak']) is int and 0<=e['managed_peak']<=512*1024*1024;summary=[]
for row,label,domain,name in zip(e['directions'],['forward','reverse'],[r['forward_roi'],r['reverse_roi']],['200501.jpg','200500.jpg']):
 radius=8 if label=='forward' else reverse_radius;taps=(2*radius+1)**2
 assert row['filter_radius']==radius
 assert row['direction']==label and row['domain']==domain;x,y,w,hh=domain;assert x+w<=dimensions[name][0] and y+hh<=dimensions[name][1];sites=w*hh;assert sites<=20000
 for key in ['training_samples','heldout_samples','pixel_reads']:assert type(row[key]) is int and row[key]>=0
 assert row['training_samples']+row['heldout_samples']<=sites and row['pixel_reads']<=sites*taps*5<=64000000
 opposite=r['reverse_roi'] if label=='forward' else r['forward_roi'];mapping=inverse if label=='forward' else matrix;sw,sh=dimensions['200500.jpg' if label=='forward' else '200501.jpg'];counts=[0,0]
 for py in range(y,y+hh):
  for px in range(x,x+w):
   cx,cy=project(mapping,px,py);ox,oy,ow,oh=opposite
   if not (ox<=cx<ox+ow and oy<=cy<oy+oh):continue
   assert px>=radius and py>=radius and px+radius<dimensions[name][0] and py+radius<dimensions[name][1]
   for dx,dy in [(-radius,-radius),(-radius,radius),(radius,-radius),(radius,radius)]:
    mx,my=project(mapping,px+dx,py+dy);assert 0<=mx and mx+1<sw and 0<=my and my+1<sh
   counts[((px-x)//8+(py-y)//8)%2]+=1
 assert row['training_samples']==counts[0] and row['heldout_samples']==counts[1] and row['pixel_reads']==sum(counts)*taps*5
 ev=row['evidence']
 if 'fit_error' in ev or 'validation_error' in ev:
  key='fit_error' if 'fit_error' in ev else 'validation_error';assert ev[key] in ['Invalid','Uninformative','IllConditioned','Bounds','Budget','Cancelled'] and all(k not in ev for k in ['matrix','samples','matched']);summary.append({'direction':label,'outcome':'refusal','error':ev[key]});continue
 assert len(ev['matrix'])==3 and all(len(rr)==3 and all(type(v) in [int,float] and math.isfinite(v) and abs(v)<=5 for v in rr) for rr in ev['matrix']);assert len(ev['offset'])==3 and all(type(v) in [int,float] and math.isfinite(v) and abs(v)<=.1 for v in ev['offset'])
 assert type(ev['samples']) is int and ev['samples']==row['heldout_samples']>=1000 and row['training_samples']>=1000;assert type(ev['matched']) is int and 0<=ev['matched']<=ev['samples'];assert type(ev['squared_error']) in [float,int] and math.isfinite(ev['squared_error']) and ev['squared_error']>=0
 summary.append({'direction':label,'matched':ev['matched'],'samples':ev['samples'],'heldout_fraction':ev['matched']/ev['samples'],'passes_fraction_0_9':ev['matched']>=.9*ev['samples'],'coverage':(row['training_samples']+row['heldout_samples'])/sites})
assert all(h(k)==v for k,v in pins.items()) and all(h(k)==v for k,v in r['input_hashes'].items());a.output.write_text(json.dumps({'status':'verified_native_color_diagnostic_counts','input_hashes':pins,'summary':summary,'scope':'Typed two-direction fit/refusal, original image/Exif and model pins, counts/coverage/work and coefficient bounds. Independent opposite-ROI center membership and geometric filter-footprint bounds; whole-image filter support retained. No independent fitted-sample resampling, copy acceptance or negative precision proof.'},indent=2)+'\n')
