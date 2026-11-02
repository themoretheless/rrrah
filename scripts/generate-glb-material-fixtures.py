#!/usr/bin/env python3
"""CC0 material controls and independent 80-digit Decimal point-texel oracle."""
from decimal import Decimal as D,getcontext
import copy,hashlib,json,struct,zlib
from pathlib import Path
getcontext().prec=80
ROOT=Path(__file__).resolve().parents[1]/'tests/fixtures/glb'
def encode(doc,binary=None):
    text=json.dumps(doc,separators=(',',':')).encode();text+=b' '*(-len(text)%4);chunks=struct.pack('<II',len(text),0x4e4f534a)+text
    if binary is not None:
        binary+=b'\0'*(-len(binary)%4);chunks+=struct.pack('<II',len(binary),0x004e4942)+binary
    return b'glTF'+struct.pack('<II',2,12+len(chunks))+chunks
b=(ROOT/'primitive-controls.glb').read_bytes();n=struct.unpack_from('<I',b,12)[0];doc=json.loads(b[20:20+n]);binary=bytearray(b[20+n+8:])
def chunk(kind,payload):return struct.pack('>I',len(payload))+kind+payload+struct.pack('>I',zlib.crc32(kind+payload)&0xffffffff)
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(bytes([0,128,64,192,128])))+chunk(b'IEND',b'')
binary.extend(b'\0'*(-len(binary)%4));start=len(binary);binary.extend(png);doc['bufferViews'].append(dict(buffer=0,byteOffset=start,byteLength=len(png)));image_view=len(doc['bufferViews'])-1
doc['buffers'][0]['byteLength']=len(binary);doc['images']=[dict(bufferView=image_view,mimeType='image/png'),dict(uri='textures/unfetched.png',mimeType='IMAGE/PNG')];doc['samplers']=[dict(magFilter=9729,minFilter=9987,wrapS=33071,wrapT=33648),{}];doc['textures']=[dict(source=0,sampler=0),dict(source=1)]
full=dict(pbrMetallicRoughness=dict(baseColorFactor=[0.25,0.5,0.75,0.5],metallicFactor=0.8,roughnessFactor=0.3,baseColorTexture=dict(index=0),metallicRoughnessTexture=dict(index=0)),emissiveFactor=[0.4,0.2,0.1],emissiveTexture=dict(index=0),normalTexture=dict(index=0,texCoord=1,scale=-2),occlusionTexture=dict(index=0,strength=0.25),alphaMode='BLEND',alphaCutoff=0.9,doubleSided=True)
materials=[{},full]
for mode,cutoff in [('MASK',1.5),('MASK',0.5),('BLEND',10),('OPAQUE',1.5)]:
    material=copy.deepcopy(full);material.update(alphaMode=mode,alphaCutoff=cutoff);materials.append(material)
materials.append(dict(pbrMetallicRoughness=dict(baseColorFactor=[0.25,0.5,0.75,0.125],metallicFactor=0.5,roughnessFactor=0.6),emissiveFactor=[0.1,0.2,0.3],alphaMode='BLEND'))
m=copy.deepcopy(full);m['normalTexture']['scale']=0;m['occlusionTexture']['strength']=0;materials.append(m)
doc['materials']=materials;doc['meshes'][0]['primitives'][0]['material']=1
(ROOT/'material-controls.glb').write_bytes(encode(doc,bytes(binary)))
samples=[
[[.5,.25,.75,.5],[.91,.6,.4,.77],[.75,.25,1,.1],[.2,.9,.8,.7],[.5,.25,.75,.4],[2,-1,.5,.25]],
[[.04045,.04046,1,1],[.1,0,1,.2],[.5,.5,1,0],[1,0,0,0],[1,.04045,0,1],[1,1,1,1]],
[[0,1,.5,.25],[0,1,0,1],[1,.5,.75,1],[0,1,1,1],[.25,.5,1,0],[.5,.25,2,.5]],
[[1,1,1,1],[1,.5,.5,1],[.5,.75,1,.1],[.5,1,0,0],[.8,.9,1,0],[1,1,1,1]]]
(ROOT/'material-samples.tsv').write_text(''.join('\t'.join(map(str,[i,*(v for slot in s for v in slot)]))+'\n' for i,s in enumerate(samples)))
def decimal(value):return D(str(value))
def srgb(v):return v/D('12.92') if v<=D('.04045') else ((v+D('.055'))/D('1.055'))**D('2.4')
rows=[];parameters=[]
for index,m in enumerate(materials):
    p=m.get('pbrMetallicRoughness',{});base=list(map(decimal,p.get('baseColorFactor',[1]*4)));metal=decimal(p.get('metallicFactor',1));rough=decimal(p.get('roughnessFactor',1));emissive=list(map(decimal,m.get('emissiveFactor',[0]*3)));mode=m.get('alphaMode','OPAQUE');cutoff=decimal(m.get('alphaCutoff',.5))
    parameters.append([index,*map(float,base),float(metal),float(rough),*map(float,emissive),['OPAQUE','MASK','BLEND'].index(mode),float(cutoff),int(m.get('doubleSided',False)),m.get('normalTexture',{}).get('scale',0),m.get('occlusionTexture',{}).get('strength',1)])
    for sample_index,slots in enumerate(samples):
        b,mr,n,o,e,v=[list(map(decimal,slot)) for slot in slots];v=[min(D(1),max(D(0),x)) for x in v]
        if 'baseColorTexture' not in p:b=[D(1)]*4
        if 'metallicRoughnessTexture' not in p:mr=[D(1)]*4
        if 'emissiveTexture' not in m:e=[D(1)]*4
        rgb=[base[c]*srgb(b[c])*v[c] for c in range(3)];emission=[emissive[c]*srgb(e[c]) for c in range(3)]
        raw=base[3]*b[3]*v[3];discard=mode=='MASK' and raw<cutoff;alpha=D(1) if mode=='OPAQUE' or (mode=='MASK' and not discard) else (D(0) if mode=='MASK' else raw)
        occlusion=D(1)+decimal(m['occlusionTexture'].get('strength',1))*(o[0]-1) if 'occlusionTexture' in m else D(1)
        normal=[D(0)]*3
        if 'normalTexture' in m:
            scale=decimal(m['normalTexture'].get('scale',1));normal=[(2*n[0]-1)*scale,(2*n[1]-1)*scale,2*n[2]-1];length=sum(x*x for x in normal).sqrt();normal=[x/length for x in normal]
        rows.append([index,sample_index,*map(float,rgb),float(alpha),int(discard),float(metal*mr[2]),float(rough*mr[1]),*map(float,emission),float(occlusion),int('normalTexture' in m),*map(float,normal)])
(ROOT/'material-surfaces.tsv').write_text(''.join('\t'.join(map(str,row))+'\n' for row in rows));(ROOT/'material-parameters.tsv').write_text(''.join('\t'.join(map(str,row))+'\n' for row in parameters))
invalid=[]
negative_base=dict(asset=dict(version='2.0'),buffers=copy.deepcopy(doc['buffers']),bufferViews=[copy.deepcopy(doc['bufferViews'][image_view])],images=[dict(bufferView=0,mimeType='image/png')],textures=[dict(source=0,sampler=0)],samplers=[copy.deepcopy(doc['samplers'][0])],materials=[{},copy.deepcopy(full)])
def bad(name,change):
    d=copy.deepcopy(negative_base);change(d);invalid.append((name,json.dumps(d,separators=(',',':')).encode()))
def mat(d):return d['materials'][1]
def pbr(d):return mat(d)['pbrMetallicRoughness']
bad('factor-negative',lambda d:pbr(d).update(metallicFactor=-.1));bad('factor-excess',lambda d:pbr(d).update(roughnessFactor=1.1));bad('factor-null',lambda d:pbr(d).update(metallicFactor=None));bad('base-length',lambda d:pbr(d).update(baseColorFactor=[1,1,1]));bad('base-range',lambda d:pbr(d).update(baseColorFactor=[1,1,1,2]));bad('emission-range',lambda d:mat(d).update(emissiveFactor=[2,0,0]));bad('negative-cutoff',lambda d:mat(d).update(alphaCutoff=-1));bad('mode-type',lambda d:mat(d).update(alphaMode=1));bad('mode-extension',lambda d:mat(d).update(alphaMode='FUTURE'));bad('side-type',lambda d:mat(d).update(doubleSided=1));bad('occlusion-range',lambda d:mat(d)['occlusionTexture'].update(strength=2));bad('normal-scale-null',lambda d:mat(d)['normalTexture'].update(scale=None));bad('texture-range',lambda d:pbr(d)['baseColorTexture'].update(index=99));bad('uv-set-limit',lambda d:pbr(d)['baseColorTexture'].update(texCoord=64));bad('uv-fraction',lambda d:pbr(d)['baseColorTexture'].update(texCoord=.5));bad('texture-missing-source',lambda d:d['textures'][0].pop('source'));bad('image-range',lambda d:d['textures'][0].update(source=99));bad('sampler-range',lambda d:d['textures'][0].update(sampler=99));bad('mag-enum',lambda d:d['samplers'][0].update(magFilter=9987));bad('min-enum',lambda d:d['samplers'][0].update(minFilter=123));bad('wrap-enum',lambda d:d['samplers'][0].update(wrapT=1));bad('image-both-sources',lambda d:d['images'][0].update(uri='image.png'));bad('image-no-source',lambda d:d['images'][0].pop('bufferView'));bad('image-missing-mime',lambda d:d['images'][0].pop('mimeType'));bad('image-unsupported-mime',lambda d:d['images'][0].update(mimeType='image/ktx2'));bad('image-view-range',lambda d:d['images'][0].update(bufferView=999));bad('image-view-stride',lambda d:d['bufferViews'][0].update(byteStride=4));bad('image-view-target',lambda d:d['bufferViews'][0].update(target=34962))
# Store only metadata: negative cases reuse the same authored immutable BIN.
(ROOT/'material-invalid.tsv').write_text(''.join(f'{name}\t{data.hex()}\n' for name,data in invalid))
# Independently pin original producer image byte range, no duplicated PNG asset.
b=(ROOT/'BoxTextured.glb').read_bytes();n=struct.unpack_from('<I',b,12)[0];original=json.loads(b[20:20+n]);v=original['bufferViews'][original['images'][0]['bufferView']];image_start=20+n+8+v.get('byteOffset',0);image=b[image_start:image_start+v['byteLength']]
outputs=['material-controls.glb','material-samples.tsv','material-surfaces.tsv','material-parameters.tsv','material-invalid.tsv']
(ROOT/'material-manifest.json').write_text(json.dumps(dict(materials=len(materials),surfaces=len(rows),invalid_contracts=len(invalid),original=dict(source_commit='8f492390ddc7a2ae221a1369ef560589cf8c70fb',bytes=len(b),sha256=hashlib.sha256(b).hexdigest(),image_start=image_start,image_bytes=len(image),image_sha256=hashlib.sha256(image).hexdigest(),sampler=original['samplers'][0]),sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in outputs}),indent=2)+'\n')
print(f'{len(materials)} materials,{len(rows)} Decimal surfaces,{len(invalid)} invalid contracts; original PNG range {image_start}..{image_start+len(image)}')
