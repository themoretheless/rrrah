#!/usr/bin/env python3
"""Generate CC0 legacy indexed and indexed-alpha XCF files."""
import hashlib
import json
from pathlib import Path
import struct

root = Path(__file__).resolve().parents[1] / 'tests/fixtures/xcf'
def word(value):
    return struct.pack('>I', value)

for name, kind, pixels, expected in [
    ('normal-indexed-v1', 4, bytes([0, 1]), bytes([17, 43, 91, 255, 239, 181, 7, 255])),
    ('normal-indexed-alpha-v1', 5, bytes([0, 128, 1, 255]), bytes([17, 43, 91, 255, 239, 181, 7, 255])),
    ('normal-indexed-alpha-boundary-v1', 5, bytes([0, 127, 1, 128]), bytes([0, 0, 0, 0, 239, 181, 7, 255])),
]:
    data = bytearray(b'gimp xcf v001\0')
    data += word(2) + word(1) + word(2) + word(1) + word(10) + word(2) + bytes([17, 43, 91, 239, 181, 7]) + word(17) + word(1) + b'\0' + bytes(8)
    table = len(data)
    data += bytes(12)
    data[table:table + 4] = word(len(data))
    label = b'indexed\0'
    data += word(2) + word(1) + word(kind) + word(len(label)) + label
    data += word(7) + word(4) + word(0) + word(8) + word(4) + word(1) + bytes(8)
    pointer = len(data)
    data += bytes(8)
    data[pointer:pointer + 4] = word(len(data))
    data += word(2) + word(1) + word(kind - 3)
    pointer = len(data)
    data += bytes(8)
    data[pointer:pointer + 4] = word(len(data))
    data += word(2) + word(1)
    pointer = len(data)
    data += bytes(8)
    data[pointer:pointer + 4] = word(len(data))
    data += pixels
    (root / f'{name}.xcf').write_bytes(data)
    (root / f'{name}-manifest.json').write_text(json.dumps({
        'license': 'CC0', 'sha256': hashlib.sha256(data).hexdigest(),
        'construction': 'Authored v1 2x1 raw indexed normal layer with explicit visibility',
        'expected_rgba_hex': expected.hex(),
    }, indent=2) + '\n')
