#!/usr/bin/env python3
"""CC0 affine controls: Decimal quaternion products / Gauss-Jordan normal oracle."""
from decimal import Decimal as D, getcontext
import hashlib
import json
from pathlib import Path
import struct
getcontext().prec=80
ROOT=Path(__file__).resolve().parents[1]/'tests/fixtures/glb'
IDENTITY=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
nodes=[{},dict(translation=[10,20,30],rotation=[0,0,0.6,0.8],scale=[2,3,4]),dict(scale=[-2,3,4]),dict(scale=[0,2,3]),dict(scale=[0,0,0]),dict(matrix=[-2,0,0,0,0,3,0,0,0,0,4,0,3,4,5,1]),dict(translation=[-1,2,-3],rotation=[0.5,0.5,0.5,0.5],scale=[1,2,3]),dict(translation=[4,-5,6],rotation=[0.5,-0.5,0.5,-0.5],scale=[-1,2,-3])]
def glb(doc):
    text=json.dumps(doc,separators=(',',':')).encode();text+=b' '*(-len(text)%4)
    chunks=struct.pack('<II',len(text),0x4e4f534a)+text
    return b'glTF'+struct.pack('<II',2,12+len(chunks))+chunks
(ROOT/'transform-controls.glb').write_bytes(glb(dict(asset=dict(version='2.0'),nodes=nodes)))
def quaternion(a,b):
    w,x,y,z=a;v,i,j,k=b
    return (w*v-x*i-y*j-z*k,w*i+x*v+y*k-z*j,w*j-x*k+y*v+z*i,w*k+x*j-y*i+z*v)
def point(node,point):
    p=[D(str(v)) for v in point]
    if 'matrix' in node:
        m=[D(str(v)) for v in node['matrix']]
        return [sum(m[4*c+r]*p[c] for c in range(3))+m[12+r] for r in range(3)]
    s=[D(str(v)) for v in node.get('scale',[1,1,1])];t=[D(str(v)) for v in node.get('translation',[0,0,0])]
    x,y,z,w=[D(str(v)) for v in node.get('rotation',[0,0,0,1])]
    q=(w,x,y,z);conjugate=(w,-x,-y,-z)
    rotated=quaternion(quaternion(q,(D(0),*(p[i]*s[i] for i in range(3)))),conjugate)
    return [rotated[i+1]+t[i] for i in range(3)]
rows=[]
for index,node in enumerate(nodes):
    for p in [[0,0,0],[1,2,3],[-3,2,-1],[0.125,-0.5,4],[1e10,0,0]]:
        rows.append((index,*p,*(float(v) for v in point(node,p))))
(ROOT/'transform-points.tsv').write_text(''.join('\t'.join(map(str,row))+'\n' for row in rows))
def linear(node):
    zero=point(node,[0,0,0]);cols=[[v-zero[r] for r,v in enumerate(point(node,[int(c==i) for c in range(3)]))] for i in range(3)]
    return [[cols[c][r] for c in range(3)] for r in range(3)]
def inverse(matrix):
    a=[list(row)+[D(int(r==c)) for c in range(3)] for r,row in enumerate(matrix)]
    for c in range(3):
        pivot=next(r for r in range(c,3) if a[r][c]);a[c],a[pivot]=a[pivot],a[c]
        divisor=a[c][c];a[c]=[v/divisor for v in a[c]]
        for r in range(3):
            if r!=c:
                factor=a[r][c];a[r]=[v-factor*p for v,p in zip(a[r],a[c])]
    return [row[3:] for row in a]
rows=[]
for index in [0,1,2,5,6,7,100]:
    matrix=linear(nodes[index]) if index!=100 else [[D([2,1,3][r])*v for v in row] for r,row in enumerate(linear(nodes[1]))]
    inv=inverse(matrix)
    for n in [[1,0,0],[0,1,0],[0,0,1],[1,2,3],[-1,-2,4]]:
        out=[sum(inv[c][r]*D(n[c]) for c in range(3)) for r in range(3)]
        length=sum(v*v for v in out).sqrt();rows.append((index,*n,*(float(v/length) for v in out)))
(ROOT/'transform-normals.tsv').write_text(''.join('\t'.join(map(str,row))+'\n' for row in rows))
# Unmodified Box matrix read from its own GLB source, applied to local oracle vertices.
b=(ROOT/'Box.glb').read_bytes();length=struct.unpack_from('<I',b,12)[0];j=json.loads(b[20:20+length]);out=[]
for line in (ROOT/'Box-triangles.tsv').read_text().splitlines():
    v=list(map(float,line.split('\t')));coords=[]
    for c in range(3):coords.extend(float(x) for x in point(j['nodes'][0],v[5+c*3:8+c*3]))
    out.append((int(v[1]),*coords))
(ROOT/'Box-node-triangles.tsv').write_text(''.join('\t'.join(map(str,row))+'\n' for row in out))
invalid=[]
def bad(name,node):invalid.append((name,glb(dict(asset=dict(version='2.0'),nodes=[node]))))
bad('matrix-plus-translation',dict(matrix=IDENTITY,translation=[0,0,0]))
bad('matrix-plus-null-scale',dict(matrix=IDENTITY,scale=None))
bad('matrix-plus-rotation',dict(matrix=IDENTITY,rotation=[0,0,0,1]))
m=IDENTITY.copy();m[4]=0.2;bad('local-shear',dict(matrix=m))
m=IDENTITY.copy();m[3]=0.1;bad('projective',dict(matrix=m))
m=IDENTITY.copy();m[15]=0;bad('non-affine-last-word',dict(matrix=m))
bad('short-matrix',dict(matrix=IDENTITY[:-1]));bad('long-matrix',dict(matrix=IDENTITY+[0]))
bad('short-translation',dict(translation=[0,0]));bad('long-translation',dict(translation=[0,0,0,0]))
bad('null-translation',dict(translation=None));bad('empty-scale',dict(scale=[]))
bad('null-scale',dict(scale=None));bad('short-rotation',dict(rotation=[0,0,1]))
bad('long-rotation',dict(rotation=[0,0,0,0,1]));bad('zero-quaternion',dict(rotation=[0,0,0,0]))
bad('nonunit-quaternion',dict(rotation=[0,0,0,2]));bad('huge-quaternion',dict(rotation=[1e300,0,0,0]))
bad('not-object',[]);bad('nonnumeric-scale',dict(scale=[1,True,1]))
(ROOT/'transform-invalid.tsv').write_text(''.join(f'{name}\t{b.hex()}\n' for name,b in invalid))
outputs=['transform-controls.glb','transform-points.tsv','transform-normals.tsv','Box-node-triangles.tsv','transform-invalid.tsv']
(ROOT/'transform-manifest.json').write_text(json.dumps(dict(nodes=len(nodes),points=40,normals=35,original_box_triangles=len(out),invalid_nodes=len(invalid),sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in outputs}),indent=2)+'\n')
print(f'{len(nodes)} nodes,40 Decimal points,35 Gauss-Jordan normals,12 original Box triangles,20 invalid nodes')
