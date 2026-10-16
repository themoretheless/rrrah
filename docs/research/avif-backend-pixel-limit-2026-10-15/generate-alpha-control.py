"""Malformed metadata/geometry control: small coded primary, larger coded alpha."""
from pathlib import Path
import importlib.util,json,hashlib
root=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('boxes',root/'scripts/generate-avif-square-pixel-fixtures.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
def parts(data):
 top=list(m.boxes(data));meta=next(v for k,v in top if k==b'meta');children=list(m.boxes(meta[4:]));iloc=next(v for k,v in children if k==b'iloc');assert iloc[:6]==bytes.fromhex('000000004400');at=8;items=[]
 for _ in range(int.from_bytes(iloc[6:8],'big')):
  item=int.from_bytes(iloc[at:at+2],'big');assert iloc[at+2:at+6]==b'\0\0\0\1';offset=int.from_bytes(iloc[at+6:at+10],'big');size=int.from_bytes(iloc[at+10:at+14],'big');items.append((item,data[offset:offset+size]));at+=14
 assert at==len(iloc)
 return top,meta,children,items
base=root/'tests/fixtures/raster/ravif-8bit-prem.avif';small=root/'tests/fixtures/raster/avif-orientation-2.avif';top,meta,children,items=parts(base.read_bytes());small_primary=dict(parts(small.read_bytes())[3])[1]
items=[(i,small_primary if i==1 else b)for i,b in items];offset=sum(len(m.box(k,v))for k,v in top if k!=b'mdat')+8
iloc=bytearray(bytes.fromhex('000000004400')+len(items).to_bytes(2,'big'))
for i,b in items:
 iloc.extend(i.to_bytes(2,'big')+b'\0\0\0\1'+offset.to_bytes(4,'big')+len(b).to_bytes(4,'big'));offset+=len(b)
newmeta=meta[:4]+b''.join(m.box(k,iloc if k==b'iloc' else v)for k,v in children)
result=b''.join(m.box(k,newmeta if k==b'meta' else b''.join(b for _,b in items)if k==b'mdat' else v)for k,v in top)
dest=Path(__file__).with_name('small-primary-large-alpha.avif');dest.write_bytes(result)
Path(__file__).with_name('alpha-control.json').write_text(json.dumps({'sources':{str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest()for p in [base,small]},'sha256':hashlib.sha256(result).hexdigest(),'scope':'adversarial container control; primary coded8x4, alpha coded16x16, original ispe16x16 retained; not valid producer or fidelity fixture'},indent=2)+'\n')
