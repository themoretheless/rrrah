#!/usr/bin/env python3
"""Self-authored RGB16 PNG controls; no external image encoder or oracle."""
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'


def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))


def main():
    samples = [0, 0, 0, 32768, 65535, 0, 0, 32768, 65535, 65535, 65535, 32768]
    row = b'\0' + struct.pack('>12H', *samples)
    for name, primary, transfer in [('linear-2020', 9, 8), ('linear-p3', 12, 8), ('encoded-p3', 12, 13), ('pq-2020', 9, 16), ('hlg-2020', 9, 18)]:
        data = b'\x89PNG\r\n\x1a\n'
        data += chunk(b'IHDR', struct.pack('>IIBBBBB', 3, 1, 16, 6, 0, 0, 0))
        data += chunk(b'cICP', bytes([primary, transfer, 0, 1]))
        data += chunk(b'IDAT', zlib.compress(row)) + chunk(b'IEND', b'')
        (ROOT / f'cicp-{name}.png').write_bytes(data)
    samples = [channel for code in [0, 4096, 32768, 60160, 65535]
               for channel in [code, code, code, 32768]]
    data = b'\x89PNG\r\n\x1a\n'
    data += chunk(b'IHDR', struct.pack('>IIBBBBB', 5, 1, 16, 6, 0, 0, 0))
    data += chunk(b'cICP', bytes([1, 1, 0, 0]))
    data += chunk(b'IDAT', zlib.compress(b'\0' + struct.pack('>20H', *samples))) + chunk(b'IEND', b'')
    (ROOT / 'cicp-narrow-709.png').write_bytes(data)
    for p3 in (False, True):
        samples = [channel for code in [0, 32768, 32769, 65535]
                   for channel in [code, 0 if p3 else code, 0 if p3 else code, 32768]]
        xy = [31270,32900,68000,32000,26500,69000,15000,6000] if p3 else [31270,32900,64000,33000,30000,60000,15000,6000]
        data = b'\x89PNG\r\n\x1a\n'
        data += chunk(b'IHDR', struct.pack('>IIBBBBB', 4, 1, 16, 6, 0, 0, 0))
        data += chunk(b'gAMA', struct.pack('>I', 50000)) + chunk(b'cHRM', struct.pack('>8I', *xy))
        data += chunk(b'IDAT', zlib.compress(b'\0' + struct.pack('>16H', *samples))) + chunk(b'IEND', b'')
        (ROOT / ('gamma-chrm-p3.png' if p3 else 'gamma-chrm-709.png')).write_bytes(data)

    samples = [channel for code in [0, 32768, 32769, 65535] for channel in [code, 32768]]
    data = b'\x89PNG\r\n\x1a\n'
    data += chunk(b'IHDR', struct.pack('>IIBBBBB', 4, 1, 16, 4, 0, 0, 0))
    data += chunk(b'gAMA', struct.pack('>I', 50000))
    data += chunk(b'IDAT', zlib.compress(b'\0' + struct.pack('>8H', *samples))) + chunk(b'IEND', b'')
    (ROOT / 'gamma-gray.png').write_bytes(data)

    for depth in (1, 2, 4):
        maximum = (1 << depth)-1
        row = bytearray(1+16*depth//8)
        for x in range(16):
            bit = x*depth
            row[1+bit//8] |= (x & maximum) << (8-depth-bit%8)
        data = b'\x89PNG\r\n\x1a\n'
        data += chunk(b'IHDR', struct.pack('>IIBBBBB', 16, 1, depth, 0, 0, 0, 0))
        data += chunk(b'gAMA', struct.pack('>I', 50000))
        data += chunk(b'tRNS', struct.pack('>H', 1))
        data += chunk(b'IDAT', zlib.compress(row)) + chunk(b'IEND', b'')
        (ROOT / f'gamma-gray-packed-{depth}.png').write_bytes(data)


if __name__ == '__main__':
    main()
