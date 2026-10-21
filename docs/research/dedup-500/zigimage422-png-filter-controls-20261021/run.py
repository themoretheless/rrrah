import struct,zlib,subprocess,json,hashlib
from pathlib import Path
folder=Path(__file__).resolve().parent;root=Path('.dedup-work/zigimage422-20261021');binary=root/'png-filter-control';subprocess.run(['clang','-std=c99','-O2','-I',str(root/'include'),str(folder/'control.c'),'-lm','-o',str(binary)],check=True)
def chunk(t,p):return struct.pack('>I',len(p))+t+p+struct.pack('>I',zlib.crc32(t+p)&0xffffffff)
def paeth(a,b,c):
 p=a+b-c;ds=[abs(p-a),abs(p-b),abs(p-c)];return [a,b,c][ds.index(min(ds))]
rows=[];w,h=4,3
for channels,color in [(1,0),(3,2),(4,6)]:
 source=[bytes((i*37+y*61+13)%256 for i in range(w*channels)) for y in range(h)];expected=bytearray()
 for row in source:
  for x in range(w):
   px=row[x*channels:(x+1)*channels];expected.extend(bytes([px[0]]*3+[255]) if channels==1 else px+bytes([255]) if channels==3 else px)
 for kind in range(5):
  filtered=bytearray();prior=bytes(w*channels)
  for row in source:
   filtered.append(kind)
   for i,v in enumerate(row):
    a=row[i-channels] if i>=channels else 0;b=prior[i];c=prior[i-channels] if i>=channels else 0;predict=[0,a,b,(a+b)//2,paeth(a,b,c)][kind];filtered.append((v-predict)%256)
   prior=row
  data=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,8,color,0,0,0))+chunk(b'IDAT',zlib.compress(filtered))+chunk(b'IEND',b'');f=root/'png-filter.png';f.write_bytes(data);p=subprocess.run([str(binary),str(f)],capture_output=True,check=True);assert p.stdout==expected,(channels,kind)
  rows.append(dict(channels=channels,filter=kind,pixels_exact=True,sha256=hashlib.sha256(data).hexdigest()))
out=dict(cases=rows,passed=len(rows),scope='15 authored8bitPNGknownRGBAoraclecases,all5rowfiltertypeswithfirst/priorrowandgray/RGB/RGBA;actualoriginalCfilenameAPI. No Adam7/sub8/16bit/palette/integrity/comprehensivePNGqualification.')
(folder/'result.json').write_text(json.dumps(out,indent=2)+'\n');print('passed',len(rows))
