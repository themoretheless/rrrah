"""Splice existing coded alpha payloads; deliberately invalid depth controls."""
from pathlib import Path
import importlib.util,json,hashlib
root=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('helpers',root/'docs/research/avif-backend-pixel-limit-2026-10-15/generate-alpha-control.py');h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
report=[]
for master,alpha in [(8,10),(10,8)]:
 base=root/f'tests/fixtures/raster/ravif-{master}bit-prem.avif';other=root/f'tests/fixtures/raster/ravif-{alpha}bit-prem.avif';top,meta,children,items=h.parts(base.read_bytes());alpha_payload=dict(h.parts(other.read_bytes())[3])[2]
 items=[(i,alpha_payload if i==2 else b)for i,b in items];offset=sum(len(h.m.box(k,v))for k,v in top if k!=b'mdat')+8
 iloc=bytearray(bytes.fromhex('000000004400')+len(items).to_bytes(2,'big'))
 for i,b in items:
  iloc.extend(i.to_bytes(2,'big')+b'\0\0\0\1'+offset.to_bytes(4,'big')+len(b).to_bytes(4,'big'));offset+=len(b)
 newmeta=meta[:4]+b''.join(h.m.box(k,iloc if k==b'iloc' else v)for k,v in children)
 data=b''.join(h.m.box(k,newmeta if k==b'meta' else b''.join(b for _,b in items)if k==b'mdat' else v)for k,v in top)
 name=f'primary{master}-alpha{alpha}.avif';Path(__file__).with_name(name).write_bytes(data); (root/'tests/fixtures/raster'/('avif-mixed-'+name)).write_bytes(data)
 report.append({'file':name,'sha256':hashlib.sha256(data).hexdigest(),'coded_primary_depth':master,'coded_alpha_depth':alpha,'sources':{p.name:hashlib.sha256(p.read_bytes()).hexdigest()for p in [base,other]},'scope':'malformed diagnostic control; original declared properties retained; not valid producer/fidelity fixture'})
Path(__file__).with_name('controls.json').write_text(json.dumps(report,indent=2)+'\n')
