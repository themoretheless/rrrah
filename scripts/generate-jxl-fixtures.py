"""CC0 JPEG XL fixtures; libjxl cjxl/djxl and Pillow provide the oracle."""
from pathlib import Path
import hashlib, subprocess, tempfile
from PIL import Image
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith('.jxl')]
with tempfile.TemporaryDirectory() as directory:
 d=Path(directory)
 for container in [0,1]:
  name=f'alpha-container-{container}.jxl'
  im=Image.new('RGBA',(3,2));im.putdata([(255,0,0,255),(0,255,0,128),(0,0,255,0),(17,29,43,64),(61,73,89,127),(250,240,230,255)])
  im.save(d/'input.png')
  subprocess.run(['cjxl',str(d/'input.png'),str(root/name),'-d','0','-e','1',f'--container={container}'],check=True)
  subprocess.run(['djxl',str(root/name),str(d/'output.png')],check=True)
  (root/(name+'.rgba')).write_bytes(Image.open(d/'output.png').convert('RGBA').tobytes())
  lines.append(f'{name}\t3\t2\t0\t{name}.rgba\t{hashlib.sha256((root/name).read_bytes()).hexdigest()}')
with tempfile.TemporaryDirectory() as directory:
 p=Path(directory)/'input.pgm';p.write_bytes(b'P5\n2 1\n65535\n'+bytes([128,0,128,1]))
 subprocess.run(['cjxl',str(p),str(root/'precision16.jxl'),'-d','0','-e','1'],check=True)
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
