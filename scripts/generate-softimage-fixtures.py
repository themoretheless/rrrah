"""CC0 PIC packet fixtures; OpenImageIO sample oracle with no premultiplication."""
from pathlib import Path
import struct,subprocess,hashlib
import OpenEXR,numpy as np
r=Path(__file__).resolve().parent.parent/'tests/fixtures/raster';lines=[]
for bits,alpha in [(8,False),(8,True),(16,True),(16,16)]:
 for mode in [0,1,2]:
  w,h=140,3;b=bytearray(104);b[:4]=bytes([0x53,0x80,0xf6,0x34]);struct.pack_into('>f',b,4,1);b[88:92]=b'PICT';struct.pack_into('>HHfH',b,92,w,h,1,3)
  packets=[(bits,0xe0)]+([(16 if alpha==16 else 8,0x10)] if alpha else [])
  for i,(depth,mask) in enumerate(packets):b+=bytes([int(i<len(packets)-1),depth,mode,mask])
  for y in range(h):
   for depth,mask in packets:
    c=3 if mask==0xe0 else 1
    values=[((x if mode==0 else x//20)*29+y*67+j*43)%((1<<depth)-1) for x in range(w) for j in range(c)]
    def pix(x):return bytes(values[x*c:x*c+c]) if depth==8 else struct.pack('>'+'H'*c,*values[x*c:x*c+c])
    if mode==0:
     for x in range(w):b+=pix(x)
    elif mode==1:
     for x in range(0,w,20):b+=bytes([20])+pix(x)
    else:
     b+=bytes([128])+struct.pack('>H',20)+pix(0)
     b+=bytes([39])+b''.join(pix(x) for x in range(20,60))
     for x in range(60,w,20):b+=bytes([147])+pix(x)
  name=f'softimage-{bits}-a{int(alpha)}-rle{mode}.pic';(r/name).write_bytes(b)
  exr=Path('/tmp')/('rrrah-'+name+'.exr');subprocess.run(['oiiotool','--no-autopremult',str(r/name),'-d','float','-o',str(exr)],check=True)
  with OpenEXR.File(str(exr),separate_channels=True) as f:
   channels=f.channels();dtype=np.uint8 if bits==8 else '<u2';maximum=255 if bits==8 else 65535;out=np.full((h,w,4),maximum,dtype=dtype)
   for j,n in enumerate(['R','G','B','A']):
    if n in channels:out[:,:,j]=np.rint(channels[n].pixels*maximum)
  exr.unlink();kind='u8' if bits==8 else 'u16';oracle=name+'.'+kind;(r/oracle).write_bytes(out.tobytes());lines.append(f'{name}\t{w}\t{h}\t{kind}\t{oracle}\t{hashlib.sha256(b).hexdigest()}')
(r/'softimage-manifest.tsv').write_text('\n'.join(lines)+'\n')
