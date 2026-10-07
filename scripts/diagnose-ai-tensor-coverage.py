#!/usr/bin/env python3
"""Independent pixel-area diagnostic for a pinned original tensor-patch boundary."""
import argparse,json,struct,hashlib
from pathlib import Path
from pypdf import PdfReader
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('source',type=Path)
parser.add_argument('--report',type=Path,required=True)
args=parser.parse_args()
assert hashlib.sha256(args.source.read_bytes()).hexdigest()=='c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06','pinned source hash mismatch'
s=PdfReader(args.source).pages[0]['/Resources']['/Shading']['/Sh0'].get_object();data=s.get_data();offset=58580;assert data[offset]==0
ranges=list(map(float,s['/Decode']));points=[]
for i in range(16):
 x,y=struct.unpack_from('>II',data,offset+1+i*8);points.append((ranges[0]+x/0xffffffff*(ranges[1]-ranges[0]),246.0+(ranges[2]+y/0xffffffff*(ranges[3]-ranges[2]))))
def curve(ps,t):
 weights=[(1-t)**3,3*t*(1-t)**2,3*t*t*(1-t),t**3]
 return tuple(sum(w*p[axis] for w,p in zip(weights,ps)) for axis in range(2))
def boundary(n):
 out=[]
 for indices in [(0,1,2,3),(3,4,5,6),(6,7,8,9),(9,10,11,0)]:
  ps=[points[i] for i in indices]
  out.extend(curve(ps,i/n) for i in range(n))
 return out
def clip(poly,axis,bound,greater):
 if not poly:return []
 out=[];prev=poly[-1];inside=lambda p:p[axis]>=bound if greater else p[axis]<=bound
 for point in poly:
  a,b=inside(prev),inside(point)
  if a!=b:
   t=(bound-prev[axis])/(point[axis]-prev[axis]);out.append(tuple(prev[d]+t*(point[d]-prev[d]) for d in range(2)))
  if b:out.append(point)
  prev=point
 return out
def coverage(poly,x,y):
 for axis,bound,greater in [(0,x,True),(0,x+1,False),(1,y,True),(1,y+1,False)]:poly=clip(poly,axis,bound,greater)
 return abs(sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(poly,poly[1:]+poly[:1])))/2 if poly else 0.0
rows=[]
for x,y in [(223,117),(224,117),(224,120),(224,124),(225,120)]:
 values=[coverage(boundary(n),x,y) for n in [512,2048,8192]]
 assert abs(values[-1]-values[-2])<1e-5,'boundary area did not converge'
 rows.append({'x':x,'y':y,'coverage_512_2048_8192':values,'alpha8':round(min(1.0,values[-1])*255),'last_difference':abs(values[-1]-values[-2])})
report={'source_sha256':hashlib.sha256(args.source.read_bytes()).hexdigest(),'shading':'Sh0','record_offset':offset,'coordinate_transform':'original shading y -> CTM y flip -> PDF page-to-raster flip at height246',
 'method':'independent cubic boundary evaluation, polygon clipping to pixel squares, shoelace area; convergence at512/2048/8192 subdivisions per edge','rows':rows,'scope':'one source patch boundary; no color integration, overlaps or whole document qualification'}
args.report.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(rows,indent=2))
