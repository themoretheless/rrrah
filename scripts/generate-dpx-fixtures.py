"""CC0 authored DPX storage fixtures, independent FFmpeg sample oracles."""
from pathlib import Path
import struct, subprocess, hashlib
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[]
for bits,packs in [(8,[0]),(10,[1,2]),(12,[0,1,2]),(16,[0])]:
 for endian in ['be','le']:
  for desc in ([6,50,51,52] if bits==8 else ([6,50,51] if bits==16 else [50])):
   for packing in packs:
    w,h=5,3;c={6:1,50:3,51:4,52:4}[desc];mask=(1<<bits)-1
    e='>' if endian=='be' else '<'
    b=bytearray(2048);b[:4]=b'SDPX' if endian=='be' else b'XPDS';b[8:12]=b'V2.0'
    def u32(at,v):struct.pack_into(e+'I',b,at,v)
    def u16(at,v):struct.pack_into(e+'H',b,at,v)
    u32(4,2048);u32(24,1664);u32(28,384);u32(660,0xffffffff)
    u16(770,1);u32(772,w);u32(776,h);b[800:804]=bytes([desc,255,255,bits]);u16(804,packing);u32(808,2048)
    for y in range(h):
     s=[(i*17+y*131+1)&mask for i in range(w*c)]
     if bits==8:b+=bytes(s)
     elif bits==16:b+=struct.pack(e+'H'*len(s),*s)
     elif bits==10:
      for at in range(0,len(s),3):
       group=s[at:at+3]+[0]*max(0,3-len(s[at:at+3]))
       v=sum(n<<(22-j*10 if packing==1 else 20-j*10) for j,n in enumerate(group));b+=struct.pack(e+'I',v)
     elif packing:b+=struct.pack(e+'H'*len(s),*[v<<(4 if packing==1 else 0) for v in s])
     else:
      v=sum(n<<(i*12) for i,n in enumerate(s))
      for i in range((len(s)*12+31)//32):b+=struct.pack(e+'I',(v>>(i*32))&0xffffffff)
    u32(16,len(b));name=f'dpx-{bits}-{endian}-{desc}-p{packing}.dpx';(root/name).write_bytes(b)
    fmt=('gray' if desc==6 else 'rgba') if bits==8 else ('gray16le' if desc==6 else ('gbrp'+str(bits)+'le' if bits in [10,12] else 'rgba64le'))
    raw=subprocess.check_output(['ffmpeg','-v','error','-i',str(root/name),'-f','rawvideo','-pix_fmt',fmt,'-'])
    if bits==8:
     out=raw if desc!=6 else bytes(v for n in raw for v in (n,n,n,255));kind='u8'
    else:
     values=struct.unpack('<'+'H'*(len(raw)//2),raw);rgba=[]
     if bits in [10,12]:
      for i in range(w*h):rgba += [round(values[w*h*2+i]*65535/mask),round(values[i]*65535/mask),round(values[w*h+i]*65535/mask),65535]
     elif desc==6:
      for n in values:rgba += [n,n,n,65535]
     else:rgba=list(values)
     out=struct.pack('<'+'H'*len(rgba),*rgba);kind='u16'
    oracle=name+'.'+kind;(root/oracle).write_bytes(out)
    lines.append(f'{name}\t{w}\t{h}\t{kind}\t{oracle}\t{hashlib.sha256(b).hexdigest()}')
(root/'dpx-manifest.tsv').write_text('\n'.join(lines)+'\n')
