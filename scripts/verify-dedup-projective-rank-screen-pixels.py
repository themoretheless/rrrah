"""Independent linear-system/bilinear/window counts for real projective atlas.
Uses native normalized pixel dumps; does not independently validate decoding.
"""
import ast,hashlib,json
from pathlib import Path
import numpy as np
b=Path('docs/research');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins_path=b/'dedup-projective-rank-screen-pins.json';pins=json.loads(pins_path.read_text());assert all(h(k)==v for k,v in pins.items())
helper=Path('scripts/measure-dedup-piecewise-pixels.py');s=helper.read_text();nodes={v.name:v for v in ast.parse(s).body if isinstance(v,ast.FunctionDef)};scope={'np':np};exec(compile(ast.Module(body=[nodes['box'],nodes['rank']],type_ignores=[]),str(helper),'exec'),scope)
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/rank-normalized-pixel-oracle');magic=b'RRRAH-RANK-RGBA32-V1\n'
def read(name):
 p=root/(name+'.rgba32')
 with p.open('rb') as f:
  assert f.read(len(magic))==magic;w=int.from_bytes(f.read(4),'little');height=int.from_bytes(f.read(4),'little')
 a=np.memmap(p,dtype='<f4',mode='r',offset=len(magic)+8,shape=(height,w,4));assert np.all(a[:,:,3]==1)
 return a[:,:,0].astype('f8')*.2126+a[:,:,1].astype('f8')*.7152+a[:,:,2].astype('f8')*.0722
model=np.array(list(map(float,(b/'dedup-projective-rank-screen-model.txt').read_text().split()))).reshape(3,3);source=read('200500');target=read('200501')
report=b/'dedup-projective-rank-screen.jsonl';report_hash=h(report);rows=[json.loads(v) for v in report.read_text().splitlines()];assert len(rows)==6
summary=[]
for name,m,a,t in [('forward',model,source,target),('reverse',np.linalg.inv(model),target,source)]:
 height,w=t.shape;output=np.full(t.shape,np.nan);covered=0
 for y0 in range(0,height,64):
  y1=min(height,y0+64);yy,xx=np.indices((y1-y0,w),dtype='f8');yy+=y0
  q=np.linalg.solve(m,np.stack([xx.ravel(),yy.ravel(),np.ones(xx.size)]));sx=(q[0]/q[2]).reshape(xx.shape);sy=(q[1]/q[2]).reshape(xx.shape)
  assert np.all(np.isfinite(sx)) and np.all(np.isfinite(sy));geometry=(sx>=0)&(sy>=0)&(sx<a.shape[1])&(sy<a.shape[0]);covered+=int(geometry.sum())
  valid=geometry&(sx+1<a.shape[1])&(sy+1<a.shape[0]);ix=np.clip(np.floor(sx),0,a.shape[1]-2).astype('i8');iy=np.clip(np.floor(sy),0,a.shape[0]-2).astype('i8');tx=sx-ix;ty=sy-iy
  values=(a[iy,ix]*(1-tx)+a[iy,ix+1]*tx)*(1-ty)+(a[iy+1,ix]*(1-tx)+a[iy+1,ix+1]*tx)*ty
  output[y0:y1][valid]=values[valid]
 for radius in [0,3,8]:
  calculated=scope['rank'](output,t,radius);native=next(v for v in rows if v['direction']==name and v['filter_radius']==radius)
  assert native['sites']==t.size and native['covered']==covered and native['pixel_reads']==5*t.size
  for key in ['valid_sites','informative_pairs','agreeing_pairs']:assert native[key]==calculated[key],(name,radius,key,native[key],calculated[key])
  summary.append({'direction':name,'radius':radius,**calculated});print(name,radius,'verified',flush=True)
assert h(report)==report_hash and all(h(k)==v for k,v in pins.items())
output=b/'dedup-projective-rank-screen-pixel-audit.json';assert not output.exists();output.write_text(json.dumps({'status':'verified_real_projective_atlas_rank_pixel_counts','comparisons':6,'summary':summary,'pins':{**pins,str(pins_path):h(pins_path),str(report):report_hash,str(helper):h(helper),str(Path(__file__)):h(__file__)},'scope':'Independent linear-system mapping, geometric coverage, bilinear luminance, complete box/ordinal windows for all forward/reverse sites; six exact count checks. Shared native normalized decoder dumps, not independent decoding or copy admission/broad negatives/performance qualification.'},indent=2)+'\n')
