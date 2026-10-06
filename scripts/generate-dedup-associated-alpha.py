"""CC0 associated-alpha TIFF and losslessly equivalent straight-alpha PNG."""
from pathlib import Path
import struct,zlib,json,hashlib
folder=Path(__file__).resolve().parent.parent/'crates/rrrah-dedup/tests/fixtures/associated-alpha';folder.mkdir(parents=True,exist_ok=True)
side=8
straight=bytearray();associated=bytearray()
for y in range(side):
 for x in range(side):
  alpha=(x%4)*85;rgb=[255 if (x+y)%3==c else 0 for c in range(3)]
  straight.extend(rgb+[alpha]);associated.extend([value*alpha//255 for value in rgb]+[alpha])
def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',side,side,8,6,0,0,0))+chunk(b'sRGB',b'\0')+chunk(b'IDAT',zlib.compress(b''.join(b'\0'+straight[y*side*4:(y+1)*side*4] for y in range(side))))+chunk(b'IEND',b'')
tags=[(256,4,1,side),(257,4,1,side),(258,3,4,158),(259,3,1,1),(262,3,1,2),(273,4,1,166),(274,3,1,1),(277,3,1,4),(278,4,1,side),(279,4,1,len(associated)),(284,3,1,1),(338,3,1,1)]
tiff=b'II'+struct.pack('<HIH',42,8,len(tags))+b''.join(struct.pack('<HHII',*t) for t in tags)+struct.pack('<I4H',0,8,8,8,8)+associated
files={'straight.png':png,'associated.tiff':tiff}
tags16=[(tag,kind,count,512 if tag==279 else value) for tag,kind,count,value in tags]
files['associated16.tiff']=b'II'+struct.pack('<HIH',42,8,len(tags16))+b''.join(struct.pack('<HHII',*t) for t in tags16)+struct.pack('<I4H',0,16,16,16,16)+struct.pack('<256H',*(v*257 for v in associated))
tags32=[(tag,kind,count,170 if tag==258 else 186 if tag==273 else 1024 if tag==279 else value) for tag,kind,count,value in tags]+[(339,3,4,178)]
files['associated-float.tiff']=b'II'+struct.pack('<HIH',42,8,len(tags32))+b''.join(struct.pack('<HHII',*t) for t in tags32)+struct.pack('<I8H',0,32,32,32,32,3,3,3,3)+struct.pack('<256f',*(v/255 for v in associated))
for name,data in files.items():(folder/name).write_bytes(data)
(folder/'manifest.json').write_text(json.dumps({name:hashlib.sha256(data).hexdigest() for name,data in files.items()},indent=2)+'\n')

# Full endian/layout matrix; BigTIFF stores the four SHORT values inline.
def matrix_tiff(bits, little, big):
 endian='<' if little else '>'
 samples=bytes(associated) if bits==8 else struct.pack(endian+'256H',*(v*257 for v in associated)) if bits==16 else struct.pack(endian+'256f',*(v/255 for v in associated))
 n=13 if bits==32 else 12
 end=(16+8+n*20+8) if big else (8+2+n*12+4)
 data_offset=end if big else end+8+(8 if bits==32 else 0)
 entries=[(256,4,1,side),(257,4,1,side),(258,3,4,bits),(259,3,1,1),(262,3,1,2),(273,16 if big else 4,1,data_offset),(274,3,1,1),(277,3,1,4),(278,4,1,side),(279,4,1,len(samples)),(284,3,1,1),(338,3,1,1)]
 if bits==32:entries.append((339,3,4,3))
 encoded=[]
 for tag,kind,count,value in entries:
  if count==4:
   payload=struct.pack(endian+'4H',*([value]*4)) if big else struct.pack(endian+'I',end+(8 if tag==339 else 0))
  else:
   payload=struct.pack(endian+('H' if kind==3 else 'Q' if kind==16 else 'I'),value)
   payload+=bytes((8 if big else 4)-len(payload))
  encoded.append(struct.pack(endian+('HHQ' if big else 'HHI'),tag,kind,count)+payload)
 magic=b'II' if little else b'MM'
 header=magic+(struct.pack(endian+'HHHQ',43,8,0,16) if big else struct.pack(endian+'HI',42,8))
 data=header+struct.pack(endian+('Q' if big else 'H'),n)+b''.join(encoded)+bytes(8 if big else 4)
 if not big:
  data+=struct.pack(endian+'4H',*([bits]*4))
  if bits==32:data+=struct.pack(endian+'4H',3,3,3,3)
 assert len(data)==data_offset
 return data+samples
for bits in (8,16,32):
 for little in (True,False):
  for big in (False,True):
   name=f'associated-{bits}-{"little" if little else "big"}-{"bigtiff" if big else "classic"}.tiff';data=matrix_tiff(bits,little,big);files[name]=data;(folder/name).write_bytes(data)
(folder/'manifest.json').write_text(json.dumps({name:hashlib.sha256(data).hexdigest() for name,data in files.items()},indent=2)+'\n')
