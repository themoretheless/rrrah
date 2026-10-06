"""CC0 KTX1 fixtures; authored PNG pixels and existing independent DDS oracles."""
from pathlib import Path
import struct,hashlib
from PIL import Image
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith('.ktx')]
def make(ty,size,fmt,internal,base,w,h,payload,be=False,orientation='S=r,T=d'):
 order='>' if be else '<';kv=b'KTXorientation\0'+orientation.encode()+b'\0';meta=struct.pack(order+'I',len(kv))+kv+bytes(-len(kv)%4)
 return b'\xabKTX 11\xbb\r\n\x1a\n'+struct.pack(order+'13I',0x04030201,ty,size,fmt,internal,base,w,h,0,0,1,1,len(meta))+meta+struct.pack(order+'I',len(payload))+payload+bytes(-len(payload)%4)
def record(name,data,w,h,rgba):
 (root/name).write_bytes(data);(root/(name+'.rgba')).write_bytes(rgba)
 lines.append(f'{name}\t{w}\t{h}\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
im=Image.new('RGBA',(3,2));im.putdata([(255,0,0,255),(0,255,0,128),(0,0,255,0),(17,29,43,64),(61,73,89,127),(250,240,230,255)])
for be in [False,True]:
 for orientation in ['S=r,T=d','S=r,T=u','S=l,T=d','S=l,T=u']:
  source=im
  if 'S=l' in orientation:source=source.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
  if 'T=u' in orientation:source=source.transpose(Image.Transpose.FLIP_TOP_BOTTOM)
  name=f'rgba8-{int(be)}-{orientation[2]}{orientation[6]}.ktx'
  record(name,make(0x1401,1,0x1908,0x8c43,0x1908,3,2,source.tobytes(),be,orientation),3,2,im.tobytes())
rgb=im.convert('RGB').tobytes();payload=b''.join(rgb[i:i+9]+bytes(3) for i in range(0,len(rgb),9))
record('rgb8-padded.ktx',make(0x1401,1,0x1907,0x8c41,0x1907,3,2,payload),3,2,im.convert('RGB').convert('RGBA').tobytes())
for name,internal,base in [('bc1.dds',0x83f0,0x1907),('bc1-alpha.dds',0x83f1,0x1908),('bc2.dds',0x83f2,0x1908),('bc3.dds',0x83f3,0x1908)]:
 d=(root/name).read_bytes();h,w=struct.unpack_from('<II',d,12)
 record(name+'.ktx',make(0,1,0,internal,base,w,h,d[128:]),w,h,(root/(name+'.rgba')).read_bytes())
for depth,ty,size,internal,values in [(16,0x1403,2,0x805b,[32768,1,65534,65535,32769,2,65535,65535]),(32,0x1406,4,0x8814,[0.5,0.1,0.3,1.0,2.0,0.2,0.4,1.0])]:
 for be in [False,True]:
  payload=b''.join(struct.pack(('>' if be else '<')+('H' if depth==16 else 'f'),v) for v in values)
  (root/f'precision-{depth}-{int(be)}.ktx').write_bytes(make(ty,size,0x1908,internal,0x1908,2,1,payload,be))
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
