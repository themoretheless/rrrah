"""CC0 KTX1 wrappers and crop/orientation oracles from independent Android ETC1 blocks.
Run generate-etc1-block-oracles.py first; no production decoder used here.
"""
from pathlib import Path
import hashlib
import struct
ROOT = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
blocks = (ROOT / 'etc1-block-oracles.bin').read_bytes()
rgb = (ROOT / 'etc1-block-oracles.rgb').read_bytes()
assert hashlib.sha256(blocks).hexdigest() == '0cf34deb2ad130d8afaafaa0a7a9195e2a2242c0db2af725e4d04b7fe7e85cd1'
assert hashlib.sha256(rgb).hexdigest() == 'c09a0e8677061b24a1dde95832c85dab1bae87d275eb632980fd25349b4407dd'
manifest = ['# source\tmip\twidth\theight\tkind\tcolor\toracle\tsha256']
for endian, label in [('<', 'le'), ('>', 'be')]:
    for horizontal in ['r', 'l']:
        for vertical in ['d', 'u']:
            name = f'ktx-etc1-{label}-{horizontal}{vertical}.ktx'
            kv = b'KTXorientation\0' + f'S={horizontal},T={vertical}'.encode() + b'\0'
            metadata = struct.pack(endian+'I', len(kv)) + kv
            metadata += bytes((-len(kv)) % 4)
            source = b'\xabKTX 11\xbb\r\n\x1a\n' + struct.pack(endian+'13I',
                         0x04030201, 0, 1, 0, 0x8d64, 0x1907, 9, 5, 0, 0, 1, 4, len(metadata)) + metadata
            start = 0
            for level in range(4):
                w, h = max(1, 9 >> level), max(1, 5 >> level)
                cols, rows = (w+3)//4, (h+3)//4
                n = cols * rows
                payload = blocks[start*8:(start+n)*8]
                source += struct.pack(endian+'I', len(payload)) + payload
                output = bytearray()
                for y in range(h):
                    for x in range(w):
                        sx = w-1-x if horizontal == 'l' else x
                        sy = h-1-y if vertical == 'u' else y
                        block = start + (sy//4)*cols + sx//4
                        pixel = (sy%4)*4 + sx%4
                        offset = block*48 + pixel*3
                        output.extend(rgb[offset:offset+3] + b'\xff')
                oracle = f'{name}.mip{level}.rgba8'
                (ROOT / oracle).write_bytes(output)
                manifest.append(f'{name}\t{level}\t{w}\t{h}\trgba8\tlinear\t{oracle}\tHASH')
                start += n
            (ROOT / name).write_bytes(source)
            digest = hashlib.sha256(source).hexdigest()
            manifest = [line.replace('\tHASH', '\t'+digest) for line in manifest]
(ROOT / 'ktx-etc1-manifest.tsv').write_text('\n'.join(manifest)+'\n')
print('8 KTX1 ETC1 sources, 32 selected mip oracles')
