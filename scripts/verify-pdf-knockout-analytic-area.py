#!/usr/bin/env python3
"""Compare authored fractional rectangles to exact pixel-area knockout algebra.

Input is the explicit test-only interpreter diagnostic directory, not production
renderer output. This oracle establishes this rectangle model only.
"""
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('directory', type=Path)
args = parser.parse_args()
rows = []
for name in ('pdf-knockout-fractional-isolated', 'pdf-knockout-fractional-nonisolated'):
    data = (args.directory / (name + '.rgba')).read_bytes()
    assert len(data) == 32 * 16 * 4
    errors = []
    for y in range(16):
        for x in range(32):
            # Exact overlap of a unit pixel and the authored blue rectangle.
            shape = max(0, min(x + 1, 31.5) - max(x, 8.5)) * max(0, min(y + 1, 15.5) - max(y, 0.5))
            alpha = shape / 8
            previous = (1, 1, 1) if x >= 24 else (1, 15 / 16, 15 / 16)
            expected = [round(255 * ((alpha if c == 2 else 0) + shape - alpha + (1 - shape) * previous[c])) for c in range(3)]
            pixel = data[(y * 32 + x) * 4:(y * 32 + x + 1) * 4]
            assert pixel[3] == 255
            errors.extend(abs(pixel[c] - expected[c]) for c in range(3))
    rows.append({'fixture': name, 'channels': len(errors), 'max_error': max(errors)})
    assert max(errors) == 0, rows[-1]
print(json.dumps(rows, indent=2))
