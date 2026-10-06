"""Author sRGB PNGs and untagged RGB TIFFs independently from the BMP probe's specified pixel formula."""
from pathlib import Path
import hashlib,json,struct,sys,zlib
folder=Path(sys.argv[1]);folder.mkdir(parents=True,exist_ok=True)
def chunk(kind,data):
 return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
records=[]
for side in (1024,2048,4096):
 tags=[(256,4,1,side),(257,4,1,side),(258,3,3,146),(259,3,1,1),(262,3,1,2),(273,4,1,152),(274,3,1,1),(277,3,1,3),(278,4,1,side),(279,4,1,side*side*3),(284,3,1,1)]
 tiff_header=b'II'+struct.pack('<HIH',42,8,len(tags))+b''.join(struct.pack('<HHII',*tag) for tag in tags)+struct.pack('<I3H',0,8,8,8)
 assert len(tiff_header)==152
 tiff_path=folder/f'{side}.tiff';tiff_file=tiff_path.open('wb');tiff_file.write(tiff_header)
 compressor=zlib.compressobj();parts=[]
 for y in reversed(range(side)):
  row=bytes(value for x in range(side) for value in ((x+y)%239,y%241,x%251))
  tiff_file.write(row)
  parts.append(compressor.compress(b'\0'+row))
 tiff_file.close();records.append({'name':tiff_path.name,'side':side,'sha256':hashlib.sha256(tiff_path.read_bytes()).hexdigest(),'bytes':tiff_path.stat().st_size})
 parts.append(compressor.flush());encoded=b''.join(parts)
 data=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',side,side,8,2,0,0,0))+chunk(b'sRGB',b'\0')+chunk(b'IDAT',encoded)+chunk(b'IEND',b'')
 name=f'{side}.png';(folder/name).write_bytes(data);records.append({'name':name,'side':side,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)})
(folder/'manifest.json').write_text(json.dumps(records,indent=2)+'\n')
