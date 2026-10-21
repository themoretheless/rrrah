import subprocess,json,hashlib
from pathlib import Path
folder=Path(__file__).resolve().parent;root=Path('.dedup-work/zigimage422-20261021');binary=root/'wrapper-dimension-control'
b=(root/'include/stb_image_resize.h').read_bytes();tree=json.loads(Path('docs/research/dedup-500/evidence/galactixx--imagehash/tree.json').read_text());expected=next(x['sha'] for x in tree['tree'] if x['path']=='include/stb_image_resize.h');assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==expected
subprocess.run(['clang','-std=c99','-O2','-I',str(root/'include'),str(folder/'control.c'),'-lm','-o',str(binary)],check=True)
rows=[]
for w,h in [(1,1),(3,5),(7,1),(11,1),(13,7),(17,9),(31,33),(47,51),(63,65),(97,101)]:
 for ow,oh in [(8,8),(9,8),(32,32),(64,64)]:
  p=subprocess.run([str(binary),str(w),str(h),str(ow),str(oh)],capture_output=True,text=True,timeout=10)
  rows.append(dict(input=[w,h],output=[ow,oh],returncode=p.returncode,stderr=p.stderr))
out=dict(cases=rows,passed=sum(r['returncode']==0 for r in rows),failed=sum(r['returncode']!=0 for r in rows),scope='Actual pinned C resize helper under wrapper dimension/filter/edge/colorspace recipe, synthetic grayscale input. Not actual Zig RGB preprocessing/hash functions; no release/assert-disabled rerun.')
(folder/'result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({k:v for k,v in out.items() if k!='cases'}));print(json.dumps([r for r in rows if r['returncode']]))
