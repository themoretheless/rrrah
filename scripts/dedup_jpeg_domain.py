"""Independent JPEG EXIF IFD0 orientation reader for diagnostic domain bounds."""
import struct
from pathlib import Path

def oriented_dimensions(path,stored):
 data=Path(path).read_bytes();assert data[:2]==b'\xff\xd8';pos=2;orientations=[]
 while pos<len(data):
  assert data[pos]==255
  while data[pos]==255:pos+=1
  marker=data[pos];pos+=1
  if marker in (0xda,0xd9):break
  assert marker not in range(0xd0,0xd9) and marker!=1
  size=int.from_bytes(data[pos:pos+2],'big');assert size>=2 and pos+size<=len(data)
  payload=data[pos+2:pos+size];pos+=size
  if marker!=0xe1 or not payload.startswith(b'Exif\x00\x00'):continue
  t=payload[6:];assert len(t)>=8 and t[:2] in (b'II',b'MM');order='<' if t[:2]==b'II' else '>'
  def read(fmt,offset):return struct.unpack_from(order+fmt,t,offset)[0]
  assert read('H',2)==42;offset=read('I',4);assert offset>=8 and offset+2<=len(t)
  count=read('H',offset);assert offset+2+count*12+4<=len(t)
  for index in range(count):
   entry=offset+2+index*12
   if read('H',entry)!=274:continue
   assert read('H',entry+2)==3 and read('I',entry+4)==1
   value=read('H',entry+8);assert 1<=value<=8;orientations.append(value)
 assert len(orientations)<=1
 orientation=orientations[0] if orientations else 1
 assert len(stored)==2 and all(type(v) is int and v>0 for v in stored)
 return tuple(reversed(stored)) if orientation>=5 else tuple(stored)
