"""CC0 Cineon storage fixtures; OpenImageIO native sample readback oracle."""
from pathlib import Path
import struct,subprocess,hashlib
import OpenEXR,numpy as np
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[]
for bits in [8,10,16]:
 for be in [True,False]:
  for count in ([3] if bits==10 else [1,3]):
   for packing in ([5,6] if bits==10 else ([1] if bits==8 else [3])):
    e='>' if be else '<';w,h=5,3;mask=(1<<bits)-1
    b=bytearray(2048);b[:4]=bytes([0x80,0x2a,0x5f,0xd7]) if be else bytes([0xd7,0x5f,0x2a,0x80]);b[24:28]=b'V4.5'
    def put(at,v):struct.pack_into(e+'I',b,at,v)
    put(4,2048);put(8,1024);put(12,1024);b[193]=count
    for c in range(count):
     at=196+c*28;b[at+1]=0 if count==1 else c+1;b[at+2]=bits;put(at+4,w);put(at+8,h)
     struct.pack_into(e+'ffff',b,at+12,0,0,float(mask),2.048)
    b[681]=packing
    for y in range(h):
     s=[(i*43+y*257+1)&mask for i in range(w*count)]
     if bits==8:b+=bytes(s)
     elif bits==16:b+=struct.pack(e+'H'*len(s),*s)
     else:
      for at in range(0,len(s),3):
       v=sum(n<<((22 if packing==5 else 20)-j*10) for j,n in enumerate(s[at:at+3]));b+=struct.pack(e+'I',v)
    put(20,len(b));name=f'cineon-{bits}-{"be" if be else "le"}-{count}-p{packing}.cin';(root/name).write_bytes(b)
    exr=Path('/tmp')/('rrrah-'+name+'.exr')
    subprocess.run(['oiiotool','--no-autopremult',str(root/name),'-d','float','-o',str(exr)],check=True)
    with OpenEXR.File(str(exr),separate_channels=True) as f:
     channels=f.channels();names=['R','G','B'] if count==3 else [next(iter(channels))]*3
     codes=[np.rint(channels[n].pixels*65535).astype(np.uint32) for n in names]
     if bits==10:codes=[v>>6 for v in codes]
     elif bits==8:codes=[v//257 for v in codes]
     rgba=np.empty((h,w,4),dtype=np.uint8 if bits==8 else '<u2')
     for c,code in enumerate(codes):rgba[:,:,c]=code if bits==8 else ((code.astype(np.uint64)*65535+mask//2)//mask)
     rgba[:,:,3]=255 if bits==8 else 65535
     kind='u8' if bits==8 else 'u16';oracle=name+'.'+kind;(root/oracle).write_bytes(rgba.tobytes())
    exr.unlink()
    lines.append(f'{name}\t{w}\t{h}\t{kind}\t{oracle}\t{hashlib.sha256(b).hexdigest()}')
(root/'cineon-manifest.tsv').write_text('\n'.join(lines)+'\n')
