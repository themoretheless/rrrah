#!/usr/bin/env python3
"""Author Gray ICC gamma-2 PNG/TIFF controls without a color conversion library."""
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'

def profile():
    header = bytearray(128)
    header[8:12] = bytes([2, 0x10, 0, 0])
    header[12:24] = b'mntrGRAYXYZ '
    header[24:36] = struct.pack('>6H', 2026, 10, 12, 0, 0, 0)
    header[36:40] = b'acsp'
    header[68:80] = struct.pack('>3i', 63190, 65536, 54061)
    tags = [(b'wtpt', b'XYZ ' + bytes(4) + header[68:80]),
            (b'kTRC', b'curv' + bytes(4) + struct.pack('>IH', 1, 512))]
    table = bytearray(struct.pack('>I', len(tags)))
    payload = bytearray()
    for signature, data in tags:
        offset = 128 + 4 + 12 * len(tags) + len(payload)
        table += signature + struct.pack('>II', offset, len(data))
        payload += data
        payload += bytes((-len(payload)) % 4)
    result = header + table + payload
    result[:4] = struct.pack('>I', len(result))
    return bytes(result)

def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))

icc = profile()
samples = [0, 32768, 32769, 65535]
alpha = [0, 16384, 32768, 65535]
row = b'\0' + b''.join(struct.pack('>HH', value, a) for value, a in zip(samples, alpha))
png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 4, 1, 16, 4, 0, 0, 0))
png += chunk(b'iCCP', b'Gray gamma 2\0\0' + zlib.compress(icc))
png += chunk(b'IDAT', zlib.compress(row)) + chunk(b'IEND', b'')
(ROOT / 'gray-gamma2-alpha16.png').write_bytes(png)
# Baseline uncompressed little-endian grayscale TIFF, one strip, embedded ICC.
tags = [(256,4,1,4),(257,4,1,1),(258,3,1,16),(259,3,1,1),(262,3,1,1),
        (273,4,1,0),(277,3,1,1),(278,4,1,1),(279,4,1,8),(284,3,1,1),(34675,7,len(icc),0)]
start = 8 + 2 + 12 * len(tags) + 4
strip = start + len(icc)
tags = [(tag,kind,count,strip if tag == 273 else start if tag == 34675 else value) for tag,kind,count,value in tags]
tiff = b'II' + struct.pack('<HIH',42,8,len(tags))
for tag,kind,count,value in tags:
    tiff += struct.pack('<HHII',tag,kind,count,value)
tiff += bytes(4) + icc + struct.pack('<4H',*samples)
(ROOT / 'gray-gamma2-16.tif').write_bytes(tiff)
