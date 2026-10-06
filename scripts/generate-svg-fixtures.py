"""CC0 SVG fixture, independent CairoSVG PNG/Pillow RGBA oracle."""
from pathlib import Path
import hashlib,io
import cairosvg
from PIL import Image
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
data=b'''<svg xmlns="http://www.w3.org/2000/svg" width="3" height="2" viewBox="0 0 3 2">
<rect width="1" height="1" fill="#ff0000"/>
<rect x="1" width="1" height="1" fill="#00ff00" opacity="0.5"/>
<g transform="translate(2 1)"><path d="M0 0h1v1h-1z" fill="blue"/></g>
</svg>'''
name='shapes.svg';(root/name).write_bytes(data)
oracle=Image.open(io.BytesIO(cairosvg.svg2png(bytestring=data))).convert('RGBA').tobytes()
(root/(name+'.rgba')).write_bytes(oracle)
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.startswith(name+'\t')]
lines.append(f'{name}\t3\t2\t1\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
