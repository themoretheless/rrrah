#!/usr/bin/env python3
"""Independent stdlib raw-composite oracle; no PSD reader/runtime dependency."""
from pathlib import Path
import struct
import sys
source, target = map(Path, sys.argv[1:])
b = source.read_bytes()
assert b[:4] == b'8BPS'
version = int.from_bytes(b[4:6], 'big')
assert version in (1, 2)
channels = int.from_bytes(b[12:14], 'big')
h, w, depth, mode = struct.unpack('>IIHH', b[14:26])
assert depth in (16, 32) and mode in (1, 3)
assert channels == (1 if mode == 1 else 3)
at = 26
for section in range(3):
    size = 8 if version == 2 and section == 2 else 4
    length = int.from_bytes(b[at:at+size], 'big')
    at += size + length
assert b[at:at+2] == b'\0\0'
raw = b[at+2:]
size = depth // 8
assert len(raw) == channels * w * h * size
out = bytearray()
for pixel in range(w*h):
    for c in range(3):
        channel = c if mode == 3 else 0
        start = (channel*w*h+pixel)*size
        out += raw[start:start+size][::-1]
    out += struct.pack('<H', 65535) if depth == 16 else struct.pack('<f', 1.)
target.write_bytes(out)
