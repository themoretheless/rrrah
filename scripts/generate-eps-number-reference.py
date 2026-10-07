#!/usr/bin/env python3
"""Independent integer-radix fixtures using Python integers, test-only."""
import json
import platform
from pathlib import Path

alphabet = '0123456789abcdefghijklmnopqrstuvwxyz'
values = [0, 1, 35, 127, 65535, 2147483647, 2147483648, 4294967295]
cases = []
for base in range(2, 37):
    for value in values:
        remaining, digits = value, ''
        while remaining:
            remaining, remainder = divmod(remaining, base)
            digits = alphabet[remainder]+digits
        digits = digits or '0'
        assert int(digits, base) == value
        signed = value if value < 2147483648 else value-4294967296
        cases.append({'word': f'{base}#{digits}', 'signed': signed})
Path('tests/fixtures/eps').mkdir(exist_ok=True)
Path('tests/fixtures/eps/radix-python-reference.json').write_text(json.dumps({
    'license': 'CC0-1.0', 'reference': 'Python int(digits, base), explicit unsigned32 to signed32',
    'python_version': platform.python_version(), 'cases': cases,
}, indent=2)+'\n')
