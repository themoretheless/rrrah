"""CC0 KTX2 binary16, Python IEEE sample oracles; authored DFD, no libktx claim."""
from pathlib import Path
import importlib.util,struct,hashlib
ROOT=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
spec=importlib.util.spec_from_file_location('builder',Path(__file__).with_name('generate-ktx2-fixtures.py'))
builder=importlib.util.module_from_spec(spec);spec.loader.exec_module(builder)
lines=['# source\tmip\twidth\theight\tkind\tcolor\toracle\tsha256']
controls=[0x8000,0,1,0x3ff,0x400,0x3555,0x3800,0x3c00,0x4000,0x4400,0x7bff,0xb400,0xfbff]
for vk,channels in [(76,1),(83,2),(90,3),(97,4)]:
 for compression in [0,3]:
  for orientation in ['rd','ru','ld','lu']:
   payloads=[];oracles=[];dims=[(9,5),(4,2),(2,1),(1,1)]
   def sample(x,y,c,level):
    i=x*3+y*5+c*7+level*11
    return [0,0x3400,0x3800,0x3c00][i%4] if c==3 else controls[i%len(controls)]
   for level,(w,h) in enumerate(dims):
    payloads.append(b''.join(struct.pack('<H',sample(x,y,c,level)) for y in range(h) for x in range(w) for c in range(channels)))
    oracle=bytearray()
    for y in range(h):
     for x in range(w):
      sx,sy=(w-1-x if orientation[0]=='l' else x),(h-1-y if orientation[1]=='u' else y)
      rgba=[struct.unpack('<e',struct.pack('<H',sample(sx,sy,c,level)))[0] for c in range(channels)]
      if channels<4:rgba+=[0.]*(3-channels)+[1.]
      oracle+=struct.pack('<4f',*rgba)
    oracles.append(oracle)
   data=builder.make(vk,channels,2,9,5,payloads[0],orientation,compression,payloads[1:],floating=True)
   name=f'ktx2-half-{vk}-{compression}-{orientation}.ktx2';(ROOT/name).write_bytes(data);digest=hashlib.sha256(data).hexdigest()
   for level,((w,h),oracle) in enumerate(zip(dims,oracles)):
    target=f'{name}.mip{level}.rgba32f';(ROOT/target).write_bytes(oracle)
    lines.append(f'{name}\t{level}\t{w}\t{h}\trgba32f\tlinear\t{target}\t{digest}')
(ROOT/'ktx2-half-manifest.tsv').write_text('\n'.join(lines)+'\n')
print('32 KTX2 plain/DEFLATE sources;128 selected binary16 cases')
