"""CC0 KTX1 EAC wrappers over independent Ericsson-derived PVR fixture blocks."""
from pathlib import Path
import struct, hashlib
ROOT = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
lines = ['# source\tmip\twidth\theight\tkind\tcolor\toracle\tover\tsha256']
for internal, fmt, typ, base in [(0x9270,25,0,0x1903),(0x9271,25,1,0x1903),
                                (0x9272,26,0,0x8227),(0x9273,26,1,0x8227)]:
 for endian in ['le','be']:
  order = '<' if endian == 'le' else '>'
  word = lambda x: struct.pack(order+'I', x)
  for fx in [0,1]:
   for fy in [0,1]:
    original_name = f'pvr-eac-{fmt}-t{typ}-{fx}{fy}.pvr'
    original = (ROOT / original_name).read_bytes()
    at = 52 + struct.unpack_from('<I', original,48)[0]
    kv = b'KTXorientation\0'+f'S={"l" if fx else "r"},T={"u" if fy else "d"}'.encode()+b'\0'
    metadata = word(len(kv))+kv+b'\0'*((-len(kv))%4)
    source = b'\xabKTX 11\xbb\r\n\x1a\n'+word(0x04030201)
    source += b''.join(word(x) for x in [0,1,0,internal,base,9,5,0,0,1,4,len(metadata)])+metadata
    name = f'ktx-eac-{internal:x}-{endian}-{fx}{fy}.ktx'
    records = []
    for level in range(4):
     w,h=max(1,9>>level),max(1,5>>level)
     size=((w+3)//4)*((h+3)//4)*(8 if fmt==25 else 16)
     source += word(size)+original[at:at+size];at+=size
     records.append(f'{name}\t{level}\t{w}\t{h}\trgba32f\tlinear-unspecified\t{original_name}.mip{level}.rgba32f\t-')
    assert at == len(original)
    (ROOT/name).write_bytes(source)
    digest=hashlib.sha256(source).hexdigest()
    lines.extend(record+'\t'+digest for record in records)
(ROOT/'ktx-eac-manifest.tsv').write_text('\n'.join(lines)+'\n')
print('32 KTX1 sources;128 mip/codec/endian/orientation cases')
