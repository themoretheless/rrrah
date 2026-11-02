#!/usr/bin/env python3
"""CC0 authored embedded GLB accessor controls; Python struct sample oracle."""
import copy
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]/'tests/fixtures/glb'
FORMATS = {5120:'b',5121:'B',5122:'h',5123:'H',5125:'I',5126:'f'}
SHAPES = {'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4,'MAT2':4,'MAT3':9,'MAT4':16}

def glb(document, binary):
    text = json.dumps(document, separators=(',',':')).encode()
    text += b' '*(-len(text)%4)
    binary += b'\0'*(-len(binary)%4)
    chunks = struct.pack('<II',len(text),0x4e4f534a)+text
    if binary:
        chunks += struct.pack('<II',len(binary),0x004e4942)+binary
    return b'glTF'+struct.pack('<II',2,12+len(chunks))+chunks


def normalized(value, component, enabled):
    if not enabled:
        return value
    if component in (5120,5122):
        return max(-1.,value/(127 if component==5120 else 32767))
    return value/(255 if component==5121 else 65535)


binary = bytearray()
views = []
accessors = []
oracle = []

def view(payload, **extra):
    binary.extend(b'\0'*(-len(binary)%4))
    start = len(binary)
    binary.extend(payload)
    views.append(dict(buffer=0,byteOffset=start,byteLength=len(payload),**extra))
    return len(views)-1


def add(component, shape, rows, norm=False, stride=None, offset=0):
    size = struct.calcsize('<'+FORMATS[component])
    matrix = int(shape[-1]) if shape.startswith('MAT') else None
    padded_column = ((matrix*size+3)//4)*4 if matrix else 0
    packed = matrix*padded_column if matrix else SHAPES[shape]*size
    stride = stride or packed
    payload = bytearray([0xa5]*offset)
    for row_index,row in enumerate(rows):
        block = bytearray([0xa5]*stride)
        for column,value in enumerate(row):
            at = (column//matrix)*padded_column+(column%matrix)*size if matrix else column*size
            struct.pack_into('<'+FORMATS[component],block,at,value)
            actual = struct.unpack_from('<'+FORMATS[component],block,at)[0]
            oracle.append((len(accessors),row_index,column,normalized(actual,component,norm)))
        # Only the final actual component must be present: matrix tail padding
        # and the unused tail of the final interleaved row can be omitted.
        if row_index==len(rows)-1:
            block=block[:at+size]
        payload.extend(block)
    extra = dict(byteStride=stride) if stride!=packed else {}
    index=view(payload,**extra)
    accessor=dict(bufferView=index,componentType=component,count=len(rows),type=shape)
    if offset:
        accessor['byteOffset']=offset
    if norm:
        accessor['normalized']=True
    accessors.append(accessor)


for component in FORMATS:
    extremes = {5120:[-128,-127,0,127],5121:[0,1,254,255],5122:[-32768,-32767,0,32767],5123:[0,1,65534,65535],5125:[0,2**24+1,2**31,2**32-1],5126:[-0.,2**-149,-2**-126,1.25]}[component]
    add(component,'SCALAR',[[v] for v in extremes])
    if component not in (5125,5126):
        add(component,'SCALAR',[[v] for v in extremes],norm=True)
    for shape,count in SHAPES.items():
        if shape=='SCALAR' or component==5125:
            continue  # UNSIGNED_INT has only primitive-index usage in core glTF.
        rows=[[row*count+column+1 for column in range(count)] for row in range(3)]
        add(component,shape,rows)
add(5126,'VEC3',[[1.25,2.5,3.75],[-1.,-2.,-3.],[0.,4.,8.]],stride=20,offset=4)
add(5121,'VEC4',[[0,127,255,1],[254,32,64,128]],norm=True,stride=12,offset=4)

for index_component in (5121,5123,5125):
    indices=[0,3,6]
    index_payload=b'\0'*4+struct.pack('<'+FORMATS[index_component]*3,*indices)
    iv=view(index_payload)
    values=[[-32768,32767],[100,-100],[0,1234]]
    vv=view(b'\0'*4+struct.pack('<hhhhhh',*(v for row in values for v in row)))
    accessor=dict(componentType=5122,count=7,type='VEC2',normalized=True,sparse=dict(count=3,indices=dict(bufferView=iv,byteOffset=4,componentType=index_component),values=dict(bufferView=vv,byteOffset=4)))
    for row in range(7):
        for column in range(2):
            value=values[indices.index(row)][column] if row in indices else 0
            oracle.append((len(accessors),row,column,normalized(value,5122,True)))
    accessors.append(accessor)

# Sparse substitution over an interleaved base, without dense expansion.
add(5126,'VEC3',[[1.,2.,3.],[4.,5.,6.],[7.,8.,9.]],stride=16)
iv=view(bytes([1]));vv=view(struct.pack('<fff',-1.,-2.,-3.))
accessors[-1]['sparse']=dict(count=1,indices=dict(bufferView=iv,componentType=5121),values=dict(bufferView=vv))
oracle=[(a,r,c,(-1.,-2.,-3.)[c] if a==len(accessors)-1 and r==1 else v) for a,r,c,v in oracle]
# Sparse MAT3/U8 uses column padding, omitting only the final column tail.
iv=view(bytes([0,2]));payload=bytearray()
for row in range(2):
    block=bytearray([0xee]*12)
    for component in range(9):
        block[(component//3)*4+component%3]=row*10+component+1
    payload.extend(block if row==0 else block[:11])
vv=view(payload)
accessors.append(dict(componentType=5121,count=3,type='MAT3',sparse=dict(count=2,indices=dict(bufferView=iv,componentType=5121),values=dict(bufferView=vv))))
for row in range(3):
    for column in range(9):
        oracle.append((len(accessors)-1,row,column,(0 if row==1 else (row//2)*10+column+1)))
# No bufferView/sparse means implicit zero storage; byteOffset must be absent.
accessors.append(dict(componentType=5126,count=3,type='VEC3'))
oracle.extend((len(accessors)-1,r,c,0.) for r in range(3) for c in range(3))
document=dict(asset=dict(version='2.0'),buffers=[dict(byteLength=len(binary))],bufferViews=views,accessors=accessors)
(ROOT/'accessor-controls.glb').write_bytes(glb(document,bytes(binary)))
(ROOT/'accessor-controls.tsv').write_text(''.join(f'{a}\t{r}\t{c}\t{v!r}\n' for a,r,c,v in oracle))

# Original unmodified Box, independently read through its declared views.
b=(ROOT/'Box.glb').read_bytes();n=struct.unpack_from('<I',b,12)[0]
j=json.loads(b[20:20+n]);start=20+n+8;rows=[]
for a,accessor in enumerate(j['accessors']):
    v=j['bufferViews'][accessor['bufferView']];count=SHAPES[accessor['type']];fmt=FORMATS[accessor['componentType']];size=struct.calcsize('<'+fmt)
    for r in range(accessor['count']):
        for c in range(count):
            at=start+v.get('byteOffset',0)+accessor.get('byteOffset',0)+r*v.get('byteStride',count*size)+c*size
            value=struct.unpack_from('<'+fmt,b,at)[0]
            rows.append(f'{a}\t{r}\t{c}\t{value!r}\n')
(ROOT/'Box-accessors.tsv').write_text(''.join(rows))

# Bounded authored malformed layouts. These are contract controls, not an
# independent glTF validator. The expected rejection reason is recorded by name.
base=dict(asset=dict(version='2.0'),buffers=[dict(byteLength=16)],bufferViews=[dict(buffer=0,byteLength=16)],accessors=[dict(bufferView=0,componentType=5126,count=1,type='VEC3')])
negatives=[]
def negative(name, mutate, blob=b'\0'*16):
    d=copy.deepcopy(base);mutate(d);negatives.append((name,glb(d,blob)))
negative('view-overflow',lambda d:d['bufferViews'][0].update(byteOffset=2**64-1))
negative('accessor-span-overflow',lambda d:d['accessors'][0].update(byteOffset=2**64-4))
negative('accessor-oob',lambda d:d['accessors'][0].update(count=2))
negative('component-alignment',lambda d:d['accessors'][0].update(byteOffset=1))
negative('absolute-alignment',lambda d:d['bufferViews'][0].update(byteOffset=1,byteLength=15))
negative('matrix-relative-alignment',lambda d:d['accessors'][0].update(componentType=5121,type='MAT2',byteOffset=1))
negative('matrix-absolute-alignment',lambda d:(d['bufferViews'][0].update(byteOffset=1,byteLength=15),d['accessors'][0].update(componentType=5121,type='MAT2')))
negative('invalid-stride',lambda d:d['bufferViews'][0].update(byteStride=6))
negative('short-stride',lambda d:d['bufferViews'][0].update(byteStride=8))
negative('long-stride',lambda d:d['bufferViews'][0].update(byteStride=256))
negative('index-target-stride',lambda d:d['bufferViews'][0].update(byteStride=16,target=34963))
negative('invalid-target',lambda d:d['bufferViews'][0].update(target=123))
negative('invalid-component',lambda d:d['accessors'][0].update(componentType=5124))
negative('invalid-shape',lambda d:d['accessors'][0].update(type='VEC5'))
negative('zero-count',lambda d:d['accessors'][0].update(count=0))
negative('excess-count',lambda d:d['accessors'][0].update(count=16_777_217))
negative('fractional-count',lambda d:d['accessors'][0].update(count=1.5))
negative('vertex-packed-unaligned',lambda d:(d['bufferViews'][0].update(target=34962),d['accessors'][0].update(componentType=5121,count=2)))
negative('vertex-small-component-needs-stride',lambda d:(d['bufferViews'][0].update(target=34962),d['accessors'][0].update(componentType=5121,type='VEC4')))
negative('float-normalized',lambda d:d['accessors'][0].update(normalized=True))
negative('uint-normalized',lambda d:d['accessors'][0].update(componentType=5125,normalized=True))
negative('invalid-normalized',lambda d:d['accessors'][0].update(normalized=1))
negative('missing-view-offset',lambda d:(d['accessors'][0].pop('bufferView'),d['accessors'][0].update(byteOffset=0)))
negative('view-out-of-range',lambda d:d['accessors'][0].update(bufferView=1))
negative('external-buffer',lambda d:d['buffers'][0].update(uri='unread.bin'))
negative('non-embedded-buffer',lambda d:d['bufferViews'][0].update(buffer=1))
negative('missing-bin',lambda d:None,blob=b'')
negative('buffer-length-too-large',lambda d:d['buffers'][0].update(byteLength=17))
negative('buffer-padding-too-long',lambda d:d['buffers'][0].update(byteLength=12))
negative('nonzero-padding',lambda d:d['buffers'][0].update(byteLength=15),blob=b'\0'*15+b'\x01')
# Sparse controls exercise both encoded index bytes and metadata.
sparse_base=dict(asset=dict(version='2.0'),buffers=[dict(byteLength=52)],bufferViews=[dict(buffer=0,byteLength=24),dict(buffer=0,byteOffset=24,byteLength=2),dict(buffer=0,byteOffset=28,byteLength=24)],accessors=[dict(bufferView=0,componentType=5126,count=2,type='VEC3',sparse=dict(count=2,indices=dict(bufferView=1,componentType=5121),values=dict(bufferView=2)))])
sparse_binary=b'\0'*24+bytes([0,1])+b'\0'*26
def bad_sparse(name,mutate,blob=sparse_binary):
    d=copy.deepcopy(sparse_base);mutate(d);negatives.append((name,glb(d,blob)))
bad_sparse('sparse-duplicate',lambda d:None,blob=b'\0'*24+bytes([1,1])+b'\0'*26)
bad_sparse('sparse-reversed',lambda d:None,blob=b'\0'*24+bytes([1,0])+b'\0'*26)
bad_sparse('sparse-out-of-range',lambda d:None,blob=b'\0'*24+bytes([0,2])+b'\0'*26)
bad_sparse('sparse-zero-count',lambda d:d['accessors'][0]['sparse'].update(count=0))
bad_sparse('sparse-excess-count',lambda d:d['accessors'][0]['sparse'].update(count=3))
bad_sparse('sparse-signed-indices',lambda d:d['accessors'][0]['sparse']['indices'].update(componentType=5120))
bad_sparse('sparse-index-stride',lambda d:d['bufferViews'][1].update(byteStride=4))
bad_sparse('sparse-value-stride',lambda d:d['bufferViews'][2].update(byteStride=12))
bad_sparse('sparse-index-target',lambda d:d['bufferViews'][1].update(target=34963))
bad_sparse('sparse-value-target',lambda d:d['bufferViews'][2].update(target=34962))
bad_sparse('sparse-value-alignment',lambda d:d['accessors'][0]['sparse']['values'].update(byteOffset=2))
bad_sparse('sparse-value-short',lambda d:d['bufferViews'][2].update(byteLength=20))
bad_sparse('sparse-index-alignment',lambda d:d['accessors'][0]['sparse']['indices'].update(componentType=5123,byteOffset=1))
(ROOT/'accessor-invalid.tsv').write_text(''.join(f'{name}\t{data.hex()}\n' for name,data in negatives))
outputs=['accessor-controls.glb','accessor-controls.tsv','Box-accessors.tsv','accessor-invalid.tsv']
(ROOT/'accessor-manifest.json').write_text(json.dumps(dict(authored_accessors=len(accessors),authored_samples=len(oracle),original_box_samples=len(rows),invalid_layouts=len(negatives),sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in outputs}),indent=2)+'\n')
print(f'{len(accessors)} authored accessors, {len(oracle)} samples; {len(rows)} original samples; {len(negatives)} invalid layouts')
