#!/usr/bin/env python3
"""Exact Fraction oracle for authored axis-plane segment intersections."""
import fractions
import random
import struct
from pathlib import Path

root = Path(__file__).resolve().parents[1]
rng = random.Random(20261021)
rows = []
for axis in range(2):
    for cut in [0.0, 1.0]:
        for _ in range(128):
            a = [rng.uniform(0.1, 0.9), rng.uniform(0.1, 0.9)]
            b = [rng.uniform(0.1, 0.9), rng.uniform(0.1, 0.9)]
            a[axis] = cut - rng.uniform(0.01, 8.0)
            b[axis] = cut + rng.uniform(0.01, 8.0)
            rows.append((a, b, axis, cut))
rows += [([-1.0, 1e20], [1e-20, 1.0], 0, 0.0),
         ([-1e-200, 0.1], [1e200, 0.9], 0, 0.0),
         ([-1e200, 0.1], [1e-200, 0.9], 0, 0.0)]

def bits(value):
    return f"0x{struct.unpack('>Q', struct.pack('>d', value))[0]:016x}"

lines = []
for a, b, axis, cut in rows:
    af = list(map(fractions.Fraction.from_float, a))
    bf = list(map(fractions.Fraction.from_float, b))
    cf = fractions.Fraction.from_float(cut)
    t = (cf - af[axis]) / (bf[axis] - af[axis])
    expected = [float(af[k] + t * (bf[k] - af[k])) for k in range(2)]
    lines.append('    ([' + ', '.join(map(bits, a + b)) + f'], {axis}, {bits(cut)}, ['
                 + ', '.join(map(bits, expected)) + ']),')
(root / 'tests/fixtures/geometry/clip-intersection-cases.rs').write_text('[\n' + '\n'.join(lines) + '\n]\n')
print(f"Wrote {len(lines)} exact-rational intersection controls")
