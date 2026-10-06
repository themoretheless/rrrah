"""CC0 authored animation controls, libwebp raw-frame readback, linear-light oracles."""
from pathlib import Path
from PIL import Image
import struct,subprocess,tempfile,hashlib
root=Path('tests/fixtures/raster');rows=[]
def chunk(k,d):return k+struct.pack('<I',len(d))+d+b'\0'*(len(d)&1)
def u24(n):return struct.pack('<I',n)[:3]
def linear(v):
 v=v/255
 return v/12.92 if v<=.04045 else ((v+.055)/1.055)**2.4
with tempfile.TemporaryDirectory() as tmp:
 tmp=Path(tmp)
 for case in range(4):
  bg=[0,0,0,0] if case<2 else [17,31,73,128]
  frames=[(0,0,5,3,[255,0,0,255],2,30),(2,0,2,2,[0,255,0,128],case&1,70),(0,0,2,1,[0,0,255,255],3,0),(0,0,5,3,[127,83,211,255],2,110)]
  canvas=[[linear(bg[0]),linear(bg[1]),linear(bg[2]),bg[3]/255] for _ in range(15)]
  chunks=chunk(b'VP8X',bytes([18,0,0,0])+u24(4)+u24(2))+chunk(b'ANIM',bytes([bg[2],bg[1],bg[0],bg[3]])+struct.pack('<H',case))
  oracles=[]
  for i,(x,y,w,h,pixel,flags,delay) in enumerate(frames):
   im=Image.new('RGBA',(w,h),tuple(pixel));png=tmp/'raw.png';im.save(png);webp=tmp/'raw.webp';readback=tmp/'readback.png'
   subprocess.run(['/opt/homebrew/bin/cwebp','-quiet']+(['-lossless','-exact'] if case<3 else ['-q','100'])+[str(png),'-o',str(webp)],check=True)
   subprocess.run(['/opt/homebrew/bin/dwebp',str(webp),'-o',str(readback)],check=True,capture_output=True)
   decoded=Image.open(readback).convert('RGBA')
   if case<3:assert decoded.tobytes()==im.tobytes()
   encoded=webp.read_bytes();data=b'';at=12
   while at<len(encoded):
    n=struct.unpack_from('<I',encoded,at+4)[0];end=at+8+n+(n&1)
    if encoded[at:at+4] in (b'VP8L',b'VP8 ',b'ALPH'):data+=encoded[at:end]
    at=end
   chunks+=chunk(b'ANMF',u24(x//2)+u24(y//2)+u24(w-1)+u24(h-1)+u24(delay)+bytes([flags])+data)
   raw=list(decoded.get_flattened_data())
   for py in range(y,y+h):
    for px in range(x,x+w):
     pixel=raw[(py-y)*w+px-x];src=[linear(pixel[0]),linear(pixel[1]),linear(pixel[2]),pixel[3]/255]
     at=py*5+px;dst=canvas[at]
     if flags&2:canvas[at]=src.copy()
     else:
      a=src[3]+dst[3]*(1-src[3]);canvas[at]=[(src[c]*src[3]+dst[c]*dst[3]*(1-src[3]))/a if a else 0 for c in range(3)]+[a]
   oracle=f'webp-animation-{case}-frame-{i}.rgba32f';(root/oracle).write_bytes(struct.pack('<60f',*(v for p in canvas for v in p)));oracles.append((i,delay,oracle))
   if flags&1:
    for py in range(y,y+h):
     for px in range(x,x+w):canvas[py*5+px]=[linear(bg[0]),linear(bg[1]),linear(bg[2]),bg[3]/255]
  name=f'webp-animation-{case}.webp';p=root/name;p.write_bytes(b'RIFF'+struct.pack('<I',len(chunks)+4)+b'WEBP'+chunks)
  for i,delay,oracle in oracles:rows.append('\t'.join(map(str,[name,i,5,3,delay,case,oracle,hashlib.sha256(p.read_bytes()).hexdigest()])))
(root/'webp-animation-manifest.tsv').write_text('# source\tindex\twidth\theight\tdelay_ms\tplays\tlinear_oracle\tsource_sha256\n'+'\n'.join(rows)+'\n')
