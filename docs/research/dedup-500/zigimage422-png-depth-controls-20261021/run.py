import struct,zlib,subprocess,json,hashlib
from pathlib import Path
folder=Path(__file__).resolve().parent;root=Path('.dedup-work/zigimage422-20261021');fixture=root/'depth-fixtures';fixture.mkdir(exist_ok=True)
def chunk(t,p):return struct.pack('>I',len(p))+t+p+struct.pack('>I',zlib.crc32(t+p)&0xffffffff)
paths=[];pins={}
for value in [0x1200,0x12ff]:
 data=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,16,0,0,0,0))+chunk(b'IDAT',zlib.compress(b'\0'+struct.pack('>H',value)))+chunk(b'IEND',b'');p=fixture/f'{value}.png';p.write_bytes(data);paths.append(str(p));pins[p.name]=dict(value=value,sha256=hashlib.sha256(data).hexdigest())
binary=root/'png-depth-control';subprocess.run(['clang','-std=c99','-O2','-I',str(root/'include'),str(folder/'control.c'),'-lm','-o',str(binary)],check=True);p=subprocess.run([str(binary),*paths],capture_output=True,text=True,check=True);out=dict(result=json.loads(p.stdout),fixtures=pins,scope='Two authored valid grayscale16 PNG with explicit big-endian samples/CRC. Original pinned C decoder16/8/float filename APIs, no Zig bindings or full hash call. Equalfloatvalues reflect earlier8bitconversion, not exact pixel identity.');(folder/'result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
