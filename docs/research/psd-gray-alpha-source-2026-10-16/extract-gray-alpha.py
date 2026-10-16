#!/usr/bin/env python3
from pathlib import Path
import sys, struct
p=Path(sys.argv[1]);b=p.read_bytes();version=int.from_bytes(b[4:6],'big');channels,h,w,depth,mode=struct.unpack('>HIIHH',b[12:26]);assert (channels,depth,mode)==(2,8,1)
at=26
for section in range(3):
 size=8 if version==2 and section==2 else 4;n=int.from_bytes(b[at:at+size],'big');start=at+size
 if section==1:
  resources=b[start:start+n];pos=0
  while pos<len(resources):
   assert resources[pos:pos+4]==b'8BIM';id=int.from_bytes(resources[pos+4:pos+6],'big');pos+=6;pos+=(resources[pos]+2)&~1;length=int.from_bytes(resources[pos:pos+4],'big');pos+=4
   if id==1039:Path(str(p)+'.icc').write_bytes(resources[pos:pos+length])
   pos+=length+length%2
 if section==2:assert int.from_bytes(b[start+size:start+size+2],'big',signed=True)<0
 at=start+n
assert b[at:at+2]==b'\0\1';at+=2;size=4 if version==2 else 2
lengths=[int.from_bytes(b[at+i*size:at+(i+1)*size],'big') for i in range(channels*h)];at+=size*len(lengths);raw=bytearray()
for length in lengths:
 row=b[at:at+length];at+=length;i=0;out=bytearray()
 while i<len(row):
  code=row[i];i+=1
  if code<128:out+=row[i:i+code+1];i+=code+1
  elif code>128:out+=bytes([row[i]])*(257-code);i+=1
 assert len(out)==w;raw+=out
assert at==len(b)
rgba=bytearray()
for value,alpha in zip(raw[:w*h],raw[w*h:]):
 straight=min(255,max(0,value+alpha-255)*255//alpha) if alpha else value
 rgba+=bytes([straight,straight,straight,alpha])
Path(str(p)+'.rgba').write_bytes(rgba)
