"""CC0 binary16 KTX1 mip/endian/orientation fixtures; Python IEEE expansion oracle."""
from pathlib import Path
import struct,hashlib
ROOT=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=['# source\tmip\twidth\theight\tkind\tcolor\toracle\tover\tsha256']
controls=[0x8000,0,1,0x3ff,0x400,0x3555,0x3800,0x3c00,0x4000,0x4400,0x7bff,0xb400,0xfbff]
for channels,internal,base in [(1,0x822d,0x1903),(2,0x822f,0x8227),(3,0x881b,0x1907),(4,0x881a,0x1908)]:
 def bits(x,y,c,level):
  i=x*3+y*5+c*7+level*11
  return [0,0x3400,0x3800,0x3c00][i%4] if c==3 else controls[i%len(controls)]
 for endian in ['le','be']:
  order='<' if endian=='le' else '>';word=lambda x:struct.pack(order+'I',x)
  for fx in [0,1]:
   for fy in [0,1]:
    name=f'ktx-half-{internal:x}-{endian}-{fx}{fy}.ktx'
    kv=b'KTXorientation\0'+f'S={"l" if fx else "r"},T={"u" if fy else "d"}'.encode()+b'\0'
    meta=word(len(kv))+kv+b'\0'*((-len(kv))%4)
    source=b'\xabKTX 11\xbb\r\n\x1a\n'+word(0x04030201)+b''.join(word(v) for v in [0x140b,2,base,internal,base,9,5,0,0,1,4,len(meta)])+meta
    records=[]
    for level in range(4):
     w,h=max(1,9>>level),max(1,5>>level);data=bytearray();oracle=bytearray()
     for y in range(h):
      row=b''.join(struct.pack(order+'H',bits(x,y,c,level)) for x in range(w) for c in range(channels))
      data+=row+b'\xAD'*((-len(row))%4)
     for y in range(h):
      for x in range(w):
       sx,sy=(w-1-x if fx else x),(h-1-y if fy else y)
       rgba=[struct.unpack('<e',struct.pack('<H',bits(sx,sy,c,level)))[0] for c in range(channels)]
       if channels<4:rgba += [0.]*(3-channels)+[1.]
       oracle+=struct.pack('<4f',*rgba)
     source+=word(len(data))+data
     oracle_name=f'{name}.mip{level}.rgba32f';(ROOT/oracle_name).write_bytes(oracle)
     color='linear-unspecified' if channels<3 else 'linear'
     records.append(f'{name}\t{level}\t{w}\t{h}\trgba32f\t{color}\t{oracle_name}\t-')
    (ROOT/name).write_bytes(source);digest=hashlib.sha256(source).hexdigest()
    lines.extend(record+'\t'+digest for record in records)
(ROOT/'ktx-half-manifest.tsv').write_text('\n'.join(lines)+'\n')
print('32 sources;128 selected KTX binary16 cases')
