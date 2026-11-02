"""Small authored gamma-only PNG with asymmetric color and straight alpha."""
import pathlib
import struct
import zlib

def chunk(kind, data):
    return struct.pack('>I', len(data))+kind+data+struct.pack('>I', zlib.crc32(kind+data))
pixels = [(32768,0,0,32768),(0,32768,0,65535),(0,0,32768,0),(32768,16384,49152,32768)]
data = b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',4,1,16,6,0,0,0))
data += chunk(b'gAMA',struct.pack('>I',50000))
data += chunk(b'IDAT',zlib.compress(b'\0'+b''.join(struct.pack('>4H',*p) for p in pixels),9))+chunk(b'IEND',b'')
project=pathlib.Path(__file__).resolve().parents[3]
(project/'tests/fixtures/raster/gamma-only-cache-policy.png').write_bytes(data)
