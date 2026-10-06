"""Authored CC0 APNG frame controls; FFmpeg compatibility readback and authored linear-light presentation oracles."""
from pathlib import Path
import struct, zlib, hashlib, subprocess
ROOT = Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
def chunk(kind,data):
 return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
def encoded(w,h,pixel):
 return zlib.compress(b''.join(b'\0'+bytes(pixel)*w for _ in range(h)))
lines=['# source\tframe\twidth\theight\tdelay_num\tdelay_den\tplays\toracle\tsha256']
frames=[(5,3,0,0,(255,0,0,255),0,0,1,10),
        (2,1,1,1,(0,255,0,128),2,1,3,100),
        (1,2,3,0,(0,0,255,255),1,0,1,0),
        (2,1,0,2,(255,255,0,255),0,0,0,100)]
for thumbnail,plays in [(False,0),(False,3),(True,2)]:
 source=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',5,3,8,6,0,0,0))+chunk(b'sRGB',b'\0')+chunk(b'acTL',struct.pack('>2I',4,plays))
 if thumbnail:source+=chunk(b'IDAT',encoded(5,3,(17,29,43,255)))
 sequence=0
 for index,(w,h,x,y,pixel,dispose,blend,num,den) in enumerate(frames):
  source+=chunk(b'fcTL',struct.pack('>5I2H2B',sequence,w,h,x,y,num,den,dispose,blend));sequence+=1
  if index==0 and not thumbnail:source+=chunk(b'IDAT',encoded(w,h,pixel))
  else:source+=chunk(b'fdAT',struct.pack('>I',sequence)+encoded(w,h,pixel));sequence+=1
 source+=chunk(b'IEND',b'');name=f'apng-compose-{int(thumbnail)}-{plays}.apng';(ROOT/name).write_bytes(source)
 raw=subprocess.check_output(['ffmpeg','-v','error','-ignore_loop','1','-i',str(ROOT/name),'-fps_mode','passthrough','-f','rawvideo','-pix_fmt','rgba','pipe:1'])
 assert len(raw)==4*5*3*4, len(raw)
 canvas=[[0.,0.,0.,0.] for _ in range(15)]
 for index,frame in enumerate(frames):
  reference=raw[index*60:(index+1)*60]
  (ROOT/f'{name}.frame{index}.compat-rgba').write_bytes(reference)
  w,h,x,y,pixel,dispose,blend,num,den=frame
  previous=[p[:] for p in canvas]
  def linear(v):
   v=v/255.
   return v/12.92 if v<=.04045 else ((v+.055)/1.055)**2.4
  p=[linear(v) for v in pixel[:3]]+[pixel[3]/255.]
  for yy in range(h):
   for xx in range(w):
    at=(y+yy)*5+x+xx;d=canvas[at]
    if blend==0: canvas[at]=p[:]
    else:
     alpha=p[3]+d[3]*(1-p[3])
     canvas[at]=[(p[c]*p[3]+d[c]*d[3]*(1-p[3]))/alpha for c in range(3)]+[alpha] if alpha else [0.,0.,0.,0.]
  oracle=struct.pack('<60f',*(v for p in canvas for v in p));target=f'{name}.frame{index}.rgba32f';(ROOT/target).write_bytes(oracle)
  lines.append(f'{name}\t{index}\t5\t3\t{num*1000}\t{den or 100}\t{plays}\t{target}\t{hashlib.sha256(source).hexdigest()}')
  if dispose==2:canvas=previous
  elif dispose==1:
   for yy in range(h):
    for xx in range(w):canvas[(y+yy)*5+x+xx]=[0.,0.,0.,0.]
(ROOT/'apng-manifest.tsv').write_text('\n'.join(lines)+'\n')
