#!/usr/bin/env python3
"""Independent pinned AI Type7 boundary/clip areas; diagnostic, not rendering.

Needs pypdf and shapely2.1.2. Reads original compressed patch records, not native
triangles. Refuses unsupported encodings and invalid patch boundaries.
"""
from pathlib import Path
import json,struct,hashlib
from pypdf import PdfReader
import shapely
from shapely.geometry import Polygon,LineString,box
from shapely.ops import unary_union,polygonize
source=Path('tests/fixtures/data/metal-recovery/ai-corpus/VectorApple.ai')
assert hashlib.sha256(source.read_bytes()).hexdigest()=='c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06'
r=PdfReader(source);page_top=float(r.pages[0].cropbox.top);inventory=json.loads(Path('docs/research/ai-original-path-inventory-2026-10-13.json').read_text())
def transform(m,p):
 x,y=p;return (m[0]*x+m[2]*y+m[4],page_top-(m[1]*x+m[3]*y+m[5]))
def cubic(ps,n):
 return [tuple(sum(w*p[d] for w,p in zip([(1-t)**3,3*t*(1-t)**2,3*t*t*(1-t),t**3],ps)) for d in (0,1)) for t in [i/n for i in range(n+1)]]
def patches(s):
 assert [s[k] for k in ('/ShadingType','/BitsPerCoordinate','/BitsPerComponent','/BitsPerFlag')]==[7,32,8,8]
 data=s.get_data();decode=list(map(float,s['/Decode']));channels=(len(decode)-4)//2;assert channels==4
 offset=0;previous=None
 while offset<len(data):
  flag=data[offset];offset+=1;assert flag<=3
  if flag:
   assert previous is not None
   indices={1:[3,4,5,6],2:[6,7,8,9],3:[9,10,11,0]}[flag];points=[previous[i] for i in indices]
  else:points=[]
  for _ in range(12 if flag else 16):
   x,y=struct.unpack_from('>II',data,offset);offset+=8
   points.append((decode[0]+x/0xffffffff*(decode[1]-decode[0]),decode[2]+y/0xffffffff*(decode[3]-decode[2])))
  offset+=(2 if flag else 4)*channels
  assert offset<=len(data);previous=points
  yield points
 assert offset==len(data)
def contours(path,n):
 result=[];poly=[];current=None
 for e in path:
  op,ps=e['op'],e['points']
  if op in ('m','re'):
   if poly:result.append(poly)
   poly=ps[:];current=ps[-1]
  elif op=='l':poly+=ps;current=ps[-1]
  elif op in ('c','v','y'):
   controls=[current]+(ps if op=='c' else ([current]+ps if op=='v' else ps+[ps[-1]]));poly+=cubic(controls,n)[1:];current=controls[-1]
  elif op=='h':
   if poly:result.append(poly);poly=[];current=None
 if poly:result.append(poly)
 return result
def winding(poly,p):
 count=0
 for a,b in zip(poly,poly[1:]+poly[:1]):
  cross=(b[0]-a[0])*(p[1]-a[1])-(p[0]-a[0])*(b[1]-a[1])
  if a[1]<=p[1]<b[1] and cross>0:count+=1
  if b[1]<=p[1]<a[1] and cross<0:count-=1
 return count
def clip_geometry(path,n):
 rings=contours(path,n)
 lines=unary_union([LineString(poly+[poly[0]]) for poly in rings if len(poly)>2])
 faces=[]
 for face in polygonize(lines):
  p=face.representative_point();pt=(p.x,p.y)
  if sum(winding(poly,pt) for poly in rings)!=0:faces.append(face)
 return unary_union(faces)
pixel=box(149,56,150,57);rows=[]
for n in [128,512,2048]:
 for event in inventory['shadings']:
  if event['stream']!='page0':continue
  s=r.pages[0]['/Resources']['/Shading'][event['name']].get_object();polys=[];candidates=0;invalid=0
  if s['/ShadingType']!=7:
   clipped=pixel
   for path in event['clips']:clipped=clipped.intersection(clip_geometry(path,n))
   rows.append({'shading':event['name'],'operator':event['operator_index'],'subdivisions':n,'skipped_shading_type':int(s['/ShadingType']),'candidate_patches':0,'clip_area':clipped.area,'excluded_by_clip':clipped.is_empty})
   continue
  for points in patches(s):
   p=[transform(event['ctm'],p) for p in points]
   if max(q[0] for q in p)<149 or min(q[0] for q in p)>150 or max(q[1] for q in p)<56 or min(q[1] for q in p)>57:continue
   candidates+=1;boundary=[]
   for indices in [(0,1,2,3),(3,4,5,6),(6,7,8,9),(9,10,11,0)]:boundary+=cubic([p[i] for i in indices],n)[:-1]
   poly=Polygon(boundary)
   if not poly.is_valid:invalid+=1;continue
   polys.append(poly.intersection(pixel))
  area=unary_union(polys)
  for path in event['clips']:
   if area.is_empty:break
   area=area.intersection(clip_geometry(path,n))
  rows.append({'shading':event['name'],'operator':event['operator_index'],'subdivisions':n,'candidate_patches':candidates,'invalid_candidate_boundaries':invalid,'area':area.area,'graphics_state':event['graphics_state'],'clip_count':len(event['clips'])})
report={'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'shapely':shapely.__version__,'geos':shapely.geos_version_string,'pixel':[149,56],'page_top':page_top,'rows':rows,'limitations':['Boundary union of valid original patches only; folded/interior-noninjective patches not qualified','No opacity/color/mask composition or form shading evaluation','Nonzero path clips; source evenodd clips require separate verification','Not final page alpha or production renderer qualification']}
Path('docs/research/ai-original-tensor-domains-2026-10-13.json').write_text(json.dumps(report,indent=2)+'\n')
print([(x['shading'],x['subdivisions'],x['area'],x['invalid_candidate_boundaries']) for x in rows if x['candidate_patches']])
