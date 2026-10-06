"""CC0 authored JNG containers with independent Pillow JPEG sample readback."""
from pathlib import Path
from PIL import Image
import io,struct,zlib,hashlib
root=Path('tests/fixtures/raster');rows=[]
def chunk(k,d):return struct.pack('>I',len(d))+k+d+struct.pack('>I',zlib.crc32(k+d))
w,h=8,3
for mode,alpha,progressive in [('RGB',0,False),('L',0,True),('RGB',1,False),('RGB',2,True),('RGB',4,False),('RGB',8,True),('RGB',16,False),('L',8,False)]:
 jpeg_alpha=mode=='L' and alpha==8
 source=Image.new(mode,(w,h));source.putdata([(x*29,y*67,(x+y)*21) if mode=='RGB' else x*29 for y in range(h) for x in range(w)])
 buf=io.BytesIO();source.save(buf,format='JPEG',quality=95,subsampling=0,progressive=progressive);jpeg=buf.getvalue();rgb=list(Image.open(io.BytesIO(jpeg)).convert('RGB').get_flattened_data())
 aa=[((x+y)%(1<<alpha)) if alpha and alpha<=4 else (((x+y)*31)%256 if alpha==8 else 32768+x+y) for y in range(h) for x in range(w)] if alpha else [255]*(w*h)
 alphachunks=b''
 if jpeg_alpha:
  im=Image.new('L',(w,h));im.putdata(aa);buf=io.BytesIO();im.save(buf,format='JPEG',quality=95);adata=buf.getvalue();aa=list(Image.open(io.BytesIO(adata)).get_flattened_data());alphachunks=chunk(b'JDAA',adata)
 elif alpha:
  scan=b''
  for y in range(h):
   row=aa[y*w:(y+1)*w]
   if alpha==16:data=b''.join(struct.pack('>H',v) for v in row)
   elif alpha==8:data=bytes(row)
   else:
    data=bytes(sum(row[x+j]<<(8-alpha*(j+1)) for j in range(8//alpha)) for x in range(0,w,8//alpha))
   scan+=b'\0'+data
  alphachunks=chunk(b'IDAT',zlib.compress(scan))
 ctype=(8 if mode=='L' else 10)+(4 if alpha else 0)
 hdr=struct.pack('>II8B',w,h,ctype,8,8,8 if progressive else 0,alpha,8 if jpeg_alpha else 0,0,0)
 name=f'jng-{mode}-{alpha}-{int(progressive)}.jng';p=root/name
 p.write_bytes(b'\x8bJNG\r\n\x1a\n'+chunk(b'JHDR',hdr)+chunk(b'sRGB',b'\0')+chunk(b'JDAT',jpeg[:len(jpeg)//2])+alphachunks+chunk(b'JDAT',jpeg[len(jpeg)//2:])+chunk(b'IEND',b''))
 out=[]
 for i,pixel in enumerate(rgb):
  av=aa[i] if alpha==16 else aa[i]*65535//((1<<alpha)-1) if alpha else 65535
  out.extend([v*257 for v in pixel]+[av])
 oracle=name+'.rgba16';(root/oracle).write_bytes(struct.pack('<'+str(len(out))+'H',*out))
 rows.append('\t'.join(map(str,[name,w,h,alpha,jpeg_alpha,oracle,hashlib.sha256(p.read_bytes()).hexdigest()])))
(root/'jng-manifest.tsv').write_text('# source\twidth\theight\talpha_depth\tjpeg_alpha\toracle\tsha256\n'+'\n'.join(rows)+'\n')
