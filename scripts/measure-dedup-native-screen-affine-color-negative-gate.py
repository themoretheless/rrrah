"""All-original normalized fixed-ROI native negative gate; refusals/errors retained."""
import json,hashlib,subprocess,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=Path('docs/research');out=base/'dedup-native-screen-affine-color-negative-gate.json';assert not out.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();manifest=root/'prepared.json';m=json.loads(manifest.read_text());positive=root/'original-resolution-negative-200201/200500.jpg';right=root/'original-resolution-strong-all/200501.jpg';anchor=base/'dedup-screen-global-translation-anchor.txt';exe=root/'affine-color-roi-image-probe-screen-negative'
def dimensions(path,split):
 v=next(v for v in m['images'][split] if v['filename']==path.name);assert h(path)==v['source_sha256'];return oriented_dimensions(path,v['source_size'])
pw,ph=dimensions(positive,'original');dimensions(right,'strong');original=list(map(float,anchor.read_text().split()));cases=[v for v in m['images']['original'] if v['filename']!='200500.jpg'];assert len(cases)==156
pins={str(p):h(p) for p in [manifest,positive,right,anchor,exe,Path(__file__),Path('crates/rrrah-dedup/examples/affine_color_roi_image_probe.rs'),Path('crates/rrrah-dedup/src/affine_color.rs')]};report={'status':'running','required_cases':156,'input_hashes':pins,'rows':[],'scope':'All other originals against one screen query under fixed normalized ROI geometry. Both bounded color fits and both heldout fractions>=0.9 required. Not candidate retrieval, burst precision or independent resampling evidence.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(out)
save()
for case in cases:
 left=root/'original-resolution-negative-200201'/case['filename'];nw,nh=dimensions(left,'original');sx,sy=pw/nw,ph/nh
 model=[x*(sx if i%3==0 else sy if i%3==1 else 1) for i,x in enumerate(original)];roi=[math.ceil(2115/sx),math.ceil(922/sy),math.floor(103/sx),math.floor(133/sy)]
 input=root/('screen-color-negative-gate-'+left.stem+'.txt');assert not input.exists();input.write_text(' '.join(map(str,model+roi+[240,60,80,60]))+'\n');before=h(left)
 r=subprocess.run([str(exe),str(left),str(right),str(input)],capture_output=True,text=True)
 assert h(left)==before and all(h(k)==v for k,v in pins.items())
 row={'source':str(left),'source_sha256':before,'input':str(input),'input_sha256':h(input),'roi':roi,'returncode':r.returncode,'stdout':r.stdout,'stderr':r.stderr,'accepted':False}
 if r.returncode==0:
  e=json.loads(r.stdout);assert e['status']=='ok' and e['managed_used']==0 and len(e['directions'])==2;row['evidence']=e
  row['accepted']=all('matched' in d['evidence'] and d['evidence']['matched']>=.9*d['evidence']['samples'] for d in e['directions'])
 report['rows'].append(row);save();print(left.name,r.returncode,row['accepted'],flush=True)
report['status']='complete';report['accepted']=sum(v['accepted'] for v in report['rows']);report['native_errors']=sum(v['returncode']!=0 for v in report['rows']);save()
