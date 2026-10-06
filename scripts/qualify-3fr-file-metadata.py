#!/usr/bin/env python3
"""Compare native X1D metadata with independently parsed file tags (tifffile).

Sensor acceptance remains the independent LibRaw full-sensor comparison.
This checks file-origin calibration, not photographic color accuracy.
Usage: python qualify-3fr-file-metadata.py SOURCE NATIVE_JSON REPORT
"""
import hashlib
import json
import math
from pathlib import Path
import sys

import tifffile

source, native_path, report_path = map(Path, sys.argv[1:])
native = json.loads(native_path.read_text())
with tifffile.TiffFile(source) as container:
    root = container.pages[0]
    raw = root.pages[0]
    def ratios(values):
        return [values[i] / values[i + 1] for i in range(0, len(values), 2)]
    neutral = ratios(root.tags[50728].value)
    coefficients = ratios(root.tags[50721].value)
    expected = {
        'width': raw.imagewidth,
        'height': raw.imagelength,
        'wb': [neutral[1] / neutral[0], 1.0, neutral[1] / neutral[2], 1.0],
        'xyz_to_camera': [coefficients[i:i+3] for i in range(0, 9, 3)] + [[0.0]*3],
        'black_grid': ratios(raw.tags[50714].value),
        'white': [raw.tags[50717].value],
        'crop': list(raw.tags[50719].value) + list(raw.tags[50720].value),
    }

def flatten(value):
    if isinstance(value, list):
        return [x for element in value for x in flatten(element)]
    return [value]

problems = []
for key, value in expected.items():
    actual = flatten(native[key])
    expected_values = flatten(value)
    if len(actual) != len(expected_values) or any(
        not math.isfinite(a) or not math.isclose(a, b, rel_tol=1e-6, abs_tol=1e-7)
        for a, b in zip(actual, expected_values)
    ):
        problems.append(key)
report = {
    'oracle': 'tifffile', 'version': tifffile.__version__,
    'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
    'expected': expected, 'passed': not problems, 'problems': problems,
    'scope': 'Stored file calibration and crop; photographic color accuracy and CFA qualification separate',
}
report_path.write_text(json.dumps(report, indent=2) + '\n')
print('PASS' if not problems else 'FAIL: ' + ', '.join(problems))
sys.exit(bool(problems))
