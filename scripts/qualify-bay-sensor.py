#!/usr/bin/env python3
"""Synthetic sensor-only differential check; does not qualify real camera/color."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--oracle', type=Path, required=True,
                    help='External executable built from raw-fixture-oracle.cpp with LibRaw')
parser.add_argument('--native', type=Path, required=True,
                    help='Native bay_sensor_dump example executable')
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()
oracle, native = args.oracle.resolve(), args.native.resolve()
report = {'scope': 'synthetic BAY sensor unpack only; no real camera or color qualification',
          'cases': [], 'passed': False}
def sha(data):
    return hashlib.sha256(data).hexdigest()
def run(command):
    result = subprocess.run([str(p) for p in command], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f'{command}: {result.stderr}')
    return result.stdout
try:
    report['native_sha256'] = sha(native.read_bytes())
    report['oracle_sha256'] = sha(oracle.read_bytes())
    with tempfile.TemporaryDirectory(prefix='rrrah-bay-') as scratch:
        base = Path(scratch)
        for name, width, height in [('qv2000ux', 1632, 1211),
                                    ('qv3000ex', 2080, 1547),
                                    ('qv5700', 2585, 1924)]:
            data = bytearray()
            for row in range(height):
                if name == 'qv5700':
                    for col in range(0, width - 1, 4):
                        packed = sum(((row + col + i) % 1024) << (30 - i * 10)
                                     for i in range(4))
                        data.extend(packed.to_bytes(5, 'big'))
                    data.extend((((row + width - 1) % 1024) << 6 | 63).to_bytes(2, 'big'))
                else:
                    data.extend((row * width + col) % 256 for col in range(width))
            source = base / f'{name}.bay'
            source.write_bytes(data)
            prefix = base / f'{name}-oracle'
            run([oracle, source, prefix])
            metadata = json.loads(prefix.with_suffix('.json').read_text())
            oracle_width = 2585 if name == 'qv5700' else width
            if (metadata['width'], metadata['height']) != (oracle_width, height):
                raise RuntimeError(f'{name}: oracle dimensions differ')
            if name == 'qv5700' and metadata['crop'] != [0, 0, 2576, height]:
                raise RuntimeError('QV-5700: oracle active area differs')
            output = base / f'{name}-native.u16le'
            stdout = run([native, name, source, output])
            if 'managed_final=0' not in stdout:
                raise RuntimeError(f'{name}: missing managed release evidence')
            actual, full_reference = output.read_bytes(), prefix.with_suffix('.u16le').read_bytes()
            if len(full_reference) != oracle_width * height * 2:
                raise RuntimeError(f'{name}: oracle payload length differs')
            expected = full_reference
            if len(actual) != width * height * 2 or actual != expected:
                raise RuntimeError(f'{name}: complete sensor mismatch')
            report['cases'].append({'layout': name, 'samples': width * height,
                                    'source_sha256': sha(data), 'sensor_sha256': sha(actual),
                                    'libraw_version': metadata['libraw_version'],
                                    'oracle_storage_width': oracle_width,
                                    'compared_width': width,
                                    'excluded_reference_columns': oracle_width - width,
                                    'managed_final': 0, 'equal': True})
    report['passed'] = True
except Exception as error:
    report['error'] = str(error)
finally:
    args.report.write_text(json.dumps(report, indent=2) + '\n')
if not report['passed']:
    raise SystemExit(report['error'])
print(f"BAY synthetic full-storage equality: {len(report['cases'])} layouts passed")
