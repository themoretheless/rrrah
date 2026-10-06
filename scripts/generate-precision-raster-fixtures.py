"""CC0 precision corpus: authored PAM/farbfeld/RGBE and OpenEXR 3.5.2.
Uses independent OpenEXR writing/reading for HALF/FLOAT and four compressions.
Requires Pillow, numpy and OpenEXR. Oracles never use Rrrah.
"""
from pathlib import Path
import struct, hashlib, math
import numpy as np
import OpenEXR
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[]
def record(name,data,w,h,kind,values):
 (root/name).write_bytes(data)
 oracle=name+'.'+kind
 fmt={'u8':'B','u16':'H','f32':'f'}[kind]
 (root/oracle).write_bytes(struct.pack('<'+fmt*len(values),*values))
 lines.append(f'{name}\t{w}\t{h}\t{kind}\t{oracle}\t{hashlib.sha256(data).hexdigest()}')
def pam(name,depth,maximum,tupl,values,expected,kind):
 header=f'P7\n# CC0 Rrrah precision corpus\nWIDTH 2\nHEIGHT 1\nDEPTH {depth}\nMAXVAL {maximum}\nTUPLTYPE {tupl}\nENDHDR\n'.encode()
 payload=bytes(values) if maximum<256 else struct.pack('>'+'H'*len(values),*values)
 record(name,header+payload,2,1,kind,expected)
pam('color-alpha8.pam',4,255,'RGB_ALPHA',[17,29,43,64,61,73,89,128],[17,29,43,64,61,73,89,128],'u8')
pam('color-alpha15.pam',4,15,'RGB_ALPHA',[0,1,2,3,10,11,12,13],[0,17,34,51,170,187,204,221],'u8')
pam('gray-alpha16.pam',2,65535,'GRAYSCALE_ALPHA',[32768,1001,32769,1002],[32768,32768,32768,1001,32769,32769,32769,1002],'u16')
pam('color-alpha16.pam',4,65535,'RGB_ALPHA',[32768,1,65534,32767,32769,2,65535,32768],[32768,1,65534,32767,32769,2,65535,32768],'u16')
pam('blackwhite.pam',1,1,'BLACKANDWHITE',[0,1],[0,0,0,255,255,255,255,255],'u8')
values=[32768,1,65534,32767,32769,2,65535,32768]
record('adjacent16.ff',b'farbfeld'+struct.pack('>II',2,1)+struct.pack('>8H',*values),2,1,'u16',values)
def rgbe_expected(pixels):
 return [value for pixel in pixels for value in
         [*(math.ldexp(v,pixel[3]-136) if pixel[3] else 0 for v in pixel[:3]),1.0]]
pixels=[(128,64,32,130),(129,65,33,130),(64,128,32,131),(0,0,0,0),(20,40,80,127),(255,128,64,132)]
record('raw-hdr.hdr',b'#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 2 +X 3\n'+bytes(v for p in pixels for v in p),3,2,'f32',rgbe_expected(pixels))
pixels=[((x*11+y*7)%128+128,(x*19+y*5)%128+64,(x*13+y*3)%128+32,130+(x%2)) for y in range(2) for x in range(8)]
payload=bytearray()
for y in range(2):
 payload.extend([2,2,0,8])
 for c in range(4):payload.extend([8,*[pixels[y*8+x][c] for x in range(8)]])
record('rle-hdr.hdr',b'#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 2 +X 8\n'+payload,8,2,'f32',rgbe_expected(pixels))
rgba=np.array([[[2.0,-0.25,0.5,1.0],[0.125,4.0,0.75,0.5],[0.0,0.0,0.0,0.0]],[[1.0,0.5,0.25,0.25],[0.5,0.25,0.125,0.75],[8.0,2.0,1.0,1.0]]],dtype=np.float32)
for kind,dtype in [('half',np.float16),('float',np.float32)]:
 for comp,label in [(OpenEXR.NO_COMPRESSION,'none'),(OpenEXR.RLE_COMPRESSION,'rle'),(OpenEXR.ZIP_COMPRESSION,'zip'),(OpenEXR.PIZ_COMPRESSION,'piz')]:
  name=f'{kind}-{label}.exr';stored=rgba.copy();stored[..., :3]*=stored[..., 3:4]
  name=f'{kind}-{label}.exr';channels={'RGBA':stored.astype(dtype)}
  OpenEXR.File({'compression':comp},channels).write(str(root/name))
  oracle=OpenEXR.File(str(root/name)).channels()['RGBA'].pixels.astype(np.float32)
  # Reference 'over' uses stored associated RGB directly, before unassociation.
  background=np.array([0.25,0.5,0.75],dtype=np.float32)
  over=oracle[..., :3]+(np.float32(1)-oracle[..., 3:4])*background
  (root/(name+'.over-rgb-f32')).write_bytes(over.astype('<f4').tobytes())
  np.divide(oracle[..., :3],oracle[..., 3:4],out=oracle[..., :3],where=oracle[..., 3:4]>0)
  assert np.array_equal(oracle,rgba)
  record(name,(root/name).read_bytes(),3,2,'f32',oracle.flatten().tolist())
(root/'precision-manifest.tsv').write_text('\n'.join(lines)+'\n')
# Legal EXR associated-alpha emission has no exact straight-alpha equivalent.
# Keep this fixture outside the successful-pixel manifest until the renderer
# gains associated-alpha semantics; the current API must reject it explicitly.
OpenEXR.File({'compression':OpenEXR.ZIP_COMPRESSION},
             {'RGBA':np.array([[[2.0,0.5,0.25,0.0]]],dtype=np.float32)}).write(str(root/'zero-alpha-emission.exr'))
