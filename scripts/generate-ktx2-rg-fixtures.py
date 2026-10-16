"""CC0 KTX2 native R/RG sample contracts with authored DFD and Python pixel oracle."""
from pathlib import Path
import importlib.util,struct,hashlib
ROOT=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
spec=importlib.util.spec_from_file_location('builder',Path(__file__).with_name('generate-ktx2-fixtures.py'))
builder=importlib.util.module_from_spec(spec);spec.loader.exec_module(builder)
lines=['# source\tmip\twidth\theight\tkind\tcolor\toracle\tsha256']
for vk,channels,size in [(9,1,1),(16,2,1),(70,1,2),(77,2,2),(100,1,4),(103,2,4)]:
 code={1:'B',2:'H',4:'f'}[size];kind={1:'rgba8',2:'rgba16',4:'rgba32f'}[size]
 def sample(x,y,c,level):
  i=x*3+y*5+c*7+level*11
  return ([0,1,127,128,254,255][i%6] if size==1 else [0,1,32767,32768,32769,65534,65535][i%7] if size==2 else [-0.0,-0.25,0.0,0.125,0.5,1.0,2.0,4.0][i%8])
 for compression in [0,3]:
  for orientation in ['rd','ru','ld','lu']:
   payloads=[];oracles=[];dims=[(9,5),(4,2),(2,1),(1,1)]
   for level,(w,h) in enumerate(dims):
    values=[sample(x,y,c,level) for y in range(h) for x in range(w) for c in range(channels)]
    payloads.append(struct.pack('<'+code*len(values),*values));oracle=bytearray()
    for y in range(h):
     for x in range(w):
      sx,sy=(w-1-x if orientation[0]=='l' else x),(h-1-y if orientation[1]=='u' else y)
      rgba=[sample(sx,sy,c,level) for c in range(channels)]+[0]*(3-channels)+[1.0 if size==4 else (1<<(size*8))-1]
      oracle+=struct.pack('<'+code*4,*rgba)
    oracles.append(oracle)
   data=builder.make(vk,channels,size,9,5,payloads[0],orientation,compression,payloads[1:])
   name=f'ktx2-rg-{vk}-{compression}-{orientation}.ktx2';(ROOT/name).write_bytes(data);digest=hashlib.sha256(data).hexdigest()
   for level,((w,h),oracle) in enumerate(zip(dims,oracles)):
    target=f'{name}.mip{level}.{kind}';(ROOT/target).write_bytes(oracle)
    lines.append(f'{name}\t{level}\t{w}\t{h}\t{kind}\tlinear\t{target}\t{digest}')
(ROOT/'ktx2-rg-manifest.tsv').write_text('\n'.join(lines)+'\n')
print('48 KTX2 R/RG sources;192 selected mip/precision/compression/orientation cases')
