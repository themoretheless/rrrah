#!/usr/bin/env python3
"""Exact unit-pixel area oracle for the authored rectangular alpha mask."""
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1] / 'tests/fixtures/pdf'
values = bytearray()
for y in range(8):
    for x in range(16):
        area = max(0, min(x + 1, 8) - max(x, 0)) * max(0, min(y + 1, 8) - max(y, 0))
        values.extend([0, 0, 0, round(area * 255)])
path = root / 'soft-mask-alpha-without-group-cs.analytic.rgba'
path.write_bytes(values)
path.with_suffix('.json').write_text(json.dumps({
    'scope': 'Exact unit-pixel area intersection with authored opaque rectangle [0,8] x [0,8], 16x8 canvas',
    'sha256': hashlib.sha256(values).hexdigest(),
    'source_sha256': hashlib.sha256((root / 'soft-mask-alpha-without-group-cs.pdf').read_bytes()).hexdigest(),
    'license': 'CC0',
    'formula': 'max(0,min(x+1,8)-max(x,0)) * max(0,min(y+1,8)-max(y,0))',
}, indent=2) + '\n')
