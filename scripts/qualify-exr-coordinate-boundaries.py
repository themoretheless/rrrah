#!/usr/bin/env python3
"""Exercise public raster decoding at distant and invalid EXR coordinates."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
binary = args.binary.resolve()
source = root / 'tests/fixtures/raster/float-none-rec709-declared.exr'
base = source.read_bytes()
attribute = b'displayWindow\0box2i\0\x10\0\0\0'
offset = base.index(attribute) + len(attribute)
results = []
with tempfile.TemporaryDirectory(prefix='rrrah-exr-coordinate-') as directory:
    directory = Path(directory)
    for axis in [0, 1]:
        for position, valid in [(1073741818, True), (-1073741818, True),
                                (2147483645, False), (-2147483648, False)]:
            bounds = [0, 0, 2, 1]
            bounds[axis] = position
            bounds[axis + 2] = position + (2 if axis == 0 else 1)
            data = bytearray(base)
            struct.pack_into('<4i', data, offset, *bounds)
            path = directory / 'input.exr'
            output = directory / 'pixels.f32'
            output.unlink(missing_ok=True)
            path.write_bytes(data)
            process = subprocess.run([str(binary), str(path), '0', str(output), '--linear'],
                                     capture_output=True, text=True, timeout=60)
            if valid:
                assert process.returncode == 0, process.stderr
                assert output.read_bytes() == bytes(3 * 2 * 4 * 4)
            else:
                assert process.returncode > 0 and process.returncode != 101, process.stderr
                assert 'panicked' not in process.stderr and not output.exists()
                assert 'window size exceeding integer maximum' in process.stderr
            results.append({'bounds': bounds, 'expected_valid': valid, 'exit_code': process.returncode,
                            'stdout': process.stdout, 'stderr': process.stderr})
report = {'status': 'passed', 'cases': results, 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
          'fixture_sha256': hashlib.sha256(base).hexdigest(),
          'limitations': ['display-only metadata changes', 'no claim about all malformed EXR files',
                          'CPU public decode only, no GPU or live window']}
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(report, indent=2) + '\n')
print(f'{len(results)} EXR coordinate boundary cases passed')
