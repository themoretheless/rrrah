"""CC0 Wavefront RLA byte-plane fixtures with OpenImageIO native oracles."""
from pathlib import Path
import struct,subprocess,hashlib
import OpenEXR,numpy as np
r=Path(__file__).resolve().parent.parent/'tests/fixtures/raster';lines=[]
for bits,color,alpha in [(8,3,0),(8,3,8),(16,3,16),(16,3,8),(8,3,16),(8,1,8),(16,1,16)]:
 for mixed in [False,True]:
  w,h=140,3;b=bytearray(740+h*4)
  for offset in [0,8]:struct.pack_into('>hhhh',b,offset,0,w-1,0,h-1)
  struct.pack_into('>hhhhh',b,18,int(bits==16),color,int(alpha!=0),0,-2)
  struct.pack_into('>hhh',b,658,bits,int(alpha==16),alpha)
  def encode(values):
   if mixed:return bytes([128])+bytes(values[:128])+bytes([11,values[128]])
   return bytes([128])+bytes(values[:128])+bytes([244])+bytes(values[128:])
  for row in [2,0,1]:
   struct.pack_into('>I',b,740+row*4,len(b))
   for channel in range(color+int(alpha!=0)):
    depth=bits if channel<color else alpha;mask=(1<<depth)-1
    vals=[((x//20 if mixed else x)*17+channel*43+(h-1-row)*131+1)&mask for x in range(w)]
    planes=[[v>>8 for v in vals],[v&255 for v in vals]] if depth==16 else [vals]
    encoded=b''.join(encode(v) for v in planes);b+=struct.pack('>H',len(encoded))+encoded
  name=f'rla-{bits}-c{color}-a{alpha}-mixed{int(mixed)}.rla';(r/name).write_bytes(b)
  exr=Path('/tmp')/('rrrah-'+name+'.exr');subprocess.run(['oiiotool','--no-autopremult',str(r/name),'-d','float','-o',str(exr)],check=True)
  depth=max(bits,alpha);maximum=255 if depth==8 else 65535;out=np.full((h,w,4),maximum,dtype=np.uint8 if depth==8 else '<u2')
  with OpenEXR.File(str(exr),separate_channels=True) as f:
   ch=f.channels();names=['R','G','B'] if color==3 else ['R']*3
   for c,n in enumerate(names):out[:,:,c]=np.rint(ch[n].pixels*maximum)
   if alpha:out[:,:,3]=np.rint(ch['A' if color==3 else 'G'].pixels*maximum)
  exr.unlink();kind='u8' if depth==8 else 'u16';oracle=name+'.'+kind;(r/oracle).write_bytes(out.tobytes());lines.append(f'{name}\t{w}\t{h}\t{kind}\t{oracle}\t{hashlib.sha256(b).hexdigest()}')
(r/'rla-manifest.tsv').write_text('\n'.join(lines)+'\n')
