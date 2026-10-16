#!/usr/bin/env python3
"""Regenerate native RGB16/ICC references from hash-pinned PSD-tools raw composites."""
import hashlib
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'
SOURCES = {
    'psd': 'c78689ea7b576f23bbd8b6b4f4993a365266c3ffc9aaa02cc20bef4a36a9d7a0',
    'psb': 'e0c9b37ffb5981aaedbeec2b2dfa3ad561b57e86e5e04758ed859ca819cd427c',
}
rows = ['# source\twidth\theight\toracle_rgba16le\tsha256\tprofile']
for extension, digest in SOURCES.items():
    name = f'psd-tools-16bit5x5.{extension}'
    data = (ROOT / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == digest
    version = struct.unpack_from('>H', data, 4)[0]
    channels, height, width, depth, mode = struct.unpack_from('>HIIHH', data, 12)
    assert (channels, height, width, depth, mode) == (3, 5, 5, 16, 3)
    at = 26
    length = struct.unpack_from('>I', data, at)[0]
    at += 4 + length
    length = struct.unpack_from('>I', data, at)[0]
    resources = data[at + 4:at + 4 + length]
    at += 4 + length
    cursor = 0
    profile = None
    while cursor < len(resources):
        assert resources[cursor:cursor + 4] == b'8BIM'
        identifier = struct.unpack_from('>H', resources, cursor + 4)[0]
        cursor += 6
        name_bytes = resources[cursor]
        cursor += (1 + name_bytes + 1) & ~1
        length = struct.unpack_from('>I', resources, cursor)[0]
        cursor += 4
        payload = resources[cursor:cursor + length]
        assert len(payload) == length
        if identifier == 1039:
            assert profile is None
            profile = payload
        cursor += length + length % 2
    length_bytes = 8 if version == 2 else 4
    length = int.from_bytes(data[at:at + length_bytes], 'big')
    at += length_bytes + length
    assert data[at:at + 2] == b'\0\0'
    at += 2
    assert len(data) - at == width * height * channels * 2
    values = struct.unpack(f'>{width * height * channels}H', data[at:])
    rgba = [value for pixel in range(width * height) for value in
            (values[pixel], values[width * height + pixel], values[2 * width * height + pixel], 65535)]
    oracle = name + '.rgba16le'
    (ROOT / oracle).write_bytes(struct.pack(f'<{len(rgba)}H', *rgba))
    profile_name = name + '.icc' if profile else '-'
    if profile:
        (ROOT / profile_name).write_bytes(profile)
    rows.append('\t'.join((name, str(width), str(height), oracle, digest, profile_name)))
    print(name, 'samples=', len(rgba), 'profile_bytes=', len(profile or b''))
(ROOT / 'psd-tools-16bit-manifest.tsv').write_text('\n'.join(rows) + '\n')
