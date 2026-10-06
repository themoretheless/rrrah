import argparse,hashlib,json,math
from pathlib import Path
parser=argparse.ArgumentParser(description="Numerical coverage diagnostic for the pinned CC0 cubic-circle fixture")
parser.add_argument("--native",type=Path,default=Path("/tmp/rrrah-curve-circle/native.rgba"))
parser.add_argument("--poppler",type=Path,default=Path("/tmp/rrrah-curve-circle/poppler.ppm"))
parser.add_argument("--report",type=Path,default=Path("docs/research/pdf-curve-area-oracle-2026-10-06.json"))
args=parser.parse_args()
fixture=Path(__file__).resolve().parents[1]/"tests/fixtures/pdf/curve-circle.pdf"
manifest=json.loads(fixture.with_suffix(".json").read_text())
assert hashlib.sha256(fixture.read_bytes()).hexdigest()==manifest["sha256"], "Fixture hash mismatch"
segments=[[(14,8),(14,11.3137085),(11.3137085,14),(8,14)],[(8,14),(4.6862915,14),(2,11.3137085),(2,8)],[(2,8),(2,4.6862915),(4.6862915,2),(8,2)],[(8,2),(11.3137085,2),(14,4.6862915),(14,8)]]
def polygon(steps):
 out=[]
 for ps in segments:
  for i in range(steps):
   t=i/steps;v=[(1-t)**3,3*(1-t)**2*t,3*(1-t)*t*t,t**3]
   out.append(tuple(sum(w*p[d] for w,p in zip(v,ps)) for d in range(2)))
 return out
def clip(points,axis,bound,greater):
 if not points:return []
 out=[];prev=points[-1];inside=lambda p:p[axis]>=bound if greater else p[axis]<=bound
 for curr in points:
  a,b=inside(prev),inside(curr)
  if a!=b:
   t=(bound-prev[axis])/(curr[axis]-prev[axis]);out.append(tuple(prev[d]+t*(curr[d]-prev[d]) for d in range(2)))
  if b:out.append(curr)
  prev=curr
 return out
def cover(steps):
 base=polygon(steps);out=[]
 for row in range(16):
  for x in range(16):
   y=15-row;p=base
   for axis,bound,greater in [(0,x,True),(0,x+1,False),(1,y,True),(1,y+1,False)]:p=clip(p,axis,bound,greater)
   area=abs(sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(p,p[1:]+p[:1])))/2 if p else 0
   out.append(max(0,min(255,int(area*255+0.5))))
 return out
low,high=cover(128),cover(512)
native=args.native.read_bytes();assert len(native)==16*16*4, 'Native extent mismatch'
header,ppm=args.poppler.read_bytes().split(b'\n255\n',1);assert header==b'P6\n16 16' and len(ppm)==16*16*3, 'Oracle extent mismatch'
a=native[3::4];b=bytes(255-v for v in ppm[0::3])
report={'scope':'Numerical area coverage of authored cubic path, polygon clipping with128/512 subdivisions per segment; not universal PDF rendering oracle','subdivision_quantized_difference':max(abs(x-y) for x,y in zip(low,high)),'native_vs_area_max':max(abs(x-y) for x,y in zip(a,high)),'poppler_vs_area_max':max(abs(x-y) for x,y in zip(b,high)),'native_vs_area_different':sum(x!=y for x,y in zip(a,high)),'poppler_vs_area_different':sum(x!=y for x,y in zip(b,high))}
args.report.write_text(json.dumps(report,indent=2)+'\n');print(report)
