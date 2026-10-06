"""CC0 Photoshop composition fixtures, independent FFmpeg pixel oracle."""
from pathlib import Path
from psd_tools import PSDImage
from PIL import Image
import struct,subprocess,hashlib,zlib
from psd_tools.compression import encode_prediction
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith(('.psd','.psb'))]
def make(version,compression,depth=8):
 w,h=3,2
 header=b'8BPS'+struct.pack('>H',version)+bytes(6)+struct.pack('>HIIHH',3,h,w,depth,3)
 header+=bytes(8)+bytes(8 if version==2 else 4)+struct.pack('>H',compression)
 planes=[[255,0,0,17,61,250],[0,255,0,29,73,240],[0,0,255,43,89,230]]
 raw=b''.join(bytes(p) for p in planes)
 if compression==0:return header+raw
 if compression==2:return header+zlib.compress(raw)
 if compression==3:return header+zlib.compress(encode_prediction(raw,w,h*3,8))
 rows=[raw[i:i+3] for i in range(0,len(raw),3)]
 return header+b''.join(struct.pack('>I' if version==2 else '>H',4) for row in rows)+b''.join(bytes([2])+row for row in rows)
for version in [1,2]:
 for compression in [0,1,2,3]:
  name=f'composite-{version}-{compression}.'+('psd' if version==1 else 'psb');data=make(version,compression);(root/name).write_bytes(data)
  if version==2 or compression>=2:
   (root/(name+'.rgba')).write_bytes(PSDImage.open(root/name).topil().convert('RGBA').tobytes())
  else: subprocess.run(['ffmpeg','-v','error','-i',str(root/name),'-frames:v','1','-f','rawvideo','-pix_fmt','rgba','-y',str(root/(name+'.rgba'))],check=True)
  lines.append(f'{name}\t3\t2\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
name='alpha-merged.psd'
p=PSDImage.frompil(Image.new('RGBA',(2,1),(225,240,250,127)));p.save(root/name)
(root/(name+'.rgba')).write_bytes(PSDImage.open(root/name).topil(apply_icc=False).convert('RGBA').tobytes())
lines.append(f'{name}\t2\t1\t0\t{name}.rgba\t{hashlib.sha256((root/name).read_bytes()).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')

for depth,values in [(16,[32768,32769,1,2,65534,65535]),(32,[0.5,2.0,0.1,0.2,0.3,0.4])]:
 raw=b''.join(struct.pack('>H' if depth==16 else '>f',v) for v in values)
 header=b'8BPS'+struct.pack('>H',1)+bytes(6)+struct.pack('>HIIHH',3,1,2,depth,3)+bytes(12)+struct.pack('>H',3)
 (root/f'prediction-{depth}.psd').write_bytes(header+zlib.compress(encode_prediction(raw,2,3,depth)))
