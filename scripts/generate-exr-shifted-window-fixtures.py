#!/usr/bin/env python3
"""Reposition pinned uncompressed EXR data without changing any sample bytes."""
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / 'tests/fixtures/raster'
SOURCE = DIRECTORY / 'float-none-rec709-declared.exr'
base = SOURCE.read_bytes()
assert base[:8] == bytes.fromhex('762f310102000000')
at = 8
attributes = {}
while base[at]:
    end = base.index(0, at)
    name = base[at:end].decode('ascii')
    at = end + 1
    end = base.index(0, at)
    kind = base[at:end]
    at = end + 1
    size, = struct.unpack_from('<I', base, at)
    at += 4
    assert name not in attributes
    attributes[name] = (kind, at, size)
    at += size
at += 1
assert attributes['compression'][0] == b'compression'
assert base[attributes['compression'][1]] == 0
for name in ['displayWindow', 'dataWindow']:
    kind, offset, size = attributes[name]
    assert kind == b'box2i' and size == 16
    assert struct.unpack_from('<4i', base, offset) == (0, 0, 2, 1)
chunks = struct.unpack_from('<2Q', base, at)
for y, offset in enumerate(chunks):
    assert struct.unpack_from('<2i', base, offset) == (y, 48)
    assert offset + 56 <= len(base)

cases = []
for label, origin, display in [
    ('negative', (-2, -1), (-3, -2, 2, 2)),
    ('positive-crop', (2, 1), (3, 1, 5, 3)),
    ('translated', (-7, 9), (-7, 9, -5, 10)),
]:
    data = bytearray(base)
    x, y = origin
    struct.pack_into('<4i', data, attributes['dataWindow'][1], x, y, x + 2, y + 1)
    struct.pack_into('<4i', data, attributes['displayWindow'][1], *display)
    for row, offset in enumerate(chunks):
        struct.pack_into('<i', data, offset, y + row)
        assert data[offset + 4:offset + 56] == base[offset + 4:offset + 56]
    name = f'float-shifted-window-{label}.exr'
    path = DIRECTORY / name
    if path.exists():
        assert path.read_bytes() == data
    path.write_bytes(data)
    cases.append({'fixture': name, 'data_origin': origin, 'display_window': display,
                  'sha256': hashlib.sha256(data).hexdigest(), 'sample_payload_unchanged': True})
report = {'source': SOURCE.name, 'source_sha256': hashlib.sha256(base).hexdigest(),
          'producer': 'metadata and scanline coordinate patch; no pixel decode/re-encode', 'cases': cases}
(DIRECTORY / 'exr-shifted-window-fixtures.json').write_text(json.dumps(report, indent=2) + '\n')
print('Three shifted EXR fixtures reproduced exactly; all scanline sample bytes unchanged')
