"""Synthetic CC0 JPEG 2000; OpenJPEG encoder and FFmpeg pixel oracle."""
from pathlib import Path
import tempfile,subprocess,hashlib
from PIL import Image, ImageCms
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith(('.jp2','.j2k'))]
with tempfile.TemporaryDirectory() as directory:
 d=Path(directory)
 for mode in ['RGB','RGBA','L']:
  im=Image.new(mode,(3,2))
  values=[(255,0,0,255),(0,255,0,128),(0,0,255,0),(17,29,43,64),(61,73,89,127),(250,240,230,255)]
  im.putdata([v if mode=='RGBA' else v[:3] if mode=='RGB' else v[0] for v in values]);im.save(d/'source.png')
  for ext in (['jp2','j2k'] if mode!='RGBA' else ['jp2']):
   name=f'pattern-{mode.lower()}.{ext}'
   subprocess.run(['opj_compress','-i',str(d/'source.png'),'-o',str(root/name),'-n','1'],check=True,stdout=subprocess.DEVNULL)
   subprocess.run(['ffmpeg','-v','error','-i',str(root/name),'-frames:v','1','-f','rawvideo','-pix_fmt','rgba','-y',str(root/(name+'.rgba'))],check=True)
   lines.append(f'{name}\t3\t2\t0\t{name}.rgba\t{hashlib.sha256((root/name).read_bytes()).hexdigest()}')
 p=d/'precision.pgm';p.write_bytes(b'P5\n2 1\n65535\n'+bytes([128,0,128,1]))
 subprocess.run(['opj_compress','-i',str(p),'-o',str(root/'precision16.jp2'),'-n','1'],check=True,stdout=subprocess.DEVNULL)
def box(kind,payload): return (8+len(payload)).to_bytes(4,'big')+kind+payload
def replace_color(data):
 out=b'';at=0
 while at<len(data):
  size=int.from_bytes(data[at:at+4],'big');kind=data[at+4:at+8];payload=data[at+8:at+size]
  if kind==b'jp2h': payload=replace_color(payload)
  if kind==b'colr': payload=bytes([2,0,0])+ImageCms.ImageCmsProfile(ImageCms.createProfile('sRGB')).tobytes()
  out+=box(kind,payload);at+=size
 return out
name='pattern-rgba-icc.jp2';data=replace_color((root/'pattern-rgba.jp2').read_bytes());(root/name).write_bytes(data)
subprocess.run(['ffmpeg','-v','error','-i',str(root/name),'-frames:v','1','-f','rawvideo','-pix_fmt','rgba','-y',str(root/(name+'.rgba'))],check=True)
lines.append(f'{name}\t3\t2\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
