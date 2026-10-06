#!/usr/bin/env python3
"""CC0 authored STCI fixtures and native sample references; not an external STI oracle."""
import hashlib,json,struct,zlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]/'tests/fixtures/sti';ROOT.mkdir(exist_ok=True)
def header(flags,bits,stored,original):
    h=bytearray(64);h[:4]=b'STCI';struct.pack_into('<4I2H',h,4,original,stored,0,flags,2,3);h[44]=bits;return h
def save(name,data,bits,frames):
    name+='.sti';(ROOT/name).write_bytes(data)
    for i,p in enumerate(frames): (ROOT/f'{name}-image-{i}.rgba').write_bytes(bytes(p) if bits==8 else struct.pack('<'+str(len(p))+'H',*p))
    cases.append({'file':name,'sha256':hashlib.sha256(data).hexdigest(),'sample_bits':bits,'rgba':frames})
cases=[];palette=bytes([9,8,7,255,0,0,0,255,0]);indices=bytes([1,0,2,2,1,0]);pixels=[]
for v in indices:pixels.extend(list(palette[v*3:v*3+3])+[0 if v==0 else 255])
for compressed in [False,True]:
    data=zlib.compress(indices) if compressed else indices
    h=header(25 if compressed else 9,8,len(data),len(indices));struct.pack_into('<I',h,24,3);h[30:33]=bytes([8]*3)
    save('indexed-zlib' if compressed else 'indexed',h+palette+data,8,[pixels])
data=bytes([2,1,2,129,0,129,2,1,2,0,131,0,131,0]);h=header(40,8,len(data),12);struct.pack_into('<IH3B',h,24,3,2,8,8,8)
directory=struct.pack('<IIhhHH',0,10,-1,2,2,3)+struct.pack('<IIhhHH',10,4,4,-3,2,3)
save('etrle-two-images',h+palette+directory+data,8,[[255,0,0,255,0,255,0,255,0,0,0,0,0,0,0,0,255,0,0,255,0,255,0,255],[0]*24])
for depth in [16,24,32]:
    values=[0xf800,0x07e0,0x001f,0x8410,0xffff,0] if depth==16 else [33,81,129,192,64,96,1,2,3,255,254,253,0,128,255,255,0,128]
    if depth==16:
        raw=struct.pack('<6H',*values);masks=[0xf800,0x07e0,0x001f,0];depths=[5,6,5,0];pixels=[]
        # All channel normalization uses nearest integer, matching mathematical scaling.
        pixels=[]
        for value in values:pixels.extend([(((value>>shift)&maxv)*65535+maxv//2)//maxv for shift,maxv in [(11,31),(5,63),(0,31)]]+[0 if value==0x07e0 else 65535])
    else:
        alphas=[255,128,1,0,254,64];raw=bytes(values) if depth==24 else bytes(v for i in range(6) for v in values[i*3:i*3+3]+[alphas[i]])
        masks=[0xff,0xff00,0xff0000,0xff000000 if depth==32 else 0];depths=[8,8,8,8 if depth==32 else 0];pixels=[]
        for i in range(6):pixels.extend([v*257 for v in values[i*3:i*3+3]]+[(alphas[i] if depth==32 else 255)*257])
    for compressed in [False,True]:
        data=zlib.compress(raw) if compressed else raw
        flags=(5 if depth==16 else 6 if depth==32 else 4)|(16 if compressed else 0)
        h=header(flags,depth,len(data),len(raw));struct.pack_into('<I',h,12,0x07e0 if depth==16 else 0);struct.pack_into('<4I',h,24,*masks);h[40:44]=bytes(depths)
        save(f'rgb{depth}'+('-zlib' if compressed else ''),h+data,16,[pixels])
(ROOT/'manifest.json').write_text(json.dumps({'license':'CC0-1.0','reference':'Python authored samples and mask scaling, not external STI decoder proof','cases':cases},indent=2)+'\n')
