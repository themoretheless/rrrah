"""Authored CC0 KTX1 mip chains; native sample and independent DDS block oracles."""
from pathlib import Path
import struct
import hashlib

ROOT = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
DIMS = [(9, 5), (4, 2), (2, 1), (1, 1)]
MAGIC = b'\xabKTX 11\xbb\r\n\x1a\n'
lines = ['# source\tmip\twidth\theight\tkind\tcolor\toracle\tsha256']


def record(name, ty, size, channels, internal, payloads, oracles, kind, be, flips):
    order = '>' if be else '<'
    orientation = f'S={"l" if flips[0] else "r"},T={"u" if flips[1] else "d"}'
    kv = b'KTXorientation\0' + orientation.encode() + b'\0'
    metadata = struct.pack(order + 'I', len(kv)) + kv + bytes(-len(kv) % 4)
    base = 0x1907 if channels == 3 else 0x1908
    source = MAGIC + struct.pack(order + '13I', 0x04030201, ty, size, base if ty else 0,
                                 internal, base, *DIMS[0], 0, 0, 1, len(DIMS), len(metadata)) + metadata
    for payload in payloads:
        source += struct.pack(order + 'I', len(payload)) + payload + bytes(-len(payload) % 4)
    (ROOT / name).write_bytes(source)
    digest = hashlib.sha256(source).hexdigest()
    color = 'srgb' if internal == 0x8c43 else 'linear'
    for level, ((w, h), oracle) in enumerate(zip(DIMS, oracles)):
        target = f'{name}.mip{level}.{kind}'
        (ROOT / target).write_bytes(oracle)
        lines.append(f'{name}\t{level}\t{w}\t{h}\t{kind}\t{color}\t{target}\t{digest}')


for ty, size, code, kind, rgb, rgba in [
    (0x1401, 1, 'B', 'rgba8', 0x8051, 0x8c43),
    (0x1403, 2, 'H', 'rgba16', 0x8054, 0x805b),
    (0x1406, 4, 'f', 'rgba32f', 0x8815, 0x8814),
]:
    for channels in [3, 4]:
        for be in [False, True]:
            flips = (be, channels == 3)
            payloads, oracles = [], []
            for level, (w, h) in enumerate(DIMS):
                pixels = []
                payload = b''
                for y in range(h):
                    row = b''
                    for x in range(w):
                        if size == 1:
                            values = [(x * 17 + level * 39) % 256, (y * 31 + 13) % 256,
                                      (x + y * 7 + level * 23) % 256, (x * 27 + y * 19) % 256]
                            opaque = 255
                        elif size == 2:
                            values = [32768 + x + level * 9, y * 257 + 1, 65535 - x * 17, x * 1024 + y * 257]
                            opaque = 65535
                        else:
                            values = [x * .125 - .5, y * .25 + level, 2. + x / 16., (x + y) / 16.]
                            opaque = 1.
                        row += struct.pack(('>' if be else '<') + code * channels, *values[:channels])
                        pixels.append(values if channels == 4 else values[:3] + [opaque])
                    payload += row + bytes(-len(row) % 4)
                expected = []
                for y in range(h):
                    for x in range(w):
                        sx = w - 1 - x if flips[0] else x
                        sy = h - 1 - y if flips[1] else y
                        expected.extend(pixels[sy * w + sx])
                payloads.append(payload)
                oracles.append(struct.pack('<' + code * len(expected), *expected))
            record(f'ktx-mips-{channels}ch-{size * 8}-{int(be)}.ktx', ty, size, channels,
                   rgb if channels == 3 else rgba, payloads, oracles, kind, be, flips)

for variant, internal in [('bc1', 0x83f0), ('bc2', 0x83f2), ('bc3', 0x83f3)]:
    dds_name = f'dds-mips-{variant}.dds'
    dds = (ROOT / dds_name).read_bytes()
    offset = 128
    payloads, oracles = [], []
    for level, (w, h) in enumerate(DIMS):
        length = ((w + 3) // 4) * ((h + 3) // 4) * (8 if variant == 'bc1' else 16)
        payloads.append(dds[offset:offset + length])
        offset += length
        oracles.append((ROOT / f'{dds_name}.mip{level}.rgba').read_bytes())
    assert offset == len(dds)
    record(f'ktx-mips-{variant}.ktx', 0, 1, 3 if variant == 'bc1' else 4, internal,
           payloads, oracles, 'rgba8', variant == 'bc2', (False, False))

(ROOT / 'ktx-mip-manifest.tsv').write_text('\n'.join(lines) + '\n')
