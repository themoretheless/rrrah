"""CC0 embedded PNG SVG; independent CairoSVG/Pillow oracle."""
from pathlib import Path
import base64,io,hashlib
import cairosvg
from PIL import Image
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
png=io.BytesIO();Image.new('RGBA',(1,1),(255,0,0,128)).save(png,format='PNG')
data=f'<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><image href="data:image/png;base64,{base64.b64encode(png.getvalue()).decode()}" width="1" height="1"/><rect x="1" width="1" height="1" fill="blue"/></svg>'.encode()
name='embedded.svg';(root/name).write_bytes(data)
oracle=Image.open(io.BytesIO(cairosvg.svg2png(bytestring=data))).convert('RGBA').tobytes();(root/(name+'.rgba')).write_bytes(oracle)
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.startswith(name+'\t')]
lines.append(f'{name}\t2\t1\t1\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}');(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
