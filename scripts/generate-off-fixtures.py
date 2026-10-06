#!/usr/bin/env python3
"""CC0 OFF sources: trimesh 4.11.2 geometry oracles and authored attribute/binary/concave cases."""
from pathlib import Path
import hashlib
import struct
import numpy as np
import trimesh
assert trimesh.__version__ == '4.11.2'
root=Path(__file__).resolve().parent.parent/'tests/fixtures/models'
root.mkdir(parents=True,exist_ok=True)
triangle=[[-1,-1,0],[1,-1,0],[0,1,0]]
quad=[[0,0,0],[0,2,0],[2,2,0],[2,0,0]]
u=[[0,0,0],[3,0,0],[3,3,0],[2,3,0],[2,1,0],[1,1,0],[1,3,0],[0,3,0]]
cases={
 'off-triangle.off':('OFF',triangle,[[0,1,2]]),
 'off-clockwise.off':('OFF',quad,[[0,1,2,3]]),
 'off-yz.off':('OFF',[[0,p[0],p[1]] for p in quad],[[0,1,2,3]]),
 'off-offset64.off':('OFF',[[p[0]+10**12,p[1]-10**12,10**12] for p in triangle],[[0,1,2]]),
 'off-concave-u.off':('OFF',u,[list(range(8))]),
 'off-attributes.off':('STCNOFF',triangle,[[0,1,2]]),
 'off-attributes-binary.off':('STCNOFF BINARY',triangle,[[0,1,2]]),
}
manifest=['# source\toracle\tprovenance\tsource_sha256\toracle_sha256']
for name,(header,points,faces) in cases.items():
 attrs='attributes' in name
 binary='BINARY' in header
 if binary:
  data=bytearray((header+' # CC0 authored attributes\n').encode())
  data+=struct.pack('>3i',len(points),len(faces),123)
  for i,p in enumerate(points):
   data+=struct.pack('>12f',*p,0,0,2,0.25,0.5,0.75,1,i*0.25,-i*0.5)
  data+=struct.pack('>5i4f',3,0,1,2,4,0.25,0.5,0.75,1)
 else:
  lines=[header,'# CC0 authored mesh',f'{len(points)} {len(faces)} 123']
  for i,p in enumerate(points):
   values=p+([0,0,2,0.25,0.5,0.75,1,i*0.25,-i*0.5] if attrs else [])
   lines.append(' '.join(map(str,values)))
  for f in faces:
   lines.append(' '.join(map(str,[len(f)]+f))+(' 0.25 0.5 0.75 1.0' if attrs else ''))
  data=('\n'.join(lines)+'\n').encode()
 path=root/name;path.write_bytes(data)
 if name=='off-concave-u.off':
  authored=[]
  for x0,y0,x1,y1 in [(0,0,3,1),(0,1,1,3),(2,1,3,3)]:
   authored.extend([[[x0,y0,0],[x1,y0,0],[x1,y1,0]],[[x0,y0,0],[x1,y1,0],[x0,y1,0]]])
  triangles=np.array(authored,dtype=np.float64);provenance='authored-area-and-cutout'
 elif attrs:
  triangles=np.array(points,dtype=np.float64).reshape(1,3,3);provenance='authored-geometry-and-attributes'
 else:
  mesh=trimesh.load(path,process=False,force='mesh')
  np.testing.assert_array_equal(mesh.vertices,np.array(points))
  triangles=np.asarray(mesh.triangles,dtype=np.float64);provenance='trimesh-4.11.2'
 oracle=name+'.triangles.f64le';raw=triangles.astype('<f8').tobytes();(root/oracle).write_bytes(raw)
 manifest.append('\t'.join([name,oracle,provenance,hashlib.sha256(data).hexdigest(),hashlib.sha256(raw).hexdigest()]))
(root/'off-manifest.tsv').write_text('\n'.join(manifest)+'\n')
print('Wrote seven OFF sources, seven geometry oracles and manifest')
