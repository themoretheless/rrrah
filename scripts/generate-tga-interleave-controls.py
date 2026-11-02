"""Authored legacy TGA interleave controls; every full RGBA image checked by FFmpeg."""
from pathlib import Path
import subprocess,struct,json,hashlib
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster';rows=[]
def pixel(x,y,kind):
 q=x//2
 if kind=='gray':return bytes([(q*37+y*29)%256]*3+[255])
 return bytes([(q*41+y*19)%256,(q*23+y*47)%256,(q*67+y*13)%256,((q*53+y*31+64)%256) if kind=='rgba' else 255])
def packed(data,size):
 out=bytearray();p=[data[i:i+size] for i in range(0,len(data),size)];i=0
 while i<len(p):
  n=1
  while i+n<len(p) and n<128 and p[i+n]==p[i]:n+=1
  if n>1:out.append(128+n-1);out.extend(p[i]);i+=n;continue
  start=i;i+=1
  while i<len(p) and i-start<128 and not(i+1<len(p) and p[i]==p[i+1]):i+=1
  out.append(i-start-1);out.extend(b''.join(p[start:i]))
 return out
for kind in ['rgb','rgba','gray']:
 for height in [1,3,7]:
  width=7;oracle=b''.join(pixel(x,y,kind) for y in range(height) for x in range(width))
  oracle_path=root/f'tga-interleave-{kind}-h{height}.rgba8';oracle_path.write_bytes(oracle)
  for ways in [2,4]:
   for origin in range(4):
    for rle in [False,True]:
     header=bytearray(18);header[2]=(3 if kind=='gray' else 2)+(8 if rle else 0);header[12:16]=struct.pack('<HH',width,height)
     depth={'rgb':24,'rgba':32,'gray':8}[kind];header[16]=depth;header[17]=(64 if ways==2 else 128)|(origin<<4)|(8 if kind=='rgba' else 0)
     encoded=bytearray()
     for phase in range(ways):
      for row in range(phase,height,ways):
       y=row if origin&2 else height-1-row
       raw=bytearray()
       for x in (range(width-1,-1,-1) if origin&1 else range(width)):
        rgba=pixel(x,y,kind);raw.extend(rgba[:1] if kind=='gray' else (rgba[2::-1]+(rgba[3:4] if kind=='rgba' else b'')))
       encoded.extend(packed(raw,depth//8) if rle else raw)
     path=root/(f'tga-interleave-{kind}-h{height}-{ways}way-origin{origin}-'+('rle' if rle else 'raw')+'.tga')
     path.write_bytes(header+encoded)
     result=subprocess.run(['ffmpeg','-v','error','-i',str(path),'-frames:v','1','-pix_fmt','rgba','-f','rawvideo','pipe:1'],capture_output=True)
     assert result.returncode==0,(path.name,result.stderr.decode());assert result.stdout==oracle,path.name
     rows.append({'name':path.name,'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'width':width,'height':height,'rgba8_file':oracle_path.name,'rgba8_sha256':hashlib.sha256(oracle).hexdigest()})
(root/'tga-interleave-reference.json').write_text(json.dumps({'scope':'Authored legacy two/four-way interleaving; all4 origins, RGB/RGBA/gray, raw and mixed RLE packets, heights1/3/7. Every expected RGBA byte independently agrees with FFmpeg. No TGA2 footer/color calibration/producer originals.','ffmpeg':subprocess.check_output(['ffmpeg','-version'],text=True).splitlines()[0],'rows':rows},indent=2)+'\n')
print('Generated',len(rows),'sources; all independent FFmpeg images exact.')
