"""CC0 analytical KTX1 R/RG mip/endian/orientation fixtures; no decoder dependency."""
from pathlib import Path
import struct, hashlib
ROOT=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=['# source\tmip\twidth\theight\tkind\tcolor\toracle\tover\tsha256']
for channels,size,internal in [(1,1,0x8229),(2,1,0x822b),(1,2,0x822a),(2,2,0x822c),(1,4,0x822e),(2,4,0x8230)]:
 code={1:'B',2:'H',4:'f'}[size];kind={1:'rgba8',2:'rgba16',4:'rgba32f'}[size]
 def sample(x,y,c,level):
  i=x*3+y*5+c*7+level*11
  return ([0,1,127,128,254,255][i%6] if size==1 else
          [0,1,32767,32768,32769,65534,65535][i%7] if size==2 else
          [-0.0,-0.25,0.0,0.125,0.5,1.0,2.0,4.0][i%8])
 for endian in ['le','be']:
  order='<' if endian=='le' else '>';word=lambda x:struct.pack(order+'I',x)
  for fx in [0,1]:
   for fy in [0,1]:
    name=f'ktx-rg-{internal:x}-{endian}-{fx}{fy}.ktx'
    kv=b'KTXorientation\0'+f'S={"l" if fx else "r"},T={"u" if fy else "d"}'.encode()+b'\0'
    meta=word(len(kv))+kv+b'\0'*((-len(kv))%4)
    base=0x1903 if channels==1 else 0x8227
    ty={1:0x1401,2:0x1403,4:0x1406}[size]
    source=b'\xabKTX 11\xbb\r\n\x1a\n'+word(0x04030201)+b''.join(word(v) for v in [ty,size,base,internal,base,9,5,0,0,1,4,len(meta)])+meta
    records=[]
    for level in range(4):
     w,h=max(1,9>>level),max(1,5>>level);data=bytearray();oracle=bytearray()
     for y in range(h):
      row=b''.join(struct.pack(order+code,sample(x,y,c,level)) for x in range(w) for c in range(channels))
      data+=row+b'\xAD'*((-len(row))%4)
     for y in range(h):
      for x in range(w):
       sx,sy=(w-1-x if fx else x),(h-1-y if fy else y)
       rgba=[sample(sx,sy,c,level) for c in range(channels)]+[0]*(3-channels)+[1.0 if size==4 else (1<<(size*8))-1]
       oracle+=struct.pack('<'+code*4,*rgba)
     source+=word(len(data))+data
     oracle_name=f'{name}.mip{level}.{kind}';(ROOT/oracle_name).write_bytes(oracle)
     records.append(f'{name}\t{level}\t{w}\t{h}\t{kind}\tlinear-unspecified\t{oracle_name}\t-')
    (ROOT/name).write_bytes(source);digest=hashlib.sha256(source).hexdigest()
    lines.extend(record+'\t'+digest for record in records)
(ROOT/'ktx-rg-manifest.tsv').write_text('\n'.join(lines)+'\n')
print('48 sources;192 selected native-precision KTX R/RG cases')
