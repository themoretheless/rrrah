"""CC0 PVR EAC wrappers; independent Ericsson11-bit oracles; all PowerVR channel-type sign mappings."""
from pathlib import Path
import struct,hashlib
ROOT=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
blocks=(ROOT/'eac-r11-block-oracles.bin').read_bytes()
refs={False:(ROOT/'eac-r11-block-oracles.u11le').read_bytes(),True:(ROOT/'eac-r11-block-oracles.i11le').read_bytes()}
lines=['# source\tmip\twidth\theight\tkind\tcolor\toracle\tover\tsha256']
for fmt in [25,26]:
 for typ in range(14):
  signed=typ in [1,3,5,7,9,11,12];dual=fmt==26;denom=1023 if signed else 2047
  for fx in [0,1]:
   for fy in [0,1]:
    name=f'pvr-eac-{fmt}-t{typ}-{fx}{fy}.pvr'
    custom=b'CC0 custom metadata'
    metadata=b'PVR\x03'+struct.pack('<II',3,3)+bytes([fx,fy,0])+b'CC0!'+struct.pack('<II',42,len(custom))+custom
    source=b'PVR\x03'+struct.pack('<IQ9I',0,fmt,0,typ,5,9,1,1,1,4,len(metadata))+metadata
    records=[];start=0
    for level in range(4):
     w,h=max(1,9>>level),max(1,5>>level);cols,rows=(w+3)//4,(h+3)//4
     indices=[]
     for block in range(cols*rows):
      red=(fmt*647+typ*33+(start+block)*19)%4096;green=(red+137)%4096
      source+=blocks[red*8:red*8+8]
      if dual:source+=blocks[green*8:green*8+8]
      indices.append((red,green))
     output=bytearray()
     for y in range(h):
      for x in range(w):
       sx,sy=(w-1-x if fx else x),(h-1-y if fy else y)
       red,green=indices[(sy//4)*cols+sx//4];pixel=(sy%4)*4+sx%4
       r=struct.unpack_from('<h',refs[signed],red*32+pixel*2)[0]
       g=struct.unpack_from('<h',refs[signed],green*32+pixel*2)[0] if dual else 0
       output.extend(struct.pack('<4f',r/denom,g/denom,0,1))
     oracle=f'{name}.mip{level}.rgba32f';(ROOT/oracle).write_bytes(output)
     records.append(f'{name}\t{level}\t{w}\t{h}\trgba32f\t0\t{oracle}\t-')
     start+=cols*rows
    (ROOT/name).write_bytes(source);digest=hashlib.sha256(source).hexdigest()
    lines.extend(record+'\t'+digest for record in records)
(ROOT/'pvr-eac-manifest.tsv').write_text('\n'.join(lines)+'\n')
print('112 PVR EAC sources;448 selected mip/type/orientation cases')
