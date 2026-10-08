import json, hashlib, subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe=root/'gradient-scales-probe-correspondence-stages'
out=Path('docs/research/dedup-scale-correspondence-stages-three-crops.json')
assert not out.exists()
h=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
pins={str(f):h(f) for f in [exe,Path(__file__)]}
for stem in ['2001','2002','2003']:
 for kind,suffix in [('original','00'),('strong','01')]:
  f=root/'normalized'/kind/(stem+suffix+'.png');pins[str(f)]=h(f)
r={'status':'running','input_hashes':pins,'results':[],'scope':'Four versus seven scales; raw correspondences and fitted geometry expose refusal stages on three hard crops. No independent ground-truth transform.'}
def save():out.write_text(json.dumps(r,indent=2)+'\n')
save()
for stem,mode in [(stem,mode) for stem in ['2001','2002','2003'] for mode in ['spatial-regions','spatial-regions-dense']]:
 run=subprocess.run([str(exe),mode,str(root/'normalized/original'/(stem+'00.png')),str(root/'normalized/strong'/(stem+'01.png'))],capture_output=True,text=True)
 row={'mode':mode,'query_id':stem+'01.jpg','returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr};r['results'].append(row);save()
 if run.returncode:r['status']='native_failed';save();raise SystemExit(run.returncode)
 e=json.loads(run.stdout);assert e['status']=='ok' and e['managed_used']==0
 assert type(e['whole_candidate']) is bool and type(e['region_support_count']) is int
 assert e['region_support_count']==sum(x['accepted'] for x in e['regions'])
 row['evidence']=e
 assert all(h(f)==v for f,v in pins.items())
 save()
r['status']='complete';save()
