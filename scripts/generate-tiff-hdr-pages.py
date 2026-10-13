#!/usr/bin/env python3
"""Authored CC0 float TIFF pages: signed HDR, straight/associated alpha, Deflate."""
from pathlib import Path
import struct
import zlib
import json
import hashlib

root = Path(__file__).resolve().parents[1] / 'tests/fixtures/data'
files = []
for endian, label in [('<', 'le'), ('>', 'be')]:
    pack = lambda fmt, *values: struct.pack(endian + fmt, *values)
    streams = []
    for page in range(3):
        values = []
        for y in range(2):
            for x in range(3):
                alpha = [0.25, 0.5, 1.0][(y * 3 + x) % 3]
                rgba = [2.0 + page + x / 8, -0.5 - y / 8,
                        -0.0 if (y * 3 + x) % 2 == 0 else 0.25, alpha]
                if page == 1:
                    rgba[:3] = [v * alpha for v in rgba[:3]]
                values.extend(rgba)
        streams.append(zlib.compress(pack('24f', *values)))
    stride = 2 + 13 * 12 + 4
    arrays = 8 + 3 * stride
    offset = arrays + 3 * 16
    offsets = []
    for stream in streams:
        offsets.append(offset)
        offset += len(stream)
    output = bytearray((b'II' if endian == '<' else b'MM') + pack('HI', 42, 8))
    for page in range(3):
        tags = [(256, 4, 1, 3), (257, 4, 1, 2), (258, 3, 4, arrays + page * 16),
                (259, 3, 1, 8), (262, 3, 1, 2), (273, 4, 1, offsets[page]),
                (274, 3, 1, 6 if page == 2 else 1), (277, 3, 1, 4),
                (278, 4, 1, 2), (279, 4, 1, len(streams[page])), (284, 3, 1, 1),
                (338, 3, 1, 1 if page == 1 else 2), (339, 3, 4, arrays + page * 16 + 8)]
        output += pack('H', len(tags))
        for tag, kind, count, value in tags:
            inline = pack('H', value) + b'\0\0' if kind == 3 and count == 1 else pack('I', value)
            output += pack('HHI', tag, kind, count) + inline
        output += pack('I', 8 + (page + 1) * stride if page < 2 else 0)
    for _ in range(3):
        output += pack('4H', 32, 32, 32, 32) + pack('4H', 3, 3, 3, 3)
    output += b''.join(streams)
    path = root / f'tiff-pages-hdr-deflate-{label}.tif'
    path.write_bytes(output)
    files.append({'file': path.name, 'sha256': hashlib.sha256(output).hexdigest()})
(root / 'tiff-pages-hdr-manifest.json').write_text(json.dumps({
    'license': 'CC0-1.0', 'producer': 'Authored Python struct/zlib TIFF writer',
    'pages': 3, 'stored_dimensions': [3, 2], 'bits_per_channel': 32,
    'compression': 'Deflate', 'alpha': ['straight', 'associated', 'straight'],
    'orientation': [1, 1, 6], 'color': 'Untagged, requires explicit linear-sRGB interpretation',
    'components': 'R=2+page+x/8; G=-0.5-y/8; B=negative-zero at even pixel else0.25; A=[0.25,0.5,1][x]',
    'limitations': 'Authored numerical contract; not independent producer/color fidelity',
    'files': files,
}, indent=2) + '\n')
