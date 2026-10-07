#!/usr/bin/env python3
"""Independent Python stdlib ASCII85 fixtures; never used by production decoding."""
import base64
import json
import platform
from pathlib import Path

cases = []
for length in [*range(33), 255, 256, 257, 4097]:
    raw = bytes((i * 73 + length * 11) % 256 for i in range(length))
    encoded = base64.a85encode(raw, adobe=False)
    assert base64.a85decode(encoded, adobe=False) == raw
    cases.append({'length': length, 'encoded': encoded.decode('ascii'), 'decoded_hex': raw.hex()})
for raw in [bytes(4), bytes(9), bytes([255])*4]:
    cases.append({'length': len(raw), 'encoded': base64.a85encode(raw).decode('ascii'), 'decoded_hex': raw.hex()})
Path('tests/fixtures/eps').mkdir(exist_ok=True)
Path('tests/fixtures/eps/ascii85-python-reference.json').write_text(json.dumps({
    'license': 'CC0-1.0', 'reference': 'Python standard library base64.a85encode/a85decode',
    'python_version': platform.python_version(), 'cases': cases,
}, indent=2)+'\n')
