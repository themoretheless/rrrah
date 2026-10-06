#!/usr/bin/env python3
"""Independent LittleCMS full-domain corpus for the pinned CMYK profile."""
import argparse, ctypes as c, hashlib, itertools, json, random, struct
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--profile',type=Path,required=True);p.add_argument('--library',type=Path,required=True)
p.add_argument('--prefix',type=Path,required=True)
p.add_argument('--input-cmyk8',type=Path,help='Transform independently decoded CMYK8 sample bytes instead of the synthetic grid')
p.add_argument('--quantized-input',action='store_true',help='Generate f32 inputs at exact normalized CMYK8 code values')
p.add_argument('--verify-fixtures',action='store_true',help='Require exact reproduction of repository input and RGB8 oracle')
a=p.parse_args()
b=a.profile.read_bytes();assert hashlib.sha256(b).hexdigest()=='73e1ba37d2bad5bab2a964f40a9eed96209666efc067c3322626214bbef234a0'
f32=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]
r=random.Random(20261007)
colors=list(itertools.product((0,.25,.5,.75,1),repeat=4))+[tuple(f32(r.random()) for _ in range(4)) for _ in range(4096)]
if a.input_cmyk8:
 raw=a.input_cmyk8.read_bytes()
 if len(raw)%4: raise ValueError('CMYK input must have four-byte samples')
 colors=[tuple(f32(v/255) for v in raw[i:i+4]) for i in range(0,len(raw),4)]
if a.quantized_input:
 colors=[tuple(f32(int(v*255+.5)/255) for v in row) for row in colors]
input_data=b''.join(struct.pack('<4f',*v) for v in colors)
lcms=c.CDLL(str(a.library.resolve()))
def bind(n,result,*args):
 f=getattr(lcms,n);f.restype=result;f.argtypes=list(args);return f
op=bind('cmsOpenProfileFromMem',c.c_void_p,c.c_void_p,c.c_uint32)
srgb=bind('cmsCreate_sRGBProfile',c.c_void_p)
create=bind('cmsCreateTransform',c.c_void_p,c.c_void_p,c.c_uint32,c.c_void_p,c.c_uint32,c.c_uint32,c.c_uint32)
do=bind('cmsDoTransform',None,c.c_void_p,c.c_void_p,c.c_void_p,c.c_uint32)
delete=bind('cmsDeleteTransform',None,c.c_void_p);close=bind('cmsCloseProfile',c.c_int,c.c_void_p)
buf=c.create_string_buffer(b);source=op(buf,len(b));dest=srgb();assert source and dest
executor=None
try:
 executor=create(source,(1<<22)|(6<<16)|(4<<3)|4,dest,(1<<22)|(4<<16)|(3<<3)|4,0,0x100);assert executor
 values=(c.c_float*(len(colors)*4))(*[f32(v*100) for row in colors for v in row]);out=(c.c_float*(len(colors)*3))();do(executor,values,out,len(colors))
 reference=b''.join(struct.pack('<f',v) for v in out)
finally:
 if executor:delete(executor)
 close(dest);close(source)
Path(str(a.prefix)+'.cmykf32le').write_bytes(input_data);Path(str(a.prefix)+'.rgbf32le').write_bytes(reference)
Path(str(a.prefix)+'.json').write_text(json.dumps({'count':len(colors),'grid':0 if a.input_cmyk8 else 625,'random':0 if a.input_cmyk8 else 4096,'seed':20261007,'profile_sha256':hashlib.sha256(b).hexdigest(),'library_sha256':hashlib.sha256(a.library.read_bytes()).hexdigest(),'input_sha256':hashlib.sha256(input_data).hexdigest(),'reference_sha256':hashlib.sha256(reference).hexdigest(),'intent':0,'flags':256,'scope':'LittleCMS float percent CMYK to float RGB, normalized native inputs'},indent=2)+'\n')
if a.verify_fixtures:
 fixture_root=Path(__file__).resolve().parents[1]/'tests/fixtures/pdf'
 expected_input=(fixture_root/'cmyk-full-domain-input.f32le').read_bytes()
 expected_rgb=(fixture_root/'cmyk-full-domain-littlecms.rgb8').read_bytes()
 quantized=bytes(max(0,min(255,int(v*255+.5))) for v in out)
 if input_data != expected_input or quantized != expected_rgb:
  raise ValueError('Independent CMYK corpus differs from committed reference')
 metadata=json.loads((fixture_root/'cmyk-full-domain-reference.json').read_text())
 if metadata['input_sha256'] != hashlib.sha256(input_data).hexdigest() or metadata['rgb8_reference_sha256'] != hashlib.sha256(quantized).hexdigest():
  raise ValueError('Committed CMYK corpus provenance hash mismatch')
 print('Exact independent fixture reproduction verified')
print(len(colors))
