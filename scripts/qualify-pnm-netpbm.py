#!/usr/bin/env python3
"""Independent full-range PPM BT.709 preparation comparison against Netpbm."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--native', type=Path, required=True)
parser.add_argument('--pnmgamma', type=Path, required=True)
parser.add_argument('--netpbm-source', type=Path, required=True,
                    help='Official editor/pnmgamma.c for the documented continuous-toe model')
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()
native, oracle = args.native.resolve(), args.pnmgamma.resolve()
report = {'scope': 'authored full-range 16-bit standard PPM BT.709 to linear preparation',
          'passed': False}
def sha(data): return hashlib.sha256(data).hexdigest()
def run(command):
    result = subprocess.run([str(p) for p in command], capture_output=True, timeout=60)
    if result.returncode:
        raise RuntimeError(f'{command}: {result.stderr.decode(errors="replace")}')
    return result
try:
    report['native_sha256'] = sha(native.read_bytes())
    report['oracle_sha256'] = sha(oracle.read_bytes())
    report['oracle_source_sha256'] = sha(args.netpbm_source.read_bytes())
    report['oracle_source_url'] = 'https://svn.code.sf.net/p/netpbm/code/advanced/editor/pnmgamma.c'
    version = run([oracle, '-version'])
    report['oracle_version'] = (version.stdout + version.stderr).decode().strip()
    with tempfile.TemporaryDirectory(prefix='rrrah-pnm-oracle-') as temporary:
        root = Path(temporary)
        header = b'P6\n256 256\n65535\n'
        data = header + b''.join(struct.pack('>HHH', value, value, value) for value in range(65536))
        source = root / 'all-values.ppm'; source.write_bytes(data)
        destination = root / 'linear.rgba32f'
        native_log = run([native, source, '0', destination, '--linear'])
        if b'linear_rgba32f 256x256' not in native_log.stdout:
            raise RuntimeError('native output marker missing')
        reference = run([oracle, '-bt709tolinear', source]).stdout
        if not reference.startswith(header) or len(reference) != len(data):
            raise RuntimeError('oracle header or full payload length differs')
        native_bytes = destination.read_bytes()
        if len(native_bytes) != 65536 * 16:
            raise RuntimeError('native float payload length differs')
        actual = struct.unpack('<262144f', native_bytes)
        expected = struct.unpack('>196608H', reference[len(header):])
        maximum = 0
        nonlinear_maximum = 0
        nominal_error = 0.0
        model_deviation = 0
        toe_differences = 0
        compression = 0.018 / (1.099 * 0.018 ** 0.45 - 0.099)
        toe_last = math.floor(int(65535 * 0.018 + 0.5) / compression)
        for pixel in range(65536):
            if actual[pixel * 4 + 3] != 1.0:
                raise RuntimeError('alpha differs')
            for channel in range(3):
                value = actual[pixel * 4 + channel]
                encoded = pixel / 65535.0
                nominal = encoded / 4.5 if encoded < 0.081 else ((encoded + 0.099) / 1.099) ** (1 / 0.45)
                nominal_error = max(nominal_error, abs(value - nominal))
                model = pixel * compression if pixel <= toe_last else ((encoded + 0.099) / 1.099) ** (1 / 0.45) * 65535
                model_deviation = max(model_deviation, abs(int(model + 0.5) - expected[pixel * 3 + channel]))
                if not 0.0 <= value <= 1.0:
                    raise RuntimeError('nonfinite or unnormalized linear output')
                difference = abs(int(value * 65535 + 0.5) - expected[pixel * 3 + channel])
                maximum = max(maximum, difference)
                if pixel > toe_last:
                    nonlinear_maximum = max(nonlinear_maximum, difference)
                elif difference:
                    toe_differences += 1
        if nominal_error > 3e-7 or model_deviation > 1 or nonlinear_maximum > 1:
            raise RuntimeError(f'nominal/model/nonlinear check failed: {nominal_error}, {model_deviation}, {nonlinear_maximum}')
        report.update(source_sha256=sha(data), native_output_sha256=sha(native_bytes),
                      oracle_output_sha256=sha(reference), pixels=65536,
                      compared_rgb_values=196608, maximum_u16_code_deviation=maximum,
                      native_nominal_f64_max_absolute_error=nominal_error,
                      oracle_continuous_toe_model_max_code_deviation=model_deviation,
                      native_oracle_nonlinear_max_code_deviation=nonlinear_maximum,
                      toe_last_encoded_code=toe_last, toe_differing_rgb_values=toe_differences,
                      exact_oracle_equality=False,
                      difference_policy='Nominal ITU BT.709 retained; Netpbm continuous toe difference explicitly modeled, not hidden by widened tolerance',
                      passed=True)
except Exception as error:
    report['error'] = str(error)
finally:
    args.report.write_text(json.dumps(report, indent=2) + '\n')
if not report['passed']: raise SystemExit(report['error'])
print(f"PNM nominal/model checks passed: 196608 channels; Netpbm toe deviation {maximum}, nonlinear {nonlinear_maximum}")
