"""CC0 synthetic BC1/2/3 blocks with independent Pillow DDS RGBA oracles."""
from pathlib import Path
from PIL import Image, __version__
from io import BytesIO
import random
import struct
import hashlib
ROOT = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
rng = random.Random(0x42434f52)
for variant, fourcc in [(1, b'DXT1'), (2, b'DXT3'), (3, b'DXT5')]:
    blocks, oracle = bytearray(), bytearray()
    for case in range(4096):
        endpoints = [0, 0xffff, 0xf800, 0x7e0, 0x1f, rng.randrange(65536)]
        c0, c1 = endpoints[case % 6], endpoints[(case//6) % 6]
        selectors = [0, 0xffffffff, 0xe4e4e4e4, rng.getrandbits(32)][case % 4]
        block = struct.pack('<HHI', c0, c1, selectors)
        if variant == 2:
            block = rng.getrandbits(64).to_bytes(8, 'little') + block
        if variant == 3:
            alpha = [0, 255, 127, 128, rng.randrange(256)]
            block = bytes([alpha[case % 5], alpha[(case//5) % 5]]) + rng.getrandbits(48).to_bytes(6, 'little') + block
        header = [124, 0x81007, 4, 4, len(block), 0, 1] + [0]*11
        header += [32, 4, int.from_bytes(fourcc, 'little'), 0, 0, 0, 0, 0, 0x1000, 0, 0, 0, 0]
        source = b'DDS ' + struct.pack('<31I', *header) + block
        with Image.open(BytesIO(source)) as image:
            oracle.extend(image.convert('RGBA').tobytes())
        blocks.extend(block)
    for suffix, content in [('bin', blocks), ('rgba', oracle)]:
        name = f'bc{variant}-block-oracles.{suffix}'
        (ROOT / name).write_bytes(content)
        print(name, hashlib.sha256(content).hexdigest())
print('Pillow', __version__)
