#!/usr/bin/env python3
"""Independent Coons inverse map and polynomial tint integration for pinned Sh2."""
import argparse,hashlib,json,math,re,subprocess
from pathlib import Path
p=Path(__file__).resolve().parent
ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('corpus',type=Path);ap.add_argument('--dump',type=Path,required=True);a=ap.parse_args()
source=next(x for x in json.loads((p/'isolated-sources.json').read_text())['sources'] if x['name']=='shade-coons-sh2.pdf')
path=a.corpus/source['name'];data=path.read_bytes();assert hashlib.sha256(data).hexdigest()==source['sha256']
obj=re.search(rb'9 0 obj\s*(.*?)endobj',data,re.S)[1];body=re.search(rb'stream\s*(.*?)endstream',obj,re.S)[1];raw=bytes.fromhex(body.decode().replace('>',''));assert len(raw)==53 and raw[0]==0 and raw[-4:]==bytes([0,255,255,0])
assert b'/Decode[50 250 40 240 0 1]' in obj
assert b'/C0[1 0 0]/C1[0 1 0]/N 3' in data and b'/C0[0 1 0]/C1[0 0 1]/N 3' in data and b'/Bounds[.5]/Encode[0 1 0 1]' in data
cp=[(50+200*int.from_bytes(raw[1+i*4:3+i*4],'big')/65535,40+200*int.from_bytes(raw[3+i*4:5+i*4],'big')/65535) for i in range(12)]
def cubic(ids,t):
 b=[(1-t)**3,3*t*(1-t)**2,3*t*t*(1-t),t**3]
 return tuple(sum(cp[i][k]*w for i,w in zip(ids,b)) for k in range(2))
def surface(u,v):
 c0=cubic([0,11,10,9],u);c1=cubic([3,4,5,6],u);d0=cubic([0,1,2,3],v);d1=cubic([9,8,7,6],v)
 return tuple((1-v)*c0[k]+v*c1[k]+(1-u)*d0[k]+u*d1[k]-((1-v)*((1-u)*cp[0][k]+u*cp[9][k])+v*((1-u)*cp[3][k]+u*cp[6][k])) for k in range(2))
def inverse(x,y,seed):
 u,v=seed
 for _ in range(30):
  q=surface(u,v);rx,ry=q[0]-x,q[1]-y
  if max(abs(rx),abs(ry))<1e-9:return (u,v) if 0<=u<=1 and 0<=v<=1 else None
  h=1e-5;pu=surface(u+h,v);mu=surface(u-h,v);pv=surface(u,v+h);mv=surface(u,v-h)
  ax,ay=[(a-b)/(2*h) for a,b in zip(pu,mu)];bx,by=[(a-b)/(2*h) for a,b in zip(pv,mv)];det=ax*by-ay*bx
  if abs(det)<1e-12:return None
  u-=(rx*by-ry*bx)/det;v-=(ax*ry-ay*rx)/det
  if not(-1<u<2 and -1<v<2):return None
 return None
def tint(v):
 if v<=.5:s=(2*v)**3;return (1-s,s,0)
 s=(2*v-1)**3;return (0,1-s,s)
def integrated(x,y,seed,n):
 total=[0.,0.,0.]
 for iy in range(n):
  for ix in range(n):
   uv=inverse(x+(ix+.5)/n,y+(iy+.5)/n,seed);assert uv is not None
   for k,c in enumerate(tint(uv[1])):total[k]+=c
 return [c*255/(n*n) for c in total]
dest=a.corpus/'coons-function-oracle.native.rgba';info=json.loads(subprocess.check_output([str(a.dump),str(path),'0',str(dest)],text=True));pixels=dest.read_bytes();assert info['width']==300 and info['height']==520
rows=[]
for u in [.25,.5,.75]:
 for v in [.15,.3,.4,.6,.75,.85]:
  point=surface(u,v);x,y=map(math.floor,point);roots=[]
  for su in [.1,.5,.9]:
   for sv in [.1,.5,.9]:
    uv=inverse(x+.5,y+.5,(su,sv))
    if uv is not None and all(math.dist(uv,r)>1e-6 for r in roots):roots.append(uv)
  assert len(roots)==1,(x,y,roots)
  values={n:integrated(x,y,roots[0],n) for n in [4,8,16]};offset=((519-y)*300+x)*4;native=list(pixels[offset:offset+4]);assert native[3]==255
  row=dict(x=x,y_bottom=y,seed=[u,v],inverse_center=roots[0],native=native,integrated_rgb=values,max_8_to_16_delta=max(abs(a-b) for a,b in zip(values[8],values[16])),max_native_error=max(abs(a-b) for a,b in zip(native,values[16])))
  rows.append(row);print(x,y,row['max_native_error'],row['max_8_to_16_delta'],flush=True)
(p/'function-oracle.json').write_text(json.dumps(dict(scope='18 selected interior pixels; independently decoded16-bit controls; Newton inverse of bicubic Coons blend and polynomial Type2/3 tint; midpoint quadrature4/8/16; nine-start inverse uniqueness check at centers is not a global injectivity certificate; no renderer used as mathematical oracle; finite double-precision numerical estimate, not exact interval proof',source=source,decoder_sha256=hashlib.sha256(a.dump.read_bytes()).hexdigest(),rows=rows),indent=2)+'\n')
