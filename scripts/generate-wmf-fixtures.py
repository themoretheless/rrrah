#!/usr/bin/env python3
"""CC0 authored placeable WMF fixtures; mathematical solid-fill reference."""
import hashlib
import json
import pathlib
import struct

root = pathlib.Path(__file__).resolve().parents[1] / 'tests/fixtures/wmf'
root.mkdir(parents=True, exist_ok=True)
def record(kind, values=()):
    return struct.pack('<IH', 3 + len(values), kind) + struct.pack('<' + 'H' * len(values), *values)

manifest = []
for name, rgb in [('red', (255, 0, 0)), ('green', (0, 255, 0)), ('blue', (0, 0, 255)), ('polygon', (255, 0, 0)), ('saved-context', (255, 0, 0)), ('object-reuse', (255, 0, 0))]:
    placeable = struct.pack('<IHhhhhHI', 0x9ac6cdd7, 0, 0, 0, 10, 10, 96, 0)
    checksum = 0
    for word in struct.unpack('<10H', placeable):
        checksum ^= word
    placeable += struct.pack('<H', checksum)
    records = b''.join([
        record(0x0103, [8]), record(0x020b, [0, 0]), record(0x020c, [10, 10]),
        record(0x02fc, [0, rgb[0] | (rgb[1] << 8), rgb[2], 0]),
        record(0x02fa, [5, 0, 0, 0, 0]), record(0x012d, [0]), record(0x012d, [1]),
        (record(0x0324, [4, 0, 0, 10, 0, 10, 10, 0, 10]) if name == 'polygon' else record(0x041b, [10, 10, 0, 0])), record(0),
    ])
    if name == 'saved-context':
        # Save red brush/null pen/window, switch brush and move origin, restore.
        insert = b''.join([record(0x001e), record(0x02fc, [0, 0xff00, 0, 0]), record(0x012d, [2]), record(0x020b, [5, 5]), record(0x0127, [0xffff])])
        records = records[:-20] + insert + records[-20:]
    if name == 'object-reuse':
        insert = b''.join([record(0x02fc, [0, 0xff00, 0, 0]), record(0x012d, [2]), record(0x01f0, [0]), record(0x02fc, [0, 255, 0, 0]), record(0x012d, [0])])
        records = records[:-20] + insert + records[-20:]
    header = struct.pack('<HHHIHIH', 1, 9, 0x300, (18 + len(records)) // 2, 3 if name in ['saved-context', 'object-reuse'] else 2, 12 if name == 'polygon' else 8, 0)
    data = placeable + header + records
    pixels = bytes((*rgb, 255)) * 100
    filename = name + '.wmf'
    (root / filename).write_bytes(data)
    (root / (filename + '.rgba')).write_bytes(pixels)
    manifest.append(dict(file=filename, width=10, height=10, sha256=hashlib.sha256(data).hexdigest(), reference='mathematical opaque rectangle or polygon, explicitly created null pen; external evidence is recorded separately'))
(root / 'manifest.json').write_text(json.dumps(dict(license='CC0-1.0', images=manifest), indent=2) + '\n')
