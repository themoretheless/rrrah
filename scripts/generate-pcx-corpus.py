"""Add existing CC0 Pillow PCX fixtures to the common router corpus."""
from pathlib import Path
import hashlib,subprocess
from PIL import Image
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith('.pcx')]
for source in sorted((root.parent/'pcx').glob('*.pcx')):
 name='pcx-'+source.name;data=source.read_bytes();(root/name).write_bytes(data)
 with Image.open(source) as im: oracle=im.convert('RGBA').tobytes();w,h=im.size
 if source.name=='rgb.pcx':
  oracle=subprocess.check_output(['ffmpeg','-v','error','-i',str(source),'-f','rawvideo','-pix_fmt','rgba','pipe:1'])
 expected=bytes([11,22,33,255,192,193,255,255,1,2,3,255,44,55,66,255,77,88,99,255,0,255,127,255])
 assert oracle==expected
 (root/(name+'.rgba')).write_bytes(oracle)
 lines.append(f'{name}\t{w}\t{h}\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
