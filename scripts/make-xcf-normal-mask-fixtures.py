#!/usr/bin/env python3
"""Generate CC0 legacy normal XCF fixtures with explicit visibility and mask state."""
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1] / 'tests/fixtures/xcf'


def word(value):
    return struct.pack('>I', value)


def build(apply_mask, offset_x, visible=True, opacity=255):
    data = bytearray(b'gimp xcf v001\0')
    data += word(2) + word(1) + word(0) + word(17) + word(1) + b'\0' + bytes(8)
    table = len(data)
    data += bytes(16)

    def hierarchy(width, bpp, pixels):
        at = len(data)
        data.extend(word(width) + word(1) + word(bpp))
        pointer = len(data)
        data.extend(bytes(8))
        data[pointer:pointer + 4] = word(len(data))
        data.extend(word(width) + word(1))
        pointer = len(data)
        tiles = (width + 63) // 64
        data.extend(bytes(4 * (tiles + 1)))
        for index in range(tiles):
            data[pointer + 4 * index:pointer + 4 * index + 4] = word(len(data))
            data.extend(pixels[index * 64 * bpp:(index + 1) * 64 * bpp])
        return at

    for index in range(2):
        width = 128 if index == 0 and (not visible or opacity == 0) else 2
        pixels = bytes([0, 255, 0, 128] * width) if index == 0 else bytes([255, 0, 0, 255, 0, 0, 255, 255])
        data[table + index * 4:table + index * 4 + 4] = word(len(data))
        name = b'top\0' if index == 0 else b'bottom\0'
        data += word(width) + word(1) + word(1) + word(len(name)) + name
        data += word(7) + word(4) + word(0) + word(8) + word(4) + word(int(visible if index == 0 else True))
        if index == 0:
            data += word(11) + word(4) + word(int(apply_mask))
            if opacity != 255:
                data += word(6) + word(4) + word(opacity)
            data += word(15) + word(8) + struct.pack('>ii', offset_x, 0)
        data += bytes(8)
        pointers = len(data)
        data += bytes(8)
        data[pointers:pointers + 4] = word(hierarchy(width, 4, pixels))
        if index == 0:
            data[pointers + 4:pointers + 8] = word(len(data))
            name = b'mask\0'
            data += word(width) + word(1) + word(len(name)) + name + bytes(8)
            pointer = len(data)
            data += bytes(4)
            data[pointer:pointer + 4] = word(hierarchy(width, 1, bytes([0, 255] * (width // 2))))
    return data


for name, enabled, offset, visible, expected in [
    ('normal-mask-enabled-v1', True, 0, True, [255, 0, 0, 255, 0, 128, 127, 255]),
    ('normal-mask-disabled-v1', False, 0, True, [127, 128, 0, 255, 0, 128, 127, 255]),
    ('normal-mask-negative-offset-v1', True, -1, True, [127, 128, 0, 255, 0, 0, 255, 255]),
    ('normal-mask-half-opacity-v1', True, 0, True, [255, 0, 0, 255, 0, 64, 191, 255]),
    ('normal-mask-zero-opacity-v1', True, 0, True, [255, 0, 0, 255, 0, 0, 255, 255]),
    ('normal-mask-hidden-v1', True, 0, False, [255, 0, 0, 255, 0, 0, 255, 255]),
]:
    opacity = 0 if name == 'normal-mask-zero-opacity-v1' else 128 if name == 'normal-mask-half-opacity-v1' else 255
    data = build(enabled, offset, visible, opacity)
    (ROOT / f'{name}.xcf').write_bytes(data)
    manifest = {'license': 'CC0', 'sha256': hashlib.sha256(data).hexdigest(),
                'construction': 'Authored v1 2x1 raw RGBA normal layers, green alpha128 over opaque red/blue, raw 0/255 mask.',
                'apply_mask': enabled, 'visible': visible, 'offset_x': offset, 'expected_rgba_hex': bytes(expected).hex(),
                'scope': 'Binary mask endpoints only; no intermediate mask rounding or independent GIMP-rendered oracle.'}
    if opacity != 255:
        manifest['opacity'] = opacity
    (ROOT / f'{name}-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
