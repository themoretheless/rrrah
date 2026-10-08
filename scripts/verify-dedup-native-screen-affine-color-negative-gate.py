"""Independent membership, source pin, model construction and decision audit."""
import argparse,json,hashlib,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(a.report.read_text());assert all(h(k)==v for k,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
m=json.loads(pinned('prepared.json').read_text());original={v['filename']:v for v in m['images']['original']};positive=pinned('200500.jpg');pw,ph=oriented_dimensions(positive,original['200500.jpg']['source_size']);anchor=list(map(float,pinned('dedup-screen-global-translation-anchor.txt').read_text().split()));seen=set();accepted=errors=refusals=reverse_high=0
for row in r['rows']:
 left=Path(row['source']);assert left.name in original and left.name!='200500.jpg' and left.name not in seen;seen.add(left.name);assert h(left)==row['source_sha256']==original[left.name]['source_sha256'];inp=Path(row['input']);assert h(inp)==row['input_sha256'];nw,nh=oriented_dimensions(left,original[left.name]['source_size']);sx,sy=pw/nw,ph/nh
 roi=[math.ceil(2115/sx),math.ceil(922/sy),math.floor(103/sx),math.floor(133/sy)];expected=[v*(sx if i%3==0 else sy if i%3==1 else 1) for i,v in enumerate(anchor)]+roi+[240,60,80,60];assert list(map(float,inp.read_text().split()))==expected and row['roi']==roi
 assert type(row['returncode']) is int and type(row['accepted']) is bool
 decision=False
 if row['returncode']:
  errors+=1;assert 'evidence' not in row
 else:
  e=row['evidence'];assert json.loads(row['stdout'])==e and e['status']=='ok' and e['managed_used']==0 and len(e['directions'])==2;passes=[]
  for d,label,domain in zip(e['directions'],['forward','reverse'],[[240,60,80,60],roi]):
   assert d['direction']==label and d['domain']==domain and 1<=d['filter_radius']<=16 and 0<=d['pixel_reads']<=64000000;ev=d['evidence']
   if 'fit_error' in ev or 'validation_error' in ev:
    assert next(iter(ev.values())) in ['Invalid','Uninformative','IllConditioned','Bounds','Budget','Cancelled'];refusals+=1;passes.append(False);continue
   assert ev['samples']==d['heldout_samples']>=1000 and d['training_samples']>=1000 and type(ev['matched']) is int and 0<=ev['matched']<=ev['samples'];assert all(len(v)==3 and all(math.isfinite(x) and abs(x)<=5 for x in v) for v in ev['matrix']) and len(ev['matrix'])==3;assert len(ev['offset'])==3 and all(math.isfinite(x) and abs(x)<=.1 for x in ev['offset']);passes.append(ev['matched']>=.9*ev['samples'])
   if label=='reverse' and passes[-1]:reverse_high+=1
  decision=all(passes)
 assert row['accepted']==decision;accepted+=decision
if r['status']=='complete':assert seen==set(original)-{'200500.jpg'} and len(seen)==r['required_cases']==156 and r['accepted']==accepted and r['native_errors']==errors
assert all(h(k)==v for k,v in r['input_hashes'].items());a.output.write_text(json.dumps({'status':'verified_terminal' if r['status']=='complete' else 'verified_prefix','report_sha256':h(a.report),'verifier_sha256':h(__file__),'cases':len(seen),'accepted':accepted,'native_errors':errors,'direction_refusals':refusals,'reverse_high_fraction':reverse_high,'scope':'Independent membership/model construction and typed decision audit, no independent pixel resampling or all-query precision proof.'},indent=2)+'\n')
