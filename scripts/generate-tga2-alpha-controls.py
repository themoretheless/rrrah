"""Authored TGA2 alpha cases, independent OpenImageIO samples/math diagnostics."""
from pathlib import Path
import struct,json,hashlib
import OpenImageIO as oiio
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster';rows=[]
def sample(x,y,mode):
 i=(x//2+y*2)%6;a=[0,1,64,128,254,255][i];rgb=[(i*31+32)%256,(i*47+64)%256,(i*59+96)%256]
 if mode==4:rgb=[(c*a+127)//255 for c in rgb]
 return bytes(rgb+[a])
def normalized(p,mode):
 if mode in [0,1]:return p[:3]+b'\xff'
 if mode==4:return bytes([(c*255+p[3]//2)//p[3] if p[3] else 0 for c in p[:3]])+p[3:]
 return p
for mode in range(5):
 width,height=7,3;oracle=b''.join(normalized(sample(x,y,mode),mode) for y in range(height) for x in range(width))
 oracle_path=root/('tga2-alpha-'+hashlib.sha256(oracle).hexdigest()[:16]+'.rgba8');oracle_path.write_bytes(oracle)
 for origin in range(4):
  for rle in [False,True]:
   header=bytearray(18);header[2]=10 if rle else 2;header[12:16]=struct.pack('<HH',width,height);header[16]=32;header[17]=(origin<<4)|(0 if mode==0 else 8)
   encoded=bytearray()
   for y in (range(height) if origin&2 else range(height-1,-1,-1)):
    xs=list(range(width-1,-1,-1) if origin&1 else range(width));pixels=[sample(x,y,mode) for x in xs]
    if rle:
     # Literal packets plus repeated pairs, without crossing source rows.
     i=0
     while i<width:
      n=1
      while i+n<width and pixels[i+n]==pixels[i]:n+=1
      encoded.append((128 if n>1 else 0)+n-1);p=pixels[i];encoded.extend(p[2::-1]+p[3:]);i+=n
    else:
     for p in pixels:encoded.extend(p[2::-1]+p[3:])
   offset=18+len(encoded);extension=bytearray(495);extension[:2]=struct.pack('<H',495);extension[494]=mode
   path=root/(f'tga2-alpha-mode{mode}-origin{origin}-'+('rle' if rle else 'raw')+'.tga');path.write_bytes(header+encoded+extension+struct.pack('<II',offset,0)+b'TRUEVISION-XFILE.\0')
   config=oiio.ImageSpec();config.attribute('oiio:UnassociatedAlpha',1);inp=oiio.ImageInput.open(str(path),config);assert inp,oiio.geterror()
   assert inp.spec().get_int_attribute('targa:alpha_type',0)==mode
   data=inp.read_image(format=oiio.UINT8);inp.close();stored=b''.join(sample(x,y,mode) for y in range(height) for x in range(width));assert data.tobytes()==stored
   transformed=oiio.ImageBufAlgo.unpremult(oiio.ImageBuf(data)).get_pixels(oiio.UINT8).tobytes() if mode==4 else data.tobytes()
   mismatch=sum(a!=b for a,b in zip(transformed,oracle));maximum=max(abs(a-b) for a,b in zip(transformed,oracle))
   if mode in [2,3]:assert mismatch==0
   if mode==4:assert maximum<=1
   rows.append({'name':path.name,'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'width':width,'height':height,'rgba8_file':oracle_path.name,'rgba8_sha256':hashlib.sha256(oracle).hexdigest(),'alpha_type':mode,'oiio_reference':'raw sample and alpha-type metadata exact; explicit OIIO unpremult for mode4; modes0/1 reference ignores footer opacity semantics','oiio_different_bytes_after_explicit_unpremult':mismatch,'oiio_max_sample_error':maximum})
(root/'tga2-alpha-reference.json').write_text(json.dumps({'scope':'Authored RGBA32 TGA2 Attributes Type0..4; all origins/raw-RLE; native normalization oracle from explicit integer formulas. Mode2 only undefined-byte retention, not intended appearance. OpenImageIO raw samples and metadata independently exact; mode4 independently unpremultiplied within1; modes0/1 raw-output opacity discrepancy retained.','openimageio':oiio.VERSION_STRING,'rows':rows},indent=2)+'\n')
print('Generated',len(rows),'TGA2 sources; independent raw samples/metadata and mode4 unpremult checked; modes0/1 opacity discrepancy explicit.')
