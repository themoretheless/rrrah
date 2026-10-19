#!/usr/bin/env python3
"""Produce CC0 boundary EXR and bit oracle with independent OpenEXR C++."""
from pathlib import Path
import hashlib
import json
import os
import shlex
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
source = root / 'scripts/exr-alpha-overflow-oracle.cpp'
fixture = root / 'tests/fixtures/raster/associated-overflow-rec709.exr'
with tempfile.TemporaryDirectory() as directory:
    binary = Path(directory) / 'oracle'
    flags = shlex.split(subprocess.check_output(
        ['pkg-config', '--cflags', '--libs', 'OpenEXR'], text=True))
    subprocess.run([os.environ.get('CXX', 'c++'), '-std=c++17', str(source),
                    *flags, '-o', str(binary)], check=True)
    subprocess.run([str(binary), str(fixture)], check=True)
files = [source, fixture, Path(str(fixture) + '.native-rgba32le')]
meta = {
    'reference': 'OpenEXR ' + subprocess.check_output(
        ['pkg-config', '--modversion', 'OpenEXR'], text=True).strip(),
    'license': 'CC0 authored samples',
    'case': 'Last-pixel finite RGB/positive-alpha quotient overflow; earlier opaque/fractional/zero samples and signed zero remain associated bit-exact',
    'sha256': {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
               for p in files},
}
(root / 'tests/fixtures/raster/exr-associated-overflow.json').write_text(
    json.dumps(meta, indent=2) + '\n')
