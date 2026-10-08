import json, hashlib, subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe=root/'gradient-scales-probe-original-resolution-diagnostic'
out=Path('docs/research/dedup-original-resolution-three-crops.json')
assert not out.exists()
h=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
pins={str(f):h(f) for f in [exe,Path(__file__),Path('docs/research/dedup-original-resolution-hard-crops.json')]}
for stem in ['2001','2002','2003']:
 for kind,suffix in [('original','00'),('strong','01')]:
  f=root/'original-resolution-hard-crops'/(stem+suffix+'.jpg');pins[str(f)]=h(f)
r={'recipes':{'four_scales':[1,2,4,8],'seven_scales':'2^(step/2), step=0..6','features_per_scale':2000,'grid_quota':128,'managed_limit':536870912,'decode_max_pixels':3200000,'pixel_max_source_pixels':8000000,'filter_max_sample_pairs':800000000},'status':'running','input_hashes':pins,'results':[],'scope':'Explicit distinct-location ratio, domain-qualified geometry, fresh guarded regional pixel confirmation. Original JPEG bytes; more scale range and explicit pixel/work/memory admission. Plain diagnostic matcher/geometry scratch not included in managed allocation accounting. Three hard positives only; no negative gate or default promotion.'}
def save():out.write_text(json.dumps(r,indent=2)+'\n')
save()
for stem in ['2001','2002','2003']:
 run=subprocess.run([str(exe),'scale-distinct-original',str(root/'original-resolution-hard-crops'/(stem+'00.jpg')),str(root/'original-resolution-hard-crops'/(stem+'01.jpg'))],capture_output=True,text=True)
 row={'query_id':stem+'01.jpg','returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr};r['results'].append(row);save()
 if run.returncode:r['status']='native_failed';save();raise SystemExit(run.returncode)
 e=json.loads(run.stdout);assert e['status']=='ok' and e['managed_used']==0
 assert [v['levels'] for v in e['stats']]==[4,7]
 row['evidence']=e
 assert all(h(f)==v for f,v in pins.items())
 save()
r['status']='complete';save()
