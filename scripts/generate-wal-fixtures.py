"""CC0 indexed WAL fixtures with explicit authored RGBA palette.
Pillow's WAL parser supplies the independent pixel oracle with palette override.
No copyrighted/default game palette is used or inferred.
"""
from pathlib import Path
import struct,hashlib
from PIL import WalImageFile, Image
root=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
palette=bytes(v for i in range(256) for v in [(i*3)%256,(i*5)%256,(i*7)%256,0 if i==255 else 255])
(root/'wal-palette.rgba').write_bytes(palette)
lines=[]
for name,w,h in [('palette-grid.wal',16,16),('rectangular.wal',32,16)]:
 header=bytearray(100);header[:4]=b'CC0\0';struct.pack_into('<II',header,32,w,h)
 data=bytearray(header)
 for level in range(4):
  struct.pack_into('<I',data,40+level*4,len(data))
  lw,lh=max(1,w>>level),max(1,h>>level)
  data+=bytes((x+y*lw+level*17)%256 for y in range(lh) for x in range(lw))
 (root/name).write_bytes(data)
 image=WalImageFile.open(root/name);image.load()
 image.putpalette(bytes(v for i in range(256) for v in palette[i*4:i*4+3]))
 image.info['transparency']=255
 oracle=image.convert('RGBA').tobytes();(root/(name+'.rgba')).write_bytes(oracle)
 lines.append(f'{name}\t{w}\t{h}\t{name}.rgba\t{hashlib.sha256(data).hexdigest()}')
(root/'wal-manifest.tsv').write_text('\n'.join(lines)+'\n')

colormap=Image.new('P',(16,16))
colormap.putdata(range(256))
colormap.putpalette(bytes(v for i in range(256) for v in palette[i*4:i*4+3]))
colormap.save(root/'wal-colormap.pcx')
