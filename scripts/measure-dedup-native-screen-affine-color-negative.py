"""Native fixed normalized-ROI unrelated-origin control, not retrieval precision."""
import json,hashlib,subprocess,sys,math,shutil
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=Path('docs/research');manifest=root/'prepared.json';m=json.loads(manifest.read_text());h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
positive=root/'original-resolution-negative-200201/200500.jpg';left=root/'original-resolution-negative-200201/200000.jpg';right=root/'original-resolution-strong-all/200501.jpg'
def dim(path,split):
 v=next(v for v in m['images'][split] if v['filename']==path.name);assert h(path)==v['source_sha256'];return oriented_dimensions(path,v['source_size'])
pw,ph=dim(positive,'original');nw,nh=dim(left,'original');dim(right,'strong');sx,sy=pw/nw,ph/nh
anchor=base/'dedup-screen-global-translation-anchor.txt';v=list(map(float,anchor.read_text().split()));v=[x*(sx if i%3==0 else sy if i%3==1 else 1) for i,x in enumerate(v)]
roi=[math.ceil(2115/sx),math.ceil(922/sy),math.floor(103/sx),math.floor(133/sy)]
input=base/'dedup-native-screen-affine-color-negative-input.txt';assert not input.exists();input.write_text(' '.join(map(str,v+roi+[240,60,80,60]))+'\n')
exe=root/'affine-color-roi-image-probe-screen-negative';assert not exe.exists();shutil.copy2('target/debug/examples/affine_color_roi_image_probe',exe)
pins={str(p):h(p) for p in [manifest,positive,left,right,anchor,input,exe,Path(__file__),Path('crates/rrrah-dedup/examples/affine_color_roi_image_probe.rs'),Path('crates/rrrah-dedup/src/affine_color.rs')]}
r=subprocess.run([str(exe),str(left),str(right),str(input)],capture_output=True,text=True);assert all(h(k)==v for k,v in pins.items());out=base/'dedup-native-screen-affine-color-negative.json';assert not out.exists();result={'input_hashes':pins,'returncode':r.returncode,'stdout':r.stdout,'stderr':r.stderr,'source_roi':roi,'source_dimension_scale':[sx,sy],'scope':'One unrelated publisher original mapped to fixed screen ROI by normalized original dimensions; no discovered geometry, no all-query precision claim.'}
if r.returncode==0:result['evidence']=json.loads(r.stdout)
out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result.get('evidence',{})));sys.exit(r.returncode)
