"""CC0 MacPaint data-fork fixtures; Netpbm macptopbm and Pillow oracle.
Requires Netpbm 11.02.30 (used here) and Pillow; neither is a runtime dependency.
"""
from pathlib import Path
import hashlib,subprocess,io,struct,binascii
from PIL import Image
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith(('.mac','.macp','.pntg','.mpnt'))]
for version in [0,2]:
 for mode in ['literal','mixed']:
  header=bytearray(512);header[:4]=struct.pack('>I',version)
  if version==2:header[4:308]=bytes((i*17)%256 for i in range(304))
  data=bytearray(header);reference_data=bytearray(header)
  for y in range(720):
   if mode=='literal':
    row=bytes((x*13+y*7)%256 for x in range(72));data.extend([71]);data.extend(row);reference_data.extend([71]);reference_data.extend(row)
   else:
    # Repeat 36 bytes; literal 36 bytes, with legal PackBits no-op controls.
    value=255 if y%2==0 else 0;row=bytes((x*11+y*19)%256 for x in range(36))
    data.extend([128,221,value,128,35]);data.extend(row)
    # Netpbm treats -128 as repeat-129 incorrectly. Decode the equivalent
    # stream without no-ops; inserted no-ops must preserve those pixels.
    reference_data.extend([221,value,35]);reference_data.extend(row)
  name=f'paint-v{version}-{mode}.mac'
  (root/name).write_bytes(data)
  output=subprocess.run(['macptopbm'],input=reference_data,check=True,capture_output=True).stdout
  image=Image.open(io.BytesIO(output)).convert('RGBA')
  assert image.size==(576,720)
  (root/(name+'.rgba')).write_bytes(image.tobytes())
  lines.append(f'{name}\t576\t720\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
# Literal pixels avoid Netpbm's no-op bug and exercise its wrapper detection.
source=(root/'paint-v2-literal.mac').read_bytes()
for version in [0,129,130]:
 for resource in [b'',b'CC0 resource fork']:
  header=bytearray(128);header[1]=5;header[2:7]=b'Paint'
  header[65:69]=b'PNTG';header[69:73]=b'MPNT'
  header[83:87]=struct.pack('>I',len(source));header[87:91]=struct.pack('>I',len(resource))
  if version:
   header[122]=version;header[123]=129
   if version==130:header[102:106]=b'mBIN'
   header[124:126]=struct.pack('>H',binascii.crc_hqx(header[:124],0))
  data=header+source+bytes(-len(source)%128)+resource+bytes(-len(resource)%128)
  name=f'wrapped-{version}-{int(bool(resource))}.mac'
  (root/name).write_bytes(data)
  output=subprocess.run(['macptopbm'],input=data,check=True,capture_output=True).stdout
  image=Image.open(io.BytesIO(output)).convert('RGBA')
  assert image.tobytes()==(root/'paint-v2-literal.mac.rgba').read_bytes()
  (root/(name+'.rgba')).write_bytes(image.tobytes())
  lines.append(f'{name}\t576\t720\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
