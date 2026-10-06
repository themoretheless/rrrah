#!/usr/bin/env python3
"""CC0 PVR v3 fixtures; PowerVR SDK PVRTC raw-block oracle and authored NumPy samples."""
from pathlib import Path
import hashlib
import random
import struct
import subprocess
import tempfile
import urllib.request
import numpy as np

root=Path(__file__).resolve().parents[1]
out=root/'tests/fixtures/raster'
cache=Path(tempfile.gettempdir())/'rrrah-pvr-reference'
cache.mkdir(exist_ok=True)
revision='63f0098acdc11ec9a072c30000d2fa670a955bee'
files={
 'PVRTDecompress.cpp':('framework/PVRCore/texture/PVRTDecompress.cpp','74559c5a4b8161aafce1ebe984bf8060896feaa81276b614491dbee755d6e4c7'),
 'PVRTDecompress.h':('framework/PVRCore/texture/PVRTDecompress.h','67123a36b99df380af76a65f3dfa3a7f368a153f5401b36a9ff1ed233cee6f5f'),
 'LICENSE.md':('LICENSE.md','b1aea79afab593649ede742eccbb7feb74d216d52d120b99c4d349133871ba9f'),
}
for name,(path,digest) in files.items():
 target=cache/name
 if not target.exists():target.write_bytes(urllib.request.urlopen(f'https://raw.githubusercontent.com/powervr-graphics/Native_SDK/{revision}/{path}').read())
 assert hashlib.sha256(target.read_bytes()).hexdigest()==digest
binary=cache/'pvrtc-oracle'
wrapper=root/'scripts/pvr-fixture-oracle.cpp'
compile_hash=hashlib.sha256((cache/'PVRTDecompress.cpp').read_bytes()+(cache/'PVRTDecompress.h').read_bytes()+wrapper.read_bytes()).hexdigest()
stamp=cache/'build.sha256'
if not binary.exists() or not stamp.exists() or stamp.read_text()!=compile_hash:
 subprocess.run(['clang++','-std=c++17','-O2','-I',str(cache),str(cache/'PVRTDecompress.cpp'),str(wrapper),'-o',str(binary)],check=True)
 stamp.write_text(compile_hash)
rows=[]
def meta(endian,flip=(0,0),override=None):
 def item(fourcc,key,data):return struct.pack(endian+'III',int.from_bytes(fourcc,'little'),key,len(data))+data
 result=item(b'PVR\x03',3,bytes([*flip,0]))+item(b'CC0!',42,b'CC0 custom metadata')+item(b'PVR\x03',5,b'padding')
 if override is not None:result+=item(b'PVR\x03',6,bytes([override]*4))
 return result

def save(name,fmt,typ,color,w,h,levels,payloads,oracles,kind,flip=(0,0),endian='<',flags=0,override=None):
 metadata=meta(endian,flip,override)
 header=struct.pack(endian+'IIQIIIIIIIII',0x03525650,flags,fmt,color,typ,h,w,1,1,1,levels,len(metadata))
 path=out/name
 path.write_bytes(header+metadata+b''.join(payloads))
 for index,pixels in enumerate(oracles):
  if flip[0]:pixels=pixels[:,::-1]
  if flip[1]:pixels=pixels[::-1,:]
  ph,pw=pixels.shape[:2]
  oracle=name+f'-{index}.{kind}'
  over='-'
  if flags:
   scale=np.float32(255 if kind=='rgba8' else 65535) if kind!='rgba32f' else np.float32(1)
   stored=pixels.astype('f4')/scale
   associated=stored.copy()
   pixels=stored.copy()
   positive=pixels[:,:,3]>0
   pixels[:,:,:3][positive]/=pixels[:,:,3:4][positive]
   composed=associated[:,:,:3]+np.array([.25,.5,.75],dtype='f4')*(1-associated[:,:,3:4])
   over=name+f'-{index}.over-rgb32f'
   (out/over).write_bytes(composed.astype('<f4').tobytes())
   oracle=name+f'-{index}.rgba32f'
  dtype='<u2' if oracle.endswith('rgba16') else '<f4' if oracle.endswith('rgba32f') else 'u1'
  (out/oracle).write_bytes(pixels.astype(dtype).tobytes())
  rows.append(f'{name}\t{index}\t{pw}\t{ph}\t{oracle.rsplit(".",1)[1]}\t{color}\t{oracle}\t{over}\t{hashlib.sha256(path.read_bytes()).hexdigest()}')

cases=[('rgba8',b'rgba',1,0,1,'<',(0,0),0,None),('rgb8-flip',b'rgb',1,0,0,'<',(1,1),0,None),
 ('bgra8',b'bgra',1,0,1,'>',(1,0),0,None),('la8',b'la',1,0,0,'<',(0,1),0,None),
 ('rgba16-le',b'rgba',2,4,0,'<',(0,0),0,None),('rgba16-be',b'rgba',2,4,1,'>',(1,1),0,None),
 ('rg16-override',b'rg',2,0,0,'<',(0,0),0,4),
 ('rgba32f-le',b'rgba',4,12,0,'<',(0,0),0,None),('rgba32f-be',b'rgba',4,12,0,'>',(0,1),0,None),
 ('rgba8-premul',b'rgba',1,0,0,'<',(0,0),2,None)]
for label,names,size,typ,color,endian,flip,flags,override in cases:
 fmt=int.from_bytes(names.ljust(4,b'\0')+bytes([size*8]*len(names)).ljust(4,b'\0'),'little')
 payloads=[];oracles=[]
 for level in range(3):
  w=max(1,5>>level);h=max(1,3>>level)
  rgba=np.ones((h,w,4),dtype='u1' if size==1 else 'u2' if size==2 else 'f4')
  if size<4:
   maximum=255 if size==1 else 65535
   rgb_values=[0,1,2,127,128,254,255] if size==1 else [0,1,2,257,32767,32768,65535]
   for y in range(h):
    for x in range(w):
     i=y*w+x+level
     rgba[y,x,:]=[rgb_values[(i+j)%len(rgb_values)] for j in range(4)]
   if flags:
    rgba[:,:,:3]=np.minimum(rgba[:,:,:3],rgba[:,:,3:4])
  else:
   for y in range(h):
    for x in range(w):
     i=y*w+x+level
     rgba[y,x]=[[-.5,0.,.5,1.0000001192092896,4.][(i+j)%5] for j in range(3)]+[[0.,.25,.5,1.][i%4]]
  if names==b'rgb':rgba[:,:,3]=255
  if names==b'la':rgba[:,:,1:3]=rgba[:,:,:1]
  if names==b'rg':rgba[:,:,2]=0;rgba[:,:,3]=65535
  channels={'r':0,'g':1,'b':2,'a':3,'l':0}
  native=np.stack([rgba[:,:,channels[chr(n)]] for n in names],axis=2)
  dtype='u1' if size==1 else endian+'u2' if size==2 else endian+'f4'
  payloads.append(native.astype(dtype).tobytes());oracles.append(rgba)
 save(f'pvr-{label}.pvr',fmt,typ,color,5,3,3,payloads,oracles,'rgba8' if size==1 else 'rgba16' if size==2 else 'rgba32f',flip,endian,flags,override)

rng=random.Random(8741)
for fmt in range(4):
 for flip in [(0,0),(1,1)]:
  w,h,levels=(32,16,4) if fmt<2 else (16,8,3)
  payloads=[];oracles=[]
  for level in range(levels):
   pw=max(1,w>>level);ph=max(1,h>>level)
   length=max(pw,16 if fmt<2 else 8)*max(ph,8)//(4 if fmt<2 else 2)
   payload=b''.join(struct.pack('<I',rng.getrandbits(32)) for _ in range(length//4))
   source=cache/'authored-pvrtc.payload';source.write_bytes(payload)
   rgba=subprocess.run([str(binary),str(source),str(pw),str(ph),str(fmt)],check=True,stdout=subprocess.PIPE).stdout
   assert len(rgba)==pw*ph*4
   payloads.append(payload);oracles.append(np.frombuffer(rgba,dtype='u1').reshape(ph,pw,4))
  save(f'pvr-pvrtc1-{fmt}-flip{flip[0]}.pvr',fmt,0,1,w,h,levels,payloads,oracles,'rgba8',flip)
(out/'pvr-manifest.tsv').write_text('# CC0 authored container; PowerVR SDK raw PVRTC oracle; file\tindex\tw\th\tkind\tcolor\toracle\tover\tsha256\n'+'\n'.join(rows)+'\n')
