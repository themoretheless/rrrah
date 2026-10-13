#!/usr/bin/env python3
"""Probe VectorApple event2676 clip at pixel78,144 from raw callback trace.

Independent cubic integration; callback extraction itself is not an independent
PDF parser. This does not qualify the complete AI file or change acceptance.
"""
from pathlib import Path
import re,ast,math,json,argparse,hashlib
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--trace',type=Path,required=True)
parser.add_argument('--report',type=Path,required=True)
args=parser.parse_args()
if args.report.exists():raise ValueError('Use a fresh report path')
line=next(l for l in args.trace.read_text().splitlines() if l.startswith('NATIVE_SOURCE_CLIP event=2676'))
elements=[]
for m in re.finditer(r'(MoveTo|LineTo|CurveTo)\((.*?)\)(?=, (?:MoveTo|LineTo|CurveTo|ClosePath)|\])|(ClosePath)',line.split('elements=',1)[1]):
 if m.group(3):elements.append(('ClosePath',()));continue
 points=ast.literal_eval('('+m.group(2)+',)');elements.append((m.group(1),points))
assert elements
if elements[-1][0]!='ClosePath':elements.append(('ClosePath',()))
segments=[];start=cur=None
for kind,points in elements:
 if kind=='MoveTo':cur=start=points[0]
 elif kind=='LineTo':
  end=points[0];segments.append((cur,(cur[0]+(end[0]-cur[0])/3,cur[1]+(end[1]-cur[1])/3),(cur[0]+2*(end[0]-cur[0])/3,cur[1]+2*(end[1]-cur[1])/3),end));cur=end
 elif kind=='CurveTo':segments.append((cur,*points));cur=points[-1]
 else:
  if cur!=start:segments.append((cur,(cur[0]+(start[0]-cur[0])/3,cur[1]+(start[1]-cur[1])/3),(cur[0]+2*(start[0]-cur[0])/3,cur[1]+2*(start[1]-cur[1])/3),start))
def poly(p):return (-p[0]+3*p[1]-3*p[2]+p[3],3*p[0]-6*p[1]+3*p[2],-3*p[0]+3*p[1],p[0])
def val(c,t):return ((c[0]*t+c[1])*t+c[2])*t+c[3]
branches=[]
for s in segments:
 if min(p[1] for p in s)>145 or max(p[1] for p in s)<144:continue
 cx=poly([p[0] for p in s]);cy=poly([p[1] for p in s]);a,b,c=3*cy[0],2*cy[1],cy[2];roots=[]
 if abs(a)<1e-14:
  if abs(b)>1e-14:roots=[-c/b]
 elif b*b-4*a*c>=0:
  d=math.sqrt(b*b-4*a*c);roots=[(-b-d)/(2*a),(-b+d)/(2*a)]
 knots=[0]+sorted(t for t in roots if 0<t<1)+[1]
 for lo,hi in zip(knots,knots[1:]):
  ya,yb=val(cy,lo),val(cy,hi)
  if ya!=yb:branches.append((cx,cy,lo,hi,ya,yb))
def width(y):
 hits=[]
 for cx,cy,lo,hi,ya,yb in branches:
  if not min(ya,yb)<=y<max(ya,yb):continue
  direction=1 if yb>ya else -1
  for _ in range(52):
   mid=(lo+hi)/2
   if (val(cy,mid)<y)==(direction>0):lo=mid
   else:hi=mid
  hits.append((val(cx,(lo+hi)/2),direction))
 hits.sort();w=0;area=0;last=None
 for x,d in hits:
  if last is not None and w:area+=max(0,min(79,x)-max(78,last))
  w+=d;last=x
 assert w==0,(y,hits)
 return area
areas=[]
for n in [256,1024,4096]:
 total=width(144)+width(145)
 for i in range(1,n):total+=(4 if i%2 else 2)*width(144+i/n)
 areas.append({'simpson_steps':n,'area':total/(3*n),'coverage_u8':round(total/(3*n)*255)})
r={'pixel':[78,144],'source_clip_elements':elements,'candidate_monotonic_branches':len(branches),'integration':areas,'pixel_center_covered_width':width(144.5),'scope':'Independent direct cubic-y inversion and nonzero-winding scanline integration from pre-flatten interpreter callback clip. Not independent PDF parser or formal interval proof.'}
r['trace_sha256']=hashlib.sha256(args.trace.read_bytes()).hexdigest()
args.report.write_text(json.dumps(r,indent=2)+'\n');print(areas);print('center width',width(144.5),'elements',len(elements),'branches',len(branches))
