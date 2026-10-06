#!/usr/bin/env python3
"""Attach an ICC profile without changing JPEG compressed samples (fixture tool)."""
import argparse
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("jpeg", type=Path)
parser.add_argument("profile", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()
source = args.jpeg.read_bytes()
profile = args.profile.read_bytes()
if source[:2] != b"\xff\xd8" or not profile:
    parser.error("JPEG SOI and nonempty ICC profile required")
# APP2 length includes its two-byte length but excludes the marker.
chunk_size = 65535 - 2 - 14
chunks = [profile[i:i + chunk_size] for i in range(0, len(profile), chunk_size)]
if len(chunks) > 255:
    parser.error("ICC profile exceeds JPEG chunk count")
segments = bytearray()
for index, chunk in enumerate(chunks, 1):
    payload = b"ICC_PROFILE\0" + bytes([index, len(chunks)]) + chunk
    segments.extend(b"\xff\xe2" + (len(payload) + 2).to_bytes(2, "big") + payload)
args.output.write_bytes(source[:2] + segments + source[2:])
