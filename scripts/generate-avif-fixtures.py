"""CC0 AVIF fixtures encoded and independently decoded by Pillow/libavif."""
from pathlib import Path
import hashlib
from PIL import Image
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
source=Image.open(root/'pattern.png');icc=Image.open(root/'pattern.profiled.png').info['icc_profile']
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith('.avif')]
for name,mode,profile in [('avif-rgb.avif','RGB',None),('avif-alpha-icc.avif','RGBA',icc)]:
 path=root/name;options={'quality':100,'subsampling':'4:4:4','speed':6}
 if profile is not None:options['icc_profile']=profile
 source.convert(mode).save(path,format='AVIF',**options)
 with Image.open(path) as im: oracle=im.convert('RGBA').tobytes();w,h=im.size
 (root/(name+'.rgba')).write_bytes(oracle)
 lines.append(f'{name}\t{w}\t{h}\t3\t{name}.rgba\t{hashlib.sha256(path.read_bytes()).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
