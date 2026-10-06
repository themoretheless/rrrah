"""CC0 PKM fixtures; independent Android ETC1 C++ decoder oracle.
Downloads Apache-2.0 AOSP ETC1 source into a temporary directory only.
"""
from pathlib import Path
import tempfile,urllib.request,base64,subprocess,ctypes,struct,hashlib
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith('.pkm')]
def source(path):
 url='https://android.googlesource.com/platform/frameworks/native/+/refs/heads/main/'+path+'?format=TEXT'
 return base64.b64decode(urllib.request.urlopen(url).read())
with tempfile.TemporaryDirectory() as directory:
 d=Path(directory);(d/'ETC1').mkdir()
 cpp=source('opengl/libs/ETC1/etc1.cpp');header=source('opengl/include/ETC1/etc1.h')
 (d/'etc1.cpp').write_bytes(cpp);(d/'ETC1/etc1.h').write_bytes(header)
 assert hashlib.sha256(cpp).hexdigest()=='5e6e1f9d5515f9a4ee58ef3e40ec71ed23995ae1410c72db3ccab6e25ae1931b', 'Android oracle source changed; review before regeneration'
 subprocess.run(['clang++','-std=c++11','-O2','-shared','-fPIC','-I',str(d),str(d/'etc1.cpp'),'-o',str(d/'etc1.so')],check=True)
 lib=ctypes.CDLL(str(d/'etc1.so'));lib.etc1_decode_image.argtypes=[ctypes.c_char_p,ctypes.c_void_p,ctypes.c_uint,ctypes.c_uint,ctypes.c_uint,ctypes.c_uint]
 individual=bytes.fromhex('1234560001234567');differential=bytes.fromhex('80808003abcdef01')
 for name,version,kind,w,h,payload in [
  ('individual',10,0,4,4,individual),('differential-flip',10,0,4,4,differential),
  ('cropped',10,0,7,5,individual+differential+differential+individual),
  ('etc2-compatible',20,1,4,4,individual),
  ('etc2-alpha-constant',20,3,4,4,bytes([17])+bytes(7)+individual),
  ('etc2-rgba1-opaque',20,4,4,4,differential)]:
  name+='.pkm';ew=(w+3)//4*4;eh=(h+3)//4*4;data=f'PKM {version}'.encode()+struct.pack('>5H',kind,ew,eh,w,h)+payload
  (root/name).write_bytes(data);out=(ctypes.c_ubyte*(w*h*3))()
  assert lib.etc1_decode_image(payload[8:] if kind==3 else payload,out,w,h,3,w*3)==0
  rgb=bytes(out);rgba=b''.join(rgb[i:i+3]+bytes([17 if kind==3 else 255]) for i in range(0,len(rgb),3));(root/(name+'.rgba')).write_bytes(rgba)
  lines.append(f'{name}\t{w}\t{h}\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
