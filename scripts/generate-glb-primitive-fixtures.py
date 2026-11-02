#!/usr/bin/env python3
"""CC0 native primitive controls and independent Python struct geometry oracles."""
import copy
import hashlib
import json
import struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]/'tests/fixtures/glb'

def glb(doc,binary,duplicate=False):
    text=json.dumps(doc,separators=(',',':')).encode()
    if duplicate:
        text=text.replace(b'"POSITION":0',b'"POSITION":999,"\\u0050OSITION":0',1)
    text+=b' '*(-len(text)%4)
    binary+=b'\0'*(-len(binary)%4)
    chunks=struct.pack('<II',len(text),0x4e4f534a)+text+struct.pack('<II',len(binary),0x004e4942)+binary
    return b'glTF'+struct.pack('<II',2,12+len(chunks))+chunks

binary=bytearray();views=[];accessors=[];meshes=[];triangles=[];bounds=[]
def accessor(values,kind='VEC3',component=5126,normalized=False,vertex=True):
    fmt={5121:'B',5123:'H',5125:'I',5126:'f'}[component]
    columns={'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4}[kind]
    flat=[v for row in values for v in row];payload=struct.pack('<'+fmt*len(flat),*flat)
    # Vertex records with small components have explicit four-byte stride.
    packed=struct.calcsize('<'+fmt)*columns;stride=(packed+3)//4*4 if vertex else packed
    if stride!=packed:
        payload=b''.join(payload[i:i+packed]+b'\0'*(stride-packed) for i in range(0,len(payload),packed))
    binary.extend(b'\0'*(-len(binary)%4));start=len(binary);binary.extend(payload)
    view=dict(buffer=0,byteOffset=start,byteLength=len(payload))
    if vertex and component!=5126:
        view.update(byteStride=stride)
    views.append(view)
    obj=dict(bufferView=len(views)-1,componentType=component,count=len(values),type=kind)
    if normalized:obj['normalized']=True
    accessors.append(obj);return len(accessors)-1

def add(name,positions,indices=None,mode=4,sparse=False,attributes=False):
    positions=[list(struct.unpack('<fff',struct.pack('<fff',*p))) for p in positions]
    p=accessor(positions)
    lo=[min(v[c] for v in positions) for c in range(3)];hi=[max(v[c] for v in positions) for c in range(3)]
    accessors[p].update(min=lo,max=hi)
    if sparse:
        accessors[p].pop('bufferView')
        iv=accessor([[1],[2],[3]],kind='SCALAR',component=5121,vertex=False)
        vv=accessor(positions[1:])
        accessors[p]['sparse']=dict(count=3,indices=dict(bufferView=accessors[iv]['bufferView'],componentType=5121),values=dict(bufferView=accessors[vv]['bufferView']))
    attrs={'POSITION':p}
    if attributes:
        attrs.update(NORMAL=accessor([[0.,0.,1.]]*len(positions)),
                     TANGENT=accessor([[1.,0.,0.,-1.]]*len(positions),kind='VEC4'),
                     TEXCOORD_0=accessor([[0,65535]]*len(positions),kind='VEC2',component=5123,normalized=True),
                     TEXCOORD_1=accessor([[0.25,0.5]]*len(positions),kind='VEC2'),
                     COLOR_0=accessor([[1.,0.5,0.,0.75]]*len(positions),kind='VEC4'),
                     _TEMPERATURE=accessor([[273.15]]*len(positions),kind='SCALAR'))
    primitive=dict(attributes=attrs,material=0)
    if mode!=4:primitive['mode']=mode
    if indices is not None:
        primitive['indices']=accessor([[i] for i in indices],kind='SCALAR',component=5123,vertex=False)
    meshes.append(dict(name=name,primitives=[primitive]))
    stream=indices if indices is not None else list(range(len(positions)))
    count=len(stream)//3 if mode==4 else len(stream)-2
    used=[]
    for i in range(count):
        offsets=[3*i,3*i+1,3*i+2] if mode==4 else ([i,i+1+i%2,i+2-i%2] if mode==5 else [i+1,i+2,0])
        corners=[stream[o] for o in offsets];coords=[positions[c] for c in corners];used+=coords
        triangles.append((len(meshes)-1,i,*corners,*(v for point in coords for v in point)))
    usedlo=[min(v[c] for v in used) for c in range(3)];usedhi=[max(v[c] for v in used) for c in range(3)]
    bounds.append((len(meshes)-1,*usedlo,*usedhi,*lo,*hi))

square=[[0,0,0],[1,0,0],[1,1,0],[0,1,0]]
add('indexed-unused-far-position',square+[[20,20,20]],[0,1,2,0,2,3],attributes=True)
add('nonindexed-triangles',[square[i] for i in [0,1,2,0,2,3]])
strip=[[0,0,0],[0,1,0],[1,0,0],[1,1,0],[2,0,0],[2,1,0]]
add('strip',strip,mode=5);add('indexed-strip',strip,list(range(6)),mode=5)
add('fan',square,mode=6);add('indexed-fan',square,list(range(4)),mode=6)
add('sparse-position',square,[0,1,2,0,2,3],sparse=True)
add('translated-float32',[[1e10,0,0],[1e10+2048,0,0],[1e10,2048,0]])
add('source-degenerate-preserved',square,[0,0,1])
doc=dict(asset=dict(version='2.0'),buffers=[dict(byteLength=len(binary))],bufferViews=views,accessors=accessors,meshes=meshes,materials=[dict(pbrMetallicRoughness=dict(baseColorFactor=[1.,0.5,0.25,1.]))])
(ROOT/'primitive-controls.glb').write_bytes(glb(doc,bytes(binary),duplicate=True))
(ROOT/'primitive-triangles.tsv').write_text(''.join('\t'.join(map(str,row))+'\n' for row in triangles))
(ROOT/'primitive-bounds.tsv').write_text(''.join('\t'.join(map(str,row))+'\n' for row in bounds))
# Original Box local-space geometry; node matrix is intentionally not applied.
b=(ROOT/'Box.glb').read_bytes();n=struct.unpack_from('<I',b,12)[0];j=json.loads(b[20:20+n]);start=20+n+8
p=j['meshes'][0]['primitives'][0]
def original_values(index):
    a=j['accessors'][index];v=j['bufferViews'][a['bufferView']];count={'SCALAR':1,'VEC3':3}[a['type']];fmt={5123:'H',5126:'f'}[a['componentType']];size=struct.calcsize('<'+fmt)
    return [struct.unpack_from('<'+fmt*count,b,start+v.get('byteOffset',0)+a.get('byteOffset',0)+r*v.get('byteStride',count*size)) for r in range(a['count'])]
positions=original_values(p['attributes']['POSITION']);indices=[v[0] for v in original_values(p['indices'])];original=[]
for i in range(len(indices)//3):
    corners=indices[3*i:3*i+3];original.append((0,i,*corners,*(v for c in corners for v in positions[c])))
(ROOT/'Box-triangles.tsv').write_text(''.join('\t'.join(map(str,row))+'\n' for row in original))

# Malformed contracts based on a small independent single-primitive GLB.
base_binary=struct.pack('<ffffffffffffHHH',0,0,0,1,0,0,1,1,0,0,1,0,0,1,2)
base=dict(asset=dict(version='2.0'),buffers=[dict(byteLength=len(base_binary))],bufferViews=[dict(buffer=0,byteLength=48),dict(buffer=0,byteOffset=48,byteLength=6)],accessors=[dict(bufferView=0,componentType=5126,count=4,type='VEC3',min=[0,0,0],max=[1,1,0]),dict(bufferView=1,componentType=5123,count=3,type='SCALAR')],meshes=[dict(primitives=[dict(attributes=dict(POSITION=0),indices=1)])])
negative=[]
def bad(name,change,blob=base_binary):
    d=copy.deepcopy(base);change(d);negative.append((name,glb(d,blob)))
def primitive(d):return d['meshes'][0]['primitives'][0]
bad('index-range',lambda d:None,base_binary[:-2]+struct.pack('<H',4))
bad('restart-u16',lambda d:None,base_binary[:-2]+struct.pack('<H',65535))
bad('float-indices',lambda d:d['accessors'][1].update(bufferView=0,componentType=5126))
bad('normalized-indices',lambda d:d['accessors'][1].update(normalized=True))
bad('index-count',lambda d:d['accessors'][1].update(count=2))
bad('position-type',lambda d:d['accessors'][0].update(type='VEC2'))
bad('missing-position',lambda d:primitive(d).update(attributes={}))
bad('missing-min',lambda d:d['accessors'][0].pop('min'))
bad('bounds-mismatch',lambda d:d['accessors'][0].update(max=[2,1,0]))
bad('bounds-length',lambda d:d['accessors'][0].update(max=[1,1]))
bad('bounds-order',lambda d:d['accessors'][0].update(min=[2,0,0]))
bad('bounds-infinite-f32',lambda d:d['accessors'][0].update(max=[1e300,1,0]))
bad('position-nan',lambda d:None,struct.pack('<I',0x7fc00000)+base_binary[4:])
bad('point-mode',lambda d:primitive(d).update(mode=0))
bad('line-mode',lambda d:primitive(d).update(mode=1))
bad('invalid-mode',lambda d:primitive(d).update(mode=7))
bad('morph-target',lambda d:primitive(d).update(targets=[dict(POSITION=0)]))
bad('material-range',lambda d:primitive(d).update(material=0))
bad('index-vertex-target',lambda d:d['bufferViews'][1].update(target=34962))
bad('position-index-target',lambda d:d['bufferViews'][0].update(target=34963))
bad('index-stride-no-target',lambda d:d['bufferViews'][1].update(byteStride=4))
def extra_attribute(d,name,kind='VEC3',count=4):
    d['bufferViews'][0]['byteStride']=12
    d['accessors'].append(dict(bufferView=0,componentType=5126,count=count,type=kind))
    primitive(d)['attributes'][name]=len(d['accessors'])-1
bad('attribute-count',lambda d:extra_attribute(d,'_CUSTOM',count=3))
bad('normal-unit',lambda d:extra_attribute(d,'NORMAL'))
bad('texcoord-set-gap',lambda d:extra_attribute(d,'TEXCOORD_1',kind='VEC2'))
bad('texcoord-set-leading-zero',lambda d:extra_attribute(d,'TEXCOORD_01',kind='VEC2'))
bad('attribute-semantic',lambda d:primitive(d)['attributes'].update(UNKNOWN=0))
bad('shared-view-stride',lambda d:primitive(d)['attributes'].update(_CUSTOM=0))
bad('attribute-limit',lambda d:primitive(d)['attributes'].update({f'_C{i}':0 for i in range(64)}))
bad('attribute-name-limit',lambda d:primitive(d)['attributes'].update({'_'+'x'*4096:0}))
# Reserved U8 restart value must reject even when it is within POSITION.count.
d=copy.deepcopy(base);d['buffers'][0]['byteLength']=3075
d['bufferViews'][0]['byteLength']=3072;d['bufferViews'][1].update(byteOffset=3072,byteLength=3)
d['accessors'][0].update(count=256,min=[0,0,0],max=[0,0,0]);d['accessors'][1]['componentType']=5121
negative.append(('restart-u8-within-position-range',glb(d,b'\0'*3072+bytes([0,1,255]))))
(ROOT/'primitive-invalid.tsv').write_text(''.join(f'{name}\t{data.hex()}\n' for name,data in negative))
outputs=['primitive-controls.glb','primitive-triangles.tsv','primitive-bounds.tsv','Box-triangles.tsv','primitive-invalid.tsv']
(ROOT/'primitive-manifest.json').write_text(json.dumps(dict(meshes=len(meshes),triangles=len(triangles),original_box_triangles=len(original),invalid_contracts=len(negative),sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in outputs}),indent=2)+'\n')
print(f'{len(meshes)} meshes, {len(triangles)} authored / {len(original)} original triangles, {len(negative)} malformed contracts')
