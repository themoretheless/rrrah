"""CC0 native sample contracts for ordinary KTX2 mip selection, not libktx oracles."""
from pathlib import Path
import importlib.util
import struct
import hashlib

ROOT = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
spec = importlib.util.spec_from_file_location('ktx2_builder', Path(__file__).with_name('generate-ktx2-fixtures.py'))
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)
DIMS = [(9, 5), (4, 2), (2, 1), (1, 1)]
lines = ['# source\tmip\twidth\theight\tkind\tcolor\toracle\tsha256']
for vk, channels, size, code, kind in [(29,3,1,'B','rgba8'),(50,4,1,'B','rgba8'),
                                      (84,3,2,'H','rgba16'),(91,4,2,'H','rgba16'),
                                      (106,3,4,'f','rgba32f'),(109,4,4,'f','rgba32f')]:
    for compression in [0, 3]:
        for orientation in ['rd', 'lu']:
            payloads, oracles = [], []
            for level, (w,h) in enumerate(DIMS):
                pixels = []
                packed = []
                for y in range(h):
                    for x in range(w):
                        if size == 1:
                            p = [(x*17+level*39)%256, y*31+13, (x+y*7+level*23)%256, (x*27+y*19)%256]
                            opaque = 255
                        elif size == 2:
                            p = [32768+x+level*9, y*257+1, 65535-x*17, x*1024+y*257]
                            opaque = 65535
                        else:
                            p = [x*.125-.5, y*.25+level, 2.+x/16., (x+y)/16.]
                            opaque = 1.
                        pixels.append(p if channels == 4 else p[:3]+[opaque])
                        packed.extend([p[2],p[1],p[0],p[3]][:channels] if vk == 50 else p[:channels])
                expected = []
                for y in range(h):
                    for x in range(w):
                        sx,sy = (w-1-x,h-1-y) if orientation == 'lu' else (x,y)
                        expected.extend(pixels[sy*w+sx])
                payloads.append(struct.pack('<'+code*len(packed),*packed))
                oracles.append(struct.pack('<'+code*len(expected),*expected))
            data = builder.make(vk,channels,size,*DIMS[0],payloads[0],orientation,compression,payloads[1:])
            name = f'ktx2-mips-{vk}-{compression}-{orientation}.ktx2'
            (ROOT/name).write_bytes(data)
            digest = hashlib.sha256(data).hexdigest()
            for level,((w,h),oracle) in enumerate(zip(DIMS,oracles)):
                target = f'{name}.mip{level}.{kind}'
                (ROOT/target).write_bytes(oracle)
                lines.append(f'{name}\t{level}\t{w}\t{h}\t{kind}\t{"srgb" if vk in (29,50) else "linear"}\t{target}\t{digest}')
(ROOT/'ktx2-mip-manifest.tsv').write_text('\n'.join(lines)+'\n')
