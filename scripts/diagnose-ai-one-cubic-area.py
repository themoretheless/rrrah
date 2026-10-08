import re,json,math
from pathlib import Path
w,h=1366,768
rgba=Path('/tmp/rrrah-ai-one-current.rgba').read_bytes();ppm=Path('/tmp/rrrah-ai-one-poppler.ppm').read_bytes();m=re.match(rb'P6\s+\d+\s+\d+\s+255\s',ppm);rgb=ppm[m.end():]
actual=bytes((c*a+255*(255-a)+127)//255 for r,g,b,a in zip(rgba[::4],rgba[1::4],rgba[2::4],rgba[3::4]) for c in [r,g,b])
worst=sorted(range(w*h),key=lambda i:abs(actual[i*3]-rgb[i*3]),reverse=True)[:12]
def polygon(n,cx,rx):
 # Original four cubic segments, translated and flipped to raster coordinates.
 k=58.203 if rx==130 else 53.278; right=0;left=-2*rx;mid=-rx
 curves=[[(0,0),(0,-115.151),(-k,-208.5),(mid,-208.5)],[(mid,-208.5),(-2*rx+k,-208.5),(left,-115.151),(left,0)],[(left,0),(left,115.151),(-2*rx+k,208.5),(mid,208.5)],[(mid,208.5),(-k,208.5),(0,115.151),(0,0)]]
 return [(cx+sum(p[0]*v for p,v in zip(c,[(1-t)**3,3*t*(1-t)**2,3*t*t*(1-t),t**3])),h-370.5-sum(p[1]*v for p,v in zip(c,[(1-t)**3,3*t*(1-t)**2,3*t*t*(1-t),t**3]))) for c in curves for t in (j/n for j in range(n))]
def clip(poly,axis,bound,greater):
 out=[]
 if not poly:return out
 prev=poly[-1]
 for p in poly:
  a=(prev[axis]>=bound) if greater else (prev[axis]<=bound);b=(p[axis]>=bound) if greater else (p[axis]<=bound)
  if a!=b:
   t=(bound-prev[axis])/(p[axis]-prev[axis]);out.append(tuple(prev[i]+t*(p[i]-prev[i]) for i in [0,1]))
  if b:out.append(p)
  prev=p
 return out
def area(poly,x,y):
 for a,b,g in [(0,x,True),(0,x+1,False),(1,y,True),(1,y+1,False)]:poly=clip(poly,a,b,g)
 return abs(sum((a[0]-x)*(b[1]-y)-(b[0]-x)*(a[1]-y) for a,b in zip(poly,poly[1:]+poly[:1])))/2 if poly else 0
rows=[]
polys={n:[polygon(n,454,130),polygon(n,1186,119)] for n in [1024,8192]}
for i in worst:
 x,y=i%w,i//w;v=[sum(area(p,x,y) for p in polys[n]) for n in [1024,8192]];assert abs(v[0]-v[1])<1e-4
 expected=round(255*(1-min(1,v[-1])));rows.append(dict(x=x,y=y,coverage=v,area_gray8=expected,native_gray8=actual[i*3],poppler_gray8=rgb[i*3]))
report={'scope':'12 largest differing pixels of two authored cubic fills in producer one.ai page0; no whole-page proof','method':'Independent original cubic evaluation; polygon pixel clipping; local shoelace area; 1024/8192 subdivisions per curve converge within1e-4','rows':rows}
Path('docs/research/ai-one-cubic-area-2026-10-07.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(rows,indent=2))
