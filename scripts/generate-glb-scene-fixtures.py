#!/usr/bin/env python3
"""CC0 hierarchy controls: recursive Decimal source-pose oracle, no dense meshes."""
from decimal import Decimal as D,getcontext
import hashlib,json,struct
from pathlib import Path
getcontext().prec=80
ROOT=Path(__file__).resolve().parents[1]/'tests/fixtures/glb'
def encode(doc,binary=None):
    text=json.dumps(doc,separators=(',',':')).encode();text+=b' '*(-len(text)%4)
    chunks=struct.pack('<II',len(text),0x4e4f534a)+text
    if binary is not None:chunks+=struct.pack('<II',len(binary),0x004e4942)+binary
    return b'glTF'+struct.pack('<II',2,12+len(chunks))+chunks
b=(ROOT/'primitive-controls.glb').read_bytes();n=struct.unpack_from('<I',b,12)[0];doc=json.loads(b[20:20+n]);binary=b[20+n+8:]
nodes=[dict(translation=[10,0,0],children=[3,1]),dict(rotation=[0,0,0.6,0.8],scale=[2,1,1],mesh=0,children=[2]),dict(translation=[0,3,0],mesh=0),dict(translation=[-2,0,0],scale=[-1,1,1],mesh=1),dict(translation=[100,0,0],mesh=0),dict(scale=[2,1,3],children=[6]),dict(rotation=[0,0,0.6,0.8],mesh=2)]
doc.update(nodes=nodes,scenes=[dict(nodes=[0]),dict(nodes=[4,0]),dict(nodes=[5]),{}],scene=1)
(ROOT/'scene-controls.glb').write_bytes(encode(doc,binary))
parents={child:i for i,node in enumerate(nodes) for child in node.get('children',[])}
def multiply(a,b):
    w,x,y,z=a;v,i,j,k=b
    return (w*v-x*i-y*j-z*k,w*i+x*v+y*k-z*j,w*j-x*k+y*v+z*i,w*k+x*j-y*i+z*v)
def local(node,p):
    p=list(map(lambda x:D(str(x)),p));s=list(map(lambda x:D(str(x)),node.get('scale',[1,1,1])));t=list(map(lambda x:D(str(x)),node.get('translation',[0,0,0])));x,y,z,w=map(lambda x:D(str(x)),node.get('rotation',[0,0,0,1]))
    q=(w,x,y,z);v=multiply(multiply(q,(D(0),*(p[c]*s[c] for c in range(3)))),(w,-x,-y,-z))
    return [v[c+1]+t[c] for c in range(3)]
def world(index,p):
    p=local(nodes[index],p)
    return world(parents[index],p) if index in parents else p
geometry={}
for line in (ROOT/'primitive-triangles.tsv').read_text().splitlines():
    v=line.split('\t');geometry.setdefault(int(v[0]),[]).append((int(v[1]),list(map(D,v[5:]))))
order=[];triangles=[]
def visit(scene,index):
    if 'mesh' in nodes[index]:
        mesh=nodes[index]['mesh'];order.append((scene,index,mesh))
        for at,coords in geometry[mesh]:
            result=[]
            for c in range(3):result.extend(float(v) for v in world(index,coords[c*3:c*3+3]))
            triangles.append((scene,index,at,*result))
    for child in nodes[index].get('children',[]):visit(scene,child)
for index,scene in enumerate(doc['scenes']):
    for root in scene.get('nodes',[]):visit(index,root)
(ROOT/'scene-instances.tsv').write_text(''.join('\t'.join(map(str,row))+'\n' for row in order))
(ROOT/'scene-triangles.tsv').write_text(''.join('\t'.join(map(str,row))+'\n' for row in triangles))
invalid=[]
def bad(name,doc):invalid.append((name,encode(dict(asset=dict(version='2.0'),**doc))))
bad('cycle',dict(nodes=[dict(children=[1]),dict(children=[0])],scenes=[dict(nodes=[0])]))
bad('self-cycle',dict(nodes=[dict(children=[0])],scenes=[dict(nodes=[0])]))
bad('multiple-parent',dict(nodes=[dict(children=[2]),dict(children=[2]),{}],scenes=[dict(nodes=[0,1])]))
bad('duplicate-child',dict(nodes=[dict(children=[1,1]),{}],scenes=[dict(nodes=[0])]))
bad('child-root',dict(nodes=[dict(children=[1]),{}],scenes=[dict(nodes=[1])]))
bad('root-range',dict(nodes=[{}],scenes=[dict(nodes=[1])]))
bad('duplicate-root',dict(nodes=[{}],scenes=[dict(nodes=[0,0])]))
bad('empty-roots',dict(nodes=[{}],scenes=[dict(nodes=[])]))
bad('empty-children',dict(nodes=[dict(children=[])],scenes=[dict(nodes=[0])]))
bad('children-type',dict(nodes=[dict(children={})],scenes=[dict(nodes=[0])]))
bad('child-range',dict(nodes=[dict(children=[1])],scenes=[dict(nodes=[0])]))
bad('child-negative',dict(nodes=[dict(children=[-1])],scenes=[dict(nodes=[0])]))
bad('node-type',dict(nodes=[[]],scenes=[dict(nodes=[0])]))
bad('mesh-range',dict(nodes=[dict(mesh=0)],scenes=[dict(nodes=[0])]))
bad('selected-skin',dict(nodes=[dict(mesh=0,skin=0)],meshes=[{}],scenes=[dict(nodes=[0])]))
bad('selected-weights',dict(nodes=[dict(mesh=0,weights=[0])],meshes=[{}],scenes=[dict(nodes=[0])]))
bad('unselected-invalid-root',dict(nodes=[{},{}],scenes=[dict(nodes=[0]),dict(nodes=[2])]))
bad('default-range',dict(nodes=[{}],scenes=[dict(nodes=[0])],scene=1))
bad('empty-scenes',dict(scenes=[]))
bad('default-without-scenes',dict(scene=0))
bad('no-scenes',{})
bad('empty-declared-nodes',dict(nodes=[],scenes=[{}]))
bad('scene-type',dict(scenes=[[]]))
(ROOT/'scene-invalid.tsv').write_text(''.join(f'{name}\t{b.hex()}\n' for name,b in invalid))
outputs=['scene-controls.glb','scene-instances.tsv','scene-triangles.tsv','scene-invalid.tsv']
(ROOT/'scene-manifest.json').write_text(json.dumps(dict(nodes=len(nodes),scenes=len(doc['scenes']),instances=len(order),triangles=len(triangles),invalid_contracts=len(invalid),sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in outputs}),indent=2)+'\n')
print(f'{len(nodes)} nodes,4 scenes,{len(order)} instances,{len(triangles)} Decimal triangles,23 invalid contracts')
