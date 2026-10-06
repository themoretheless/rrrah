"""CC0 monochrome X11 fixture encoded/decoded by Pillow."""
from pathlib import Path
from PIL import Image
import hashlib,subprocess
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
im=Image.new('1',(9,3));im.putdata([255 if (x+y)%3 else 0 for y in range(3) for x in range(9)])
path=root/'pattern.xbm';im.save(path,format='XBM')
oracle=subprocess.check_output(['ffmpeg','-v','error','-i',str(path),'-f','rawvideo','-pix_fmt','rgba','pipe:1'])
(root/'pattern.xbm.rgba').write_bytes(oracle)
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.startswith('pattern.xbm\t')]
lines.append(f'pattern.xbm\t9\t3\t0\tpattern.xbm.rgba\t{hashlib.sha256(path.read_bytes()).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
