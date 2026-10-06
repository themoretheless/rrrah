"""CC0 KTX2 RGB/RGBA fixtures, authored Pillow pixels and precision values.
These do not constitute independent libktx interoperability evidence.
"""
from pathlib import Path
import struct,hashlib,zlib
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith('.ktx2')]
def make(vk,channels,size,w,h,data,orientation='rd',compression=0,mips=()):
 srgb=vk in (29,36,43,50);isfloat=size==4;bgr=vk in (30,36,44,50)
 descriptor_size=24+16*channels
 dfd=struct.pack('<IIHH4B4B8B',descriptor_size+4,0,2,descriptor_size,1,1,2 if srgb else 1,0,0,0,0,0,size*channels,0,0,0,0,0,0,0)
 for c in range(channels):
  channel=15 if c==3 else 2-c if bgr else c
  qualifier=0xc0 if isfloat else 0x10 if srgb and c==3 else 0
  lower,upper=(0xbf800000,0x3f800000) if isfloat else (0,(1<<(size*8))-1)
  dfd+=struct.pack('<HBB4BII',c*size*8,size*8-1,channel|qualifier,0,0,0,0,lower,upper)
 entry=b'KTXorientation\0'+orientation.encode()+b'\0';kvd=struct.pack('<I',len(entry))+entry+bytes(-len(entry)%4)
 levels=[data,*mips];dfd_offset=80+24*len(levels);kvd_offset=dfd_offset+len(dfd)
 out=bytearray(bytes(kvd_offset+len(kvd)));out[dfd_offset:kvd_offset]=dfd;out[kvd_offset:]=kvd
 alignment={3:12,6:12}.get(channels*size,channels*size)
 indices=[]
 for source in reversed(levels):
  payload=zlib.compress(source) if compression else source
  if not compression:out+=bytes(-len(out)%alignment)
  indices.append((len(out),len(payload),len(source)));out+=payload
 header=b'\xabKTX 20\xbb\r\n\x1a\n'+struct.pack('<13I2Q',vk,size,w,h,0,0,1,len(levels),compression,dfd_offset,len(dfd),kvd_offset,len(kvd),0,0)
 out[:80]=header
 for i,index in enumerate(reversed(indices)):struct.pack_into('<3Q',out,80+24*i,*index)
 return bytes(out)
def record(name,data,w,h,rgba):
 (root/name).write_bytes(data);(root/(name+'.rgba')).write_bytes(rgba)
 lines.append(f'{name}\t{w}\t{h}\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
def generate_existing():
 from PIL import Image
 im=Image.new('RGBA',(3,2));im.putdata([(255,0,0,255),(0,255,0,128),(0,0,255,0),(17,29,43,64),(61,73,89,127),(250,240,230,255)])
 for orientation in ['rd','ru','ld','lu']:
  source=im
  if orientation[0]=='l':source=source.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
  if orientation[1]=='u':source=source.transpose(Image.Transpose.FLIP_TOP_BOTTOM)
  for compression in [0,3]:
   record(f'rgba8-{orientation}-{compression}.ktx2',make(43,4,1,3,2,source.tobytes(),orientation,compression),3,2,im.tobytes())
 for vk,channels,size in [(29,3,1),(50,4,1),(36,3,1)]:
  source=im if channels==4 else im.convert('RGB');data=source.tobytes()
  if vk in [50,36]:data=b''.join(bytes([data[i+2],data[i+1],data[i]])+data[i+3:i+channels] for i in range(0,len(data),channels))
  record(f'format-{vk}.ktx2',make(vk,channels,size,3,2,data),3,2,source.convert('RGBA').tobytes())
 record('mips.ktx2',make(43,4,1,3,2,im.tobytes(),mips=(bytes([20,40,60,255]),)),3,2,im.tobytes())
 for vk,size,values in [(91,2,[32768,1,65534,65535,32769,2,65535,65535]),(109,4,[0.5,0.1,0.3,1.0,2.0,0.2,0.4,1.0])]:
  for compression in [0,3]:
   data=b''.join(struct.pack('<'+('H' if size==2 else 'f'),v) for v in values)
   (root/f'precision-{size*8}-{compression}.ktx2').write_bytes(make(vk,4,size,2,1,data,compression=compression))
 (root/'manifest.tsv').write_text('\n'.join(lines)+'\n')

if __name__ == '__main__':
 generate_existing()
