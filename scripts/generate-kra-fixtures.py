"""CC0 KRA container fixtures; Python ZIP and Pillow merged PNG oracle."""
from pathlib import Path
import zipfile,hashlib,io
from PIL import Image
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith('.kra')]
for source in ['pattern.png','pattern.profiled.png']:
 name=source+'.kra';path=root/name;png=(root/source).read_bytes()
 with zipfile.ZipFile(path,'w') as archive:
  archive.writestr('mimetype','application/x-krita',compress_type=zipfile.ZIP_STORED)
  archive.writestr('maindoc.xml','<!DOCTYPE DOC PUBLIC "-//KDE//DTD krita 2.0//EN" "http://www.calligra.org/DTD/krita-2.0.dtd"><DOC editor="Krita" syntaxVersion="2.0"><IMAGE mime="application/x-kra" width="16" height="16" colorspacename="RGBA"><layers/></IMAGE></DOC>',compress_type=zipfile.ZIP_DEFLATED)
  archive.writestr('mergedimage.png',png,compress_type=zipfile.ZIP_DEFLATED)
  archive.writestr('preview.png',png,compress_type=zipfile.ZIP_DEFLATED)
 with zipfile.ZipFile(path) as archive:
  oracle=Image.open(io.BytesIO(archive.read('mergedimage.png'))).convert('RGBA').tobytes()
 (root/(name+'.rgba')).write_bytes(oracle)
 lines.append(f'{name}\t16\t16\t0\t{name}.rgba\t{hashlib.sha256(path.read_bytes()).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
