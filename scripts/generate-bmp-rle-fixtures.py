#!/usr/bin/env python3
"""CC0 authored RLE commands, analytic palettes and independent FFmpeg samples."""
from pathlib import Path
import hashlib
import json
import struct
import subprocess

root = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'
palette = [(0, 0, 0), (255, 0, 0), (0, 255, 0), (0, 0, 255),
           (180, 77, 23), (31, 205, 101), (199, 11, 153), (91, 127, 241)]
cases = []
def write(bits, name, width, rows, commands):
    payload = bytes(commands)
    out = bytearray(54)
    out[:2] = b'BM'
    struct.pack_into('<I', out, 10, 86)
    struct.pack_into('<IiiHHII', out, 14, 40, width, len(rows), 1, bits, 1 if bits == 8 else 2, len(payload))
    struct.pack_into('<I', out, 46, len(palette))
    out.extend(bytes(v for r, g, b in palette for v in (b, g, r, 0)))
    out.extend(payload)
    struct.pack_into('<I', out, 2, len(out))
    filename = f'bmp-rle{bits}-{name}.bmp'
    (root / filename).write_bytes(out)
    expected = bytes(v for row in reversed(rows) for i in row for v in (*palette[i], 255))
    result = subprocess.run(['ffmpeg', '-v', 'error', '-i', str(root / filename), '-f', 'rawvideo', '-pix_fmt', 'rgba', '-'], check=True, capture_output=True)
    assert result.stdout == expected, filename
    cases.append(dict(path=filename, bits=bits, dimensions=[width, len(rows)],
                      sha256=hashlib.sha256(out).hexdigest(), rgba8_hex=expected.hex(),
                      ffmpeg_rgba_sha256=hashlib.sha256(result.stdout).hexdigest()))
for bits in (4, 8):
    absolute = [0, 3, 4, 5, 6, 0] if bits == 8 else [0, 3, 0x45, 0x60]
    top = [7, 6, 5, 4, 3, 2, 1, 0, 7]
    packed = top + [0] if bits == 8 else [0x76, 0x54, 0x32, 0x10, 0x70, 0]
    stream = [9, 1 if bits == 8 else 0x11, 0, 0, 0, 2, 2, 1,
              3, 2 if bits == 8 else 0x23, *absolute,
              1, 7 if bits == 8 else 0x7f, 0, 0, 0, 9, *packed, 0, 1]
    rows = [[1]*9, [0]*9, [0, 0, 2, 2 if bits == 8 else 3, 2, 4, 5, 6, 7], top]
    write(bits, 'mixed', 9, rows, stream)
    for count in range(3, 10):
        indices = [(i * 3 + 1) % 8 for i in range(count)]
        if bits == 8: data = indices[:]
        else: data = [(indices[i] << 4) | (indices[i+1] if i+1 < count else 15) for i in range(0, count, 2)]
        if len(data) % 2: data += [0]
        write(bits, f'absolute-{count}', count, [indices], [0, count, *data, 0, 1])
    write(bits, 'early-end', 4, [[1, 1, 0, 0], [0]*4], [2, 1 if bits == 8 else 0x11, 0, 1])
    tail = [0, 3, 3, 4, 5, 0] if bits == 8 else [0, 3, 0x34, 0x50]
    delta_body = [2, 1 if bits == 8 else 0x11, 0, 2, 1, 2,
                  2, 2 if bits == 8 else 0x22, 0, 2, 1, 0, 0, 0, *tail, 0, 1]
    delta_rows = [[1, 1, 0, 0, 0, 0], [0]*6, [0, 0, 0, 2, 2, 0], [3, 4, 5, 0, 0, 0]]
    write(bits, 'delta-gaps', 6, delta_rows, delta_body)
manifest = dict(license='CC0', scope='authored encoded/absolute/delta/EOL/EOB, analytic opaque-black skipped positions, all samples checked independently by FFmpeg',
                ffmpeg=subprocess.check_output(['ffmpeg', '-version'], text=True).splitlines()[0], cases=cases)
(root / 'bmp-rle-reference.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(f'{len(cases)} independent FFmpeg agreements')
