"""Synthetic CC0 two-page DCX; Pillow PCX encoding and DCX decoding oracle."""
from pathlib import Path
import io,hashlib
from PIL import Image
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
pages=[]
for values in [[0,1,2,3,4,5],[5,4,3,2,1,0]]:
 im=Image.new('P',(3,2));im.putpalette([v for i in range(256) for v in (i,(i*3)%256,(i*7)%256)]);im.putdata(values)
 out=io.BytesIO();im.save(out,format='PCX');pages.append(out.getvalue())
data=bytearray(4100);data[:4]=bytes([0xb1,0x68,0xde,0x3a]);offset=4100
for i,page in enumerate(pages): data[4+i*4:8+i*4]=offset.to_bytes(4,'little');data.extend(page);offset+=len(page)
name='two-pages.dcx';(root/name).write_bytes(data)
im=Image.open(io.BytesIO(data));assert im.n_frames==2
for index in range(2):
 im.seek(index);(root/(name+f'.page{index}.rgba')).write_bytes(im.convert('RGBA').tobytes())
lines=[l for l in (root/'manifest.tsv').read_text().splitlines() if not l.startswith(name+'\t')]
lines.append(f'{name}\t3\t2\t0\t{name}.page0.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'manifest.tsv').write_text('\n'.join(lines)+'\n')
