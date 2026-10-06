"""CC0 pixel-aligned AVIF crop fixtures, including seven orientations. Authored clap/ipma metadata on the
committed libavif RGB producer; Pillow/libavif supplies stored-pixel oracle,
then Pillow crops (3,5)-(11,11), or crops the orientation sources
at (2,1)-(6,3) before ImageOps.exif_transpose. Pillow itself does not apply clap on decode.
This writer deliberately supports only the pinned fixture's simple iloc layout.
"""
from pathlib import Path
import struct
from PIL import Image,ImageOps
import hashlib
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
def unpack(data):
 at=0
 while at<len(data):
  n=struct.unpack_from('>I',data,at)[0]
  assert 8<=n<=len(data)-at
  yield data[at+4:at+8],data[at+8:at+n]
  at+=n
 assert at==len(data)
def box(k,d):return struct.pack('>I',len(d)+8)+k+d
manifest=root/'manifest.tsv'
lines=[line for line in manifest.read_text().splitlines() if not line.startswith('avif-clean-aperture-')]
for name in ['avif-rgb.avif']+[f'avif-orientation-{i}.avif' for i in range(2,9)]:
 transformed=name!='avif-rgb.avif'
 aperture=(4,1,2,1,0,1,0,1) if transformed else (8,1,6,1,0xffffffff,1,0,1)
 b=(root/name).read_bytes();out=[]
 for k,p in unpack(b):
  if k==b'meta':
   children=[]
   for t,d in unpack(p[4:]):
    if t==b'iprp':
     props=[]
     for q,e in unpack(d):
      if q==b'ipco':
       property_list=list(unpack(e))
       count=len(property_list)
       e+=box(b'clap',struct.pack('>8I',*aperture))
      if q==b'ipma':
       # Existing single primary association in these committed producer fixtures.
       assert e[:8]==bytes.fromhex('0000000000000001') and e[8:10]==b'\0\1'
       associations=e[11:]
       # ispe must precede transformative properties; clap precedes rotation/mirror.
       at=next((i for i,index in enumerate(associations)
                if property_list[(index&127)-1][0] in (b'irot',b'imir')),len(associations))
       e=e[:10]+bytes([e[10]+1])+associations[:at]+bytes([0x80|(count+1)])+associations[at:]
      props.append(box(q,e))
     d=b''.join(props)
    children.append((t,d))
   new_size=4+sum(len(d)+8 for _,d in children)
   delta=new_size-len(p)
   adjusted=[]
   for t,d in children:
    if t==b'iloc':
     assert len(d)==22 and d[:8]==bytes.fromhex('0000000044000001')
     d=d[:14]+struct.pack('>I',struct.unpack_from('>I',d,14)[0]+delta)+d[18:]
    adjusted.append(box(t,d))
   p=p[:4]+b''.join(adjusted)
  out.append(box(k,p))
 output=f'avif-clean-aperture-orientation-{name.split("-")[-1]}' if transformed else 'avif-clean-aperture-integer.avif'
 cropped=root/output;cropped.write_bytes(b''.join(out))
 with Image.open(cropped) as im:
  print(name,'libavif dimensions:',im.size)
  actual=im.convert('RGBA').tobytes()
 with Image.open(root/name) as im:
  baseline=im.convert('RGBA')
  assert actual==baseline.tobytes(), 'metadata rewrite changed stored pixels'
  rect=(2,1,6,3) if transformed else (3,5,11,11)
  expected_image=ImageOps.exif_transpose(baseline.crop(rect))
  expected=expected_image.tobytes()
 print('stored pixels unchanged; Pillow leaves clap unapplied; cropped oracle bytes', len(expected))
 Path(str(cropped)+'.rgba').write_bytes(expected)

 lines.append(f'{cropped.name}\t{expected_image.width}\t{expected_image.height}\t3\t{cropped.name}.rgba\t{hashlib.sha256(cropped.read_bytes()).hexdigest()}')
manifest.write_text('\n'.join(lines)+'\n')
