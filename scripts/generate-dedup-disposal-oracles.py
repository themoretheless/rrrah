"""Encode independently composited Pillow GIF oracle frames as full-canvas APNG.

Reuses the authored CC0 oracle bytes, never rrrah decoder output.
"""
from pathlib import Path
import struct
import zlib
import hashlib
import json
root = Path(__file__).resolve().parent.parent
source = root / 'tests/fixtures/raster'
target = root / 'crates/rrrah-dedup/tests/fixtures/disposal'
target.mkdir(parents=True, exist_ok=True)
def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
manifest = [line.split('\t') for line in (source / 'gif-animation-manifest.tsv').read_text().splitlines() if not line.startswith('#')]
records = []
for mode in (1, 2, 3):
    data = b'\x89PNG\r\n\x1a\n'
    data += chunk(b'IHDR', struct.pack('>2I5B', 5, 3, 8, 6, 0, 0, 0))
    data += chunk(b'sRGB', b'\0')
    data += chunk(b'acTL', struct.pack('>2I', 4, 3))
    gif_name = f'gif-animation-disposal-{mode}.gif'
    rows = [row for row in manifest if row[0] == gif_name]
    assert len(rows) == 4
    gif_hash = hashlib.sha256((source / gif_name).read_bytes()).hexdigest()
    assert all(row[7] == gif_hash for row in rows)
    assert [int(row[4]) for row in rows] == [30, 70, 0, 110]
    assert all(row[2:4] == ['5', '3'] and row[5] == '2' for row in rows)
    sequence = 0
    oracles = []
    for index, delay in enumerate((30, 70, 0, 110)):
        name = f'gif-animation-disposal-{mode}-frame-{index}.rgba'
        pixels = (source / name).read_bytes()
        assert len(pixels) == 60
        oracles.append({'file': name, 'sha256': hashlib.sha256(pixels).hexdigest()})
        data += chunk(b'fcTL', struct.pack('>5I2H2B', sequence, 5, 3, 0, 0, delay, 1000, 0, 0))
        sequence += 1
        compressed = zlib.compress(b''.join(b'\0' + pixels[y*20:(y+1)*20] for y in range(3)))
        if index == 0:
            data += chunk(b'IDAT', compressed)
        else:
            data += chunk(b'fdAT', struct.pack('>I', sequence) + compressed)
            sequence += 1
    data += chunk(b'IEND', b'')
    name = f'disposal-{mode}.apng'
    (target / name).write_bytes(data)
    records.append({'disposal': mode, 'gif': gif_name, 'gif_sha256': gif_hash, 'apng': name, 'sha256': hashlib.sha256(data).hexdigest(), 'independent_oracles': oracles})
(target / 'manifest.json').write_text(json.dumps(records, indent=2) + '\n')
