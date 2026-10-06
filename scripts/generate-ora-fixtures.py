"""CC0 OpenRaster archives from existing PNG sources, Python zip/Pillow oracle."""
from pathlib import Path
import zipfile,hashlib,io
from PIL import Image
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith('.ora')]
for source in ['pattern.png','pattern.profiled.png']:
 name=source+'.ora';path=root/name;png=(root/source).read_bytes()
 with zipfile.ZipFile(path,'w') as archive:
  archive.writestr('mimetype','image/openraster',compress_type=zipfile.ZIP_STORED)
  archive.writestr('stack.xml','<image version="0.0.6" w="16" h="16"><stack><layer name="test" src="data/layer.png" opacity="1.0" visibility="visible" composite-op="svg:src-over"/></stack></image>',compress_type=zipfile.ZIP_DEFLATED)
  archive.writestr('mergedimage.png',png,compress_type=zipfile.ZIP_DEFLATED)
  archive.writestr('data/layer.png',png,compress_type=zipfile.ZIP_DEFLATED)
  archive.writestr('Thumbnails/thumbnail.png',png,compress_type=zipfile.ZIP_DEFLATED)
 with zipfile.ZipFile(path) as archive:
  with Image.open(io.BytesIO(archive.read('mergedimage.png'))) as im:oracle=im.convert('RGBA').tobytes()
 (root/(name+'.rgba')).write_bytes(oracle)
 lines.append(f'{name}\t16\t16\t0\t{name}.rgba\t{hashlib.sha256(path.read_bytes()).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
