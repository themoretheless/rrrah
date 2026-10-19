#!/usr/bin/env python3
"""Independent Tensor source-net sampling and projected-union pixel areas.

PDF Reference 1.3 pp240-242 defines the tensor basis, stream spiral and flags.
No native triangles/renderer colors enter the surface or projected union.
Converging floating-point polygon areas are numerical evidence, not intervals.
Requires numpy and Shapely (2.1.2 used here, installed only in a temporary path).
"""
import argparse,hashlib,json,re
from pathlib import Path
import numpy as np
import shapely
p=argparse.ArgumentParser(description=__doc__);p.add_argument('corpus',type=Path);p.add_argument('--candidates',type=Path,required=True);p.add_argument('--report',type=Path,required=True);a=p.parse_args()
r=json.loads(a.candidates.read_text());source=r['source'];data=(a.corpus/source['name']).read_bytes()
assert hashlib.sha256(data).hexdigest()==source['sha256']
assert b'1 0 0 1 0 240 cm\n/Sh1 sh' in data and b'/Sh2 sh' not in data
obj=re.search(rb'5 0 obj\s*(.*?)endobj',data,re.S)[1]
assert b'/ShadingType 7/ColorSpace/DeviceRGB/BitsPerCoordinate 16/BitsPerComponent 8/BitsPerFlag 8/Decode[50 250 40 240 0 1 0 1 0 1]' in obj
body=re.search(rb'stream\s*(.*?)endstream',obj,re.S)[1];raw=bytes.fromhex(body.decode().replace('>',''))
order=[(0,0),(0,1),(0,2),(0,3),(1,3),(2,3),(3,3),(3,2),(3,1),(3,0),(2,0),(1,0),(1,1),(1,2),(2,2),(2,1)]
patches=[];flags=[];offset=0;previous=None
while offset<len(raw):
 flag=raw[offset];offset+=1;assert flag in (0,1,2,3)
 points=[] if flag==0 else [previous[i] for i in {1:[3,4,5,6],2:[6,7,8,9],3:[9,10,11,0]}[flag]]
 for _ in range(16-len(points)):
  x=int.from_bytes(raw[offset:offset+2],'big');y=int.from_bytes(raw[offset+2:offset+4],'big');offset+=4
  points.append((50+200*x/65535,280+200*y/65535))
 colors=12 if flag==0 else 6;assert raw[offset:offset+colors]==bytes(colors);offset+=colors
 net=np.empty((4,4,2))
 for ij,point in zip(order,points):net[ij]=point
 patches.append(net);flags.append(flag);previous=points
assert offset==len(raw) and len(patches)==4 and flags==[0,2,3,1]
xs=[row['x'] for row in r['rows']];ys=[row['y_bottom'] for row in r['rows']]
bounds=(min(xs),min(ys),max(xs)+1,max(ys)+1);levels=[]
for n in [32,64,128,256,512,1024]:
 t=np.linspace(0,1,n+1);b=np.stack([(1-t)**3,3*t*(1-t)**2,3*t*t*(1-t),t**3],axis=1)
 polygons=[];patch_unions=[];count=0
 for net in patches:
  q=np.einsum('ui,vj,ijc->uvc',b,b,net)
  triangles=np.concatenate([np.stack([q[:-1,:-1],q[1:,:-1],q[1:,1:]],axis=2).reshape(-1,3,2),np.stack([q[:-1,:-1],q[1:,1:],q[:-1,1:]],axis=2).reshape(-1,3,2)])
  lo=triangles.min(axis=1);hi=triangles.max(axis=1)
  keep=(hi[:,0]>=bounds[0])&(hi[:,1]>=bounds[1])&(lo[:,0]<=bounds[2])&(lo[:,1]<=bounds[3])
  count+=int(np.count_nonzero(keep));part=shapely.polygons(triangles[keep]);polygons.extend(part)
  patch_unions.append(shapely.union_all(part))
 union=shapely.union_all(patch_unions);assert union.is_valid
 alpha=[255*union.intersection(shapely.box(x,y,x+1,y+1)).area for x,y in zip(xs,ys)]
 patch_alpha=[sum(p.intersection(shapely.box(x,y,x+1,y+1)).area for p in patch_unions)*255 for x,y in zip(xs,ys)]
 triangle_alpha=[]
 if n==1024:
  geometries=np.array(polygons,dtype=object);tree=shapely.STRtree(geometries)
  for x,y in zip(xs,ys):
   square=shapely.box(x,y,x+1,y+1);ids=tree.query(square,predicate='intersects')
   triangle_alpha.append(float(np.sum(shapely.area(shapely.intersection(geometries[ids],square))))*255)
 levels.append(dict(subdivisions=n,candidate_triangles=count,alpha=alpha,patch_sum_alpha=patch_alpha,triangle_sum_alpha=triangle_alpha));print(n,count,alpha,flush=True)
rows=[]
for i,row in enumerate(r['rows']):
 value=levels[-1]['alpha'][i];rows.append(dict(**row,area_alpha=value,area_final_delta=abs(value-levels[-2]['alpha'][i]),native_area_error=abs(row['native_rgba'][3]-value),pdfium16_area_error=abs(255-row['pdfium16_white'][0]-value),patch_sum_alpha=levels[-1]['patch_sum_alpha'][i],triangle_sum_alpha=levels[-1]['triangle_sum_alpha'][i],native_triangle_sum_error=abs(row['native_rgba'][3]-min(255,levels[-1]['triangle_sum_alpha'][i]))))
result=dict(scope='22 previously selected pixels only; independent source bytes and PDF basis/continuation; uniform32..1024 sampling, projected triangle union and GEOS pixel intersection; floating-point convergence is not an analytic/interval or full-page certificate',source=source,spec='https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.3.pdf#page=255',numpy=np.__version__,shapely=shapely.__version__,patch_flags=flags,levels=levels,rows=rows,max_final_delta=max(row['area_final_delta'] for row in rows),max_native_error=max(row['native_area_error'] for row in rows),max_pdfium16_error=max(row['pdfium16_area_error'] for row in rows))
a.report.write_text(json.dumps(result,indent=2)+'\n');print({k:v for k,v in result.items() if k not in ['rows','levels','source']})
