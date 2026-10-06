"""CC0 CUR fixtures adapted from independent Pillow PNG/DIB ICO fixtures."""
from pathlib import Path
import hashlib
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.split('\t')[0].endswith('.cur')]
for source in ['pattern.ico','pattern.bitmap.ico']:
 data=bytearray((root/source).read_bytes());data[2]=2;data[10:14]=bytes([3,0,5,0]);name=source+'.cur'
 (root/name).write_bytes(data);(root/(name+'.rgba')).write_bytes((root/(source+'.rgba')).read_bytes())
 lines.append(f'{name}\t16\t16\t0\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
