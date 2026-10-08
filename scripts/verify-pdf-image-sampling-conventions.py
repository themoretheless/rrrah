#!/usr/bin/env python3
"""Diagnose two sampling conventions on authored 2x1 PDF image interior row.

This does not change a fidelity gate or establish general PDF conformance.
"""
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('directory', type=Path)
args = parser.parse_args()
root = args.directory
native = (root / 'bilinear.rgba').read_bytes()
ppm = (root / 'bilinear.ppm').read_bytes().split(b'\n', 3)
assert ppm[:3] == [b'P6', b'32 16', b'255']
reference = ppm[3]
rows = []
for x in range(16):
    # Texel centers: inverse PDF transform maps target pixel center into
    # a width-two texture. Premultiplied child constant alpha is 128/255.
    centered = round(255 * min(1, max(0, 2 * (x + 0.5) / 16 - 0.5)))
    centered_output = 127 + (centered * 128 + 127) // 255
    # Candidate origin-aligned Splash expansion over a 17-pixel scan extent.
    # Inference from source and measured ramp, not installed binary source audit.
    origin = int(255 * min(1, 2 * x / 17))
    origin_output = 127 + (origin * 128 + 127) // 255
    measured_native = native[(8 * 32 + x) * 4]
    measured_reference = reference[(8 * 32 + x) * 3]
    assert centered_output == measured_native, (x, centered_output, measured_native)
    assert abs(origin_output - measured_reference) <= 1, (x, origin_output, measured_reference)
    rows.append(dict(x=x, centered=centered_output, prototype=measured_native,
                     origin=origin_output, poppler=measured_reference))
print(json.dumps({'scope': 'Interior row only; both coordinate models reproduce their measured ramps.', 'rows': rows}, indent=2))
