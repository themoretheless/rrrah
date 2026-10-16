"""CC0 PKM2 EAC wrappers; independent Ericsson11-bit sample oracles normalized to float32."""
from pathlib import Path
import struct,hashlib
ROOT=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
blocks=(ROOT/'eac-r11-block-oracles.bin').read_bytes()
refs={False:(ROOT/'eac-r11-block-oracles.u11le').read_bytes(),True:(ROOT/'eac-r11-block-oracles.i11le').read_bytes()}
lines=['# source\twidth\theight\tkind\toracle\tsha256']
for kind in [5,6,7,8]:
 signed=kind>=7;dual=kind in [6,8];denom=1023 if signed else 2047
 for w,h in [(4,4),(7,5),(1,1),(9,2)]:
  name=f'pkm-eac-{kind}-{w}x{h}.pkm';cols,rows=(w+3)//4,(h+3)//4
  source=b'PKM 20'+struct.pack('>5H',kind,cols*4,rows*4,w,h);indices=[]
  for block in range(cols*rows):
   red=(kind*647+block*19)%4096;green=(red+137)%4096
   source+=blocks[red*8:red*8+8]
   if dual:source+=blocks[green*8:green*8+8]
   indices.append((red,green))
  output=bytearray()
  for y in range(h):
   for x in range(w):
    red,green=indices[(y//4)*cols+x//4];pixel=(y%4)*4+x%4
    r=struct.unpack_from('<h',refs[signed],red*32+pixel*2)[0]
    g=struct.unpack_from('<h',refs[signed],green*32+pixel*2)[0] if dual else 0
    output.extend(struct.pack('<4f',r/denom,g/denom,0,1))
  oracle=name+'.rgba32f';(ROOT/name).write_bytes(source);(ROOT/oracle).write_bytes(output)
  lines.append(f'{name}\t{w}\t{h}\t{kind}\t{oracle}\t{hashlib.sha256(source).hexdigest()}')
(ROOT/'pkm-eac-manifest.tsv').write_text('\n'.join(lines)+'\n')
print('16 PKM2 EAC signed/unsigned R/RG sources and float32 oracles')
