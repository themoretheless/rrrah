"""CC0 PVR v3 BC1/2/3 wrappers; independently generated Pillow block RGBA oracles."""
from pathlib import Path
import hashlib
import struct
ROOT = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
manifest = ['# source\tmip\twidth\theight\tkind\tcolor\toracle\tover\tsha256']
for variant, fmt in [(1, 7), (2, 9), (3, 11)]:
    blocks = (ROOT / f'bc{variant}-block-oracles.bin').read_bytes()
    rgba = (ROOT / f'bc{variant}-block-oracles.rgba').read_bytes()
    block_size = 8 if variant == 1 else 16
    for color in [0, 1]:
        for fx in [0, 1]:
            for fy in [0, 1]:
                name = f'pvr-bc{variant}-{color}-{fx}{fy}.pvr'
                custom = b'CC0 custom metadata'
                metadata = b'PVR\x03' + struct.pack('<II', 3, 3) + bytes([fx, fy, 0])
                metadata += b'CC0!' + struct.pack('<II', 42, len(custom)) + custom
                source = b'PVR\x03' + struct.pack('<IQ9I', 0, fmt, color, 0, 5, 9, 1, 1, 1, 4, len(metadata)) + metadata
                start = 0
                records = []
                for level in range(4):
                    w, h = max(1, 9 >> level), max(1, 5 >> level)
                    cols, rows = (w+3)//4, (h+3)//4
                    count = cols * rows
                    source += blocks[start*block_size:(start+count)*block_size]
                    output = bytearray()
                    for y in range(h):
                        for x in range(w):
                            sx, sy = (w-1-x if fx else x), (h-1-y if fy else y)
                            offset = ((start+(sy//4)*cols+sx//4)*16 + (sy%4)*4 + sx%4)*4
                            output.extend(rgba[offset:offset+4])
                    oracle = f'{name}.mip{level}.rgba8'
                    (ROOT / oracle).write_bytes(output)
                    records.append(f'{name}\t{level}\t{w}\t{h}\trgba8\t{color}\t{oracle}\t-')
                    start += count
                (ROOT / name).write_bytes(source)
                digest = hashlib.sha256(source).hexdigest()
                manifest.extend(record+'\t'+digest for record in records)
(ROOT / 'pvr-bc-manifest.tsv').write_text('\n'.join(manifest)+'\n')
print('24 PVR v3 BC sources;96 selected mip oracles')
