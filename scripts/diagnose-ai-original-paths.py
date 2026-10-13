#!/usr/bin/env python3
"""Independent original VectorApple PDF contour inventory at pixel149,56.

Requires pypdf; no native renderer data is read. Individual contour area is
not final page coverage: clip intersections, masks and shading domains are
not adjudicated by this script.
"""
from pypdf import PdfReader
from pypdf.generic import ContentStream
from pathlib import Path
import json,hashlib
source=Path('tests/fixtures/data/metal-recovery/ai-corpus/VectorApple.ai')
assert hashlib.sha256(source.read_bytes()).hexdigest()=='c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06'
r=PdfReader(source);rows=[];shadings=[]
assert list(map(float,r.pages[0].cropbox))==[0.,0.,300.001,245.5]
page_top=float(r.pages[0].cropbox.top)
def mul(a,b):
 return [a[0]*b[0]+a[2]*b[1],a[1]*b[0]+a[3]*b[1],a[0]*b[2]+a[2]*b[3],a[1]*b[2]+a[3]*b[3],a[0]*b[4]+a[2]*b[5]+a[4],a[1]*b[4]+a[3]*b[5]+a[5]]
def point(m,x,y):return [m[0]*x+m[2]*y+m[4],page_top-(m[1]*x+m[3]*y+m[5])]
def walk(stream,res,label,m,depth=0):
 assert depth<=12
 stack=[];path=[];clips=[];pending=None;gs=None
 for index,(args,op) in enumerate(ContentStream(stream,r).operations):
  assert index<100000
  if op==b'q':stack.append((m[:],clips[:],gs))
  elif op==b'Q':m,clips,gs=stack.pop()
  elif op==b'cm':m=mul(m,list(map(float,args)))
  elif op==b'gs':gs=str(args[0])
  elif op in (b'm',b'l',b'c',b'v',b'y'):
   values=list(map(float,args));pts=[point(m,values[i],values[i+1]) for i in range(0,len(values),2)];path.append({'op':op.decode(),'points':pts})
  elif op==b're':
   x,y,w,h=map(float,args);path.append({'op':'re','points':[point(m,x,y),point(m,x+w,y),point(m,x+w,y+h),point(m,x,y+h)]})
  elif op==b'h':path.append({'op':'h','points':[]})
  elif op in (b'W',b'W*'):pending=op.decode()
  elif op in (b'n',b'f',b'f*',b'F',b'S',b's',b'B',b'B*',b'b',b'b*'):
   if path:
    pts=[p for e in path for p in e['points']];bbox=[min(p[0] for p in pts),min(p[1] for p in pts),max(p[0] for p in pts),max(p[1] for p in pts)] if pts else None
    if bbox and bbox[0]<=150 and bbox[2]>=149 and bbox[1]<=57 and bbox[3]>=56:
     rows.append({'stream':label,'operator_index':index,'end_op':op.decode(),'clip_rule':pending,'graphics_state':gs,'bbox':bbox,'path':path[:],'inherited_clip_count':len(clips)})
    if pending:clips.append(path[:])
   path=[];pending=None
  elif op==b'sh':shadings.append({'stream':label,'operator_index':index,'name':str(args[0]),'ctm':m[:],'clips':clips[:],'graphics_state':gs})
  elif op==b'Do':
   name=str(args[0]);obj=res['/XObject'][name].get_object()
   if obj.get('/Subtype')=='/Form':walk(obj,obj.get('/Resources',res),label+'/'+name,mul(m,list(map(float,obj.get('/Matrix',[1,0,0,1,0,0])))),depth+1)
walk(r.pages[0].get_contents(),r.pages[0]['/Resources'],'page0',[1,0,0,1,0,0])
report={'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'pixel':[149,56],'page_top':page_top,'paths':rows,'shadings':shadings,'scope':'Independent original PDF path control-box inventory; not visibility or compositing oracle','limits':['Control-box intersection is conservative','Inherited form bbox and masks are not rasterized','No area qualification from inventory alone']}

print([(a['stream'],a['operator_index'],a['end_op'],a['clip_rule'],len(a['path'])) for a in rows])
def clip(poly,axis,bound,greater):
 if not poly:return []
 out=[];prev=poly[-1];inside=lambda p:p[axis]>=bound if greater else p[axis]<=bound
 for p in poly:
  a,b=inside(prev),inside(p)
  if a!=b:
   t=(bound-prev[axis])/(p[axis]-prev[axis]);out.append([prev[d]+t*(p[d]-prev[d]) for d in range(2)])
  if b:out.append(p)
  prev=p
 return out
def area(path,n):
 polys=[];poly=[];current=None
 for e in path:
  op,ps=e['op'],e['points']
  if op in ('m','re'):
   if poly:polys.append(poly)
   poly=ps[:];current=ps[-1]
  elif op=='l':poly+=ps;current=ps[-1]
  elif op in ('c','v','y'):
   controls=[current]+(ps if op=='c' else ([current]+ps if op=='v' else ps+[ps[-1]]))
   for i in range(1,n+1):
    t=i/n;weights=[(1-t)**3,3*t*(1-t)**2,3*t*t*(1-t),t**3];poly.append([sum(w*p[d] for w,p in zip(weights,controls)) for d in (0,1)])
   current=controls[-1]
  elif op=='h':
   if poly:polys.append(poly);poly=[];current=None
 if poly:polys.append(poly)
 total=0
 for poly in polys:
  for axis,bound,greater in [(0,149,True),(0,150,False),(1,56,True),(1,57,False)]:poly=clip(poly,axis,bound,greater)
  if poly:total+=sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(poly,poly[1:]+poly[:1]))/2
 return abs(total)
assert area([{'op':'re','points':[[149,56],[149.5,56],[149.5,57],[149,57]]}],1)==0.5
assert area([{'op':'re','points':[[0,0],[1,0],[1,1],[0,1]]}],1)==0
for row in rows:
 row['individual_clipped_area_128_512_2048']=[area(row['path'],n) for n in (128,512,2048)]
report['limits'].append('Individual signed contour area only; no intersection of multiple clips, winding overlap resolution, shading domain or masks')
Path('docs/research/ai-original-path-inventory-2026-10-13.json').write_text(json.dumps(report,indent=2)+'\n')
print([(x['operator_index'],x['individual_clipped_area_128_512_2048']) for x in rows])
