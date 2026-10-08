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
e=r['evidence'];assert json.loads(r['stdout'])==e and e['status']=='ok' and len(e['directions'])==2;assert type(e['managed_used']) is int and e['managed_used']==0 and type(e['managed_peak']) is int and 0<=e['managed_peak']<=512*1024*1024;summary=[]
for row,label,domain,name in zip(e['directions'],['forward','reverse'],[r['forward_roi'],r['reverse_roi']],['200501.jpg','200500.jpg']):
 assert row['direction']==label and row['domain']==domain;x,y,w,hh=domain;assert x+w<=dimensions[name][0] and y+hh<=dimensions[name][1];sites=w*hh;assert sites<=20000
 for key in ['training_samples','heldout_samples','pixel_reads']:assert type(row[key]) is int and row[key]>=0
 assert row['training_samples']+row['heldout_samples']<=sites and row['pixel_reads']<=sites*289*5<=32000000
 ev=row['evidence']
 if 'fit_error' in ev or 'validation_error' in ev:
  key='fit_error' if 'fit_error' in ev else 'validation_error';assert ev[key] in ['Invalid','Uninformative','IllConditioned','Bounds','Budget','Cancelled'] and all(k not in ev for k in ['matrix','samples','matched']);summary.append({'direction':label,'outcome':'refusal','error':ev[key]});continue
 assert len(ev['matrix'])==3 and all(len(rr)==3 and all(type(v) in [int,float] and math.isfinite(v) and abs(v)<=5 for v in rr) for rr in ev['matrix']);assert len(ev['offset'])==3 and all(type(v) in [int,float] and math.isfinite(v) and abs(v)<=.1 for v in ev['offset'])
 assert type(ev['samples']) is int and ev['samples']==row['heldout_samples']>=1000 and row['training_samples']>=1000;assert type(ev['matched']) is int and 0<=ev['matched']<=ev['samples'];assert type(ev['squared_error']) in [float,int] and math.isfinite(ev['squared_error']) and ev['squared_error']>=0
 summary.append({'direction':label,'matched':ev['matched'],'samples':ev['samples'],'heldout_fraction':ev['matched']/ev['samples'],'passes_fraction_0_9':ev['matched']>=.9*ev['samples'],'coverage':(row['training_samples']+row['heldout_samples'])/sites})
assert all(h(k)==v for k,v in pins.items()) and all(h(k)==v for k,v in r['input_hashes'].items());a.output.write_text(json.dumps({'status':'verified_native_color_diagnostic_counts','input_hashes':pins,'summary':summary,'scope':'Typed two-direction fit/refusal, original image/Exif and model pins, counts/coverage/work and coefficient bounds. Whole-image filtering at ROI centers; opposite-domain clipping not imposed. No independent fitted-sample resampling, copy acceptance or negative precision proof.'},indent=2)+'\n')
