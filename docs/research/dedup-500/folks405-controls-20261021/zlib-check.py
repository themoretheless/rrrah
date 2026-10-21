from pathlib import Path
import ctypes as ct,json,subprocess,zlib
C=Path('.dedup-work/folks405-20261021');stub=C/'stbi-shared.c';stub.write_text('#define STB_IMAGE_IMPLEMENTATION\n#include "stb_image.h"\n');libpath=C/'stbi-shared.dylib';subprocess.run(['cc','-shared','-fPIC','-I',str(C),str(stub),'-lm','-o',str(libpath)],check=True);lib=ct.CDLL(str(libpath.resolve()));lib.stbi_zlib_decode_buffer.argtypes=[ct.c_void_p,ct.c_int,ct.c_void_p,ct.c_int];lib.stbi_zlib_decode_buffer.restype=ct.c_int
cases=[]
for payload in [b'hello image',bytes(range(256)),b'ABC'*1024,b'\0'*4096]:
 for level in [0,1,6,9]:
  encoded=zlib.compress(payload,level);out=ct.create_string_buffer(len(payload)+1);n=lib.stbi_zlib_decode_buffer(out,len(payload),encoded,len(encoded));assert n==len(payload) and out.raw[:n]==payload;cases.append({'length':len(payload),'level':level})
payload=b'valid pixels';encoded=zlib.compress(payload);bad=encoded[:-1]+bytes([encoded[-1]^1]);out=ct.create_string_buffer(64);n=lib.stbi_zlib_decode_buffer(out,64,bad,len(bad));assert n==len(payload) and out.raw[:n]==payload
try:zlib.decompress(bad);raise AssertionError('expected checksum rejection')
except zlib.error:pass
r={'original_stb_header_unchanged':True,'valid_cases':cases,'valid_cases_passed':len(cases),'bad_adler32':{'stb_accepted_payload':True,'python_zlib_rejected':True},'limits':['Small synthetic zlib streams only; checksum contract observation, not PNG file conformance or exploit.','No ASAN or deferred native pHash execution.']};Path('docs/research/dedup-500/folks405-controls-20261021/zlib-result.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
