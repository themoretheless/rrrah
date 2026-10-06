"""Independent authored straight-alpha PNG/TIFF matrix, CC0 pixel formula."""
from pathlib import Path
import json,struct,sys,zlib,hashlib
folder=Path(sys.argv[1]);folder.mkdir(parents=True,exist_ok=True)
def chunk(kind,data):
 return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
records=[]
for side in (1024,2048,4096):
 tags=[(256,4,1,side),(257,4,1,side),(258,3,4,158),(259,3,1,1),(262,3,1,2),(273,4,1,166),(274,3,1,1),(277,3,1,4),(278,4,1,side),(279,4,1,side*side*4),(284,3,1,1),(338,3,1,2)]
 header=b'II'+struct.pack('<HIH',42,8,len(tags))+b''.join(struct.pack('<HHII',*t) for t in tags)+struct.pack('<I4H',0,8,8,8,8);assert len(header)==166
 path=folder/f'{side}-base.tiff';file=path.open('wb');file.write(header)
 compressors={v:zlib.compressobj() for v in ('base','hidden','changed')};parts={v:[] for v in compressors}
 for y in range(side):
  row=bytearray(value for x in range(side) for value in ((x+y)%239,y%241,x%251,(x%4)*85));file.write(row)
  hidden=bytearray(row)
  for x in range(0,side,4):
   at=x*4;hidden[at:at+3]=bytes(255-v for v in hidden[at:at+3])
  changed=bytearray(row)
  if y==side//2:changed[4]^=1
  for variant,data in [('base',row),('hidden',hidden),('changed',changed)]:parts[variant].append(compressors[variant].compress(b'\0'+data))
 file.close();records.append({'name':path.name,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
 for variant in compressors:
  parts[variant].append(compressors[variant].flush());data=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',side,side,8,6,0,0,0))+chunk(b'sRGB',b'\0')+chunk(b'IDAT',b''.join(parts[variant]))+chunk(b'IEND',b'');path=folder/f'{side}-{variant}.png';path.write_bytes(data);records.append({'name':path.name,'sha256':hashlib.sha256(data).hexdigest()})
(folder/'manifest.json').write_text(json.dumps(records,indent=2)+'\n')
