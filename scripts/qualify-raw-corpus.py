#!/usr/bin/env python3
"""Required real RAW acceptance; reference hashes were produced by independent LibRaw.
Build the native dumper with cargo build -p rrrah-decode --example raw_fixture_dump.
Files stay external to Git. No case is skipped and no reference is regenerated here.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile
import urllib.parse
import urllib.request


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def qualify(case, source, native, temporary, memory_mb=None):
    if sha256(source) != case['sha256']:
        raise ValueError('source SHA256 mismatch')
    prefix = temporary / str(case['id'])
    environment = os.environ.copy()
    if memory_mb is not None:
        environment['RRRAH_RAW_MEMORY_MB'] = str(memory_mb)
    subprocess.run([str(native), str(source), str(prefix)], check=True,
                   capture_output=True, text=True, timeout=180, env=environment)
    actual = json.loads(prefix.with_suffix('.json').read_text())
    expected = case['oracle']
    pixels = prefix.with_suffix('.u16le')
    problems = []
    if memory_mb is not None or 'managed_memory' in actual:
        memory = actual.get('managed_memory', {})
        if (memory.get('managed') is not True or not 0 < memory.get('used', 0) == memory.get('pixel_capacity')
                or not memory.get('used', 0) <= memory.get('peak', -1) <= memory.get('limit', -1)):
            problems.append('managed input/output reservation contract failed')
        if memory_mb is not None and memory.get('limit') != memory_mb * 1024 * 1024:
            problems.append('native managed budget differs from requested limit')
    for key in ('width', 'height', 'cfa'):
        if actual[key] != expected[key]:
            problems.append(f'{key}: {actual[key]} != {expected[key]}')
    if pixels.stat().st_size != expected['pixels_bytes'] or sha256(pixels) != expected['pixels_sha256']:
        problems.append('full sensor samples differ from independent LibRaw reference')
    planes = case.get('color_planes', 3)
    if planes not in (3, 4):
        raise ValueError('only explicit 3/4 camera plane contracts are supported')
    wb = case['metadata_contract'].get('wb', expected['wb'])
    if len(wb) != 4 or len(actual['wb']) != 4 or not math.isfinite(wb[1]) or wb[1] <= 0:
        raise ValueError('invalid reference WB contract')
    if planes == 4 and ('wb' not in case['metadata_contract'] or not case['metadata_contract'].get('wb_source')):
        raise ValueError('four-plane WB requires explicit independently sourced metadata contract')
    # Legacy Bayer oracles may omit G2; RGBE requires every explicitly resolved gain.
    for channel in range(planes):
        reference = wb[channel] / wb[1]
        if not math.isclose(actual['wb'][channel], reference, rel_tol=1e-6, abs_tol=1e-6):
            problems.append(f'WB channel {channel}: {actual["wb"][channel]} != {reference}')
    calibration = case['metadata_contract'].get('xyz_to_camera')
    if calibration is not None:
        # Preserve the original calibration contract as well as its RGB transform.
        # Equal normalized transforms alone can hide scaled or altered source rows.
        for matrix in (actual['xyz_to_camera'], calibration):
            if len(matrix) != 4 or any(len(row) != 3 for row in matrix):
                raise ValueError('XYZ-to-camera matrix must be 4x3')
        for row, (actual_row, reference_row) in enumerate(zip(actual['xyz_to_camera'], calibration)):
            for column, (value, reference) in enumerate(zip(actual_row, reference_row)):
                if not math.isclose(value, reference, rel_tol=1e-6, abs_tol=1e-6):
                    problems.append(f'XYZ-to-camera [{row},{column}]: {value} != {reference}')
    reference_rgb = expected['camera_to_rgb4'] if planes == 4 else expected['camera_to_rgb']
    for matrix in (actual['camera_to_rgb'], reference_rgb):
        if len(matrix) != 3 or any(len(row) != planes for row in matrix):
            raise ValueError(f'camera-to-sRGB matrix must be 3x{planes}')
    for row, (actual_row, reference_row) in enumerate(zip(actual['camera_to_rgb'], reference_rgb)):
        for column, (value, reference) in enumerate(zip(actual_row, reference_row)):
            if not math.isclose(value, reference, rel_tol=1e-6, abs_tol=1e-6):
                problems.append(f'camera→sRGB [{row},{column}]: {value} != {reference}')
    # These are source-tag / declared numeric-policy contracts, not LibRaw's
    # different initial active-area or combined normalization conventions.
    for key in ('crop', 'black_grid', 'white'):
        reference = case['metadata_contract'][key]
        if actual[key] != reference:
            problems.append(f'{key}: {actual[key]} != source/policy contract {reference}')
    return problems, actual


def main():
    repository = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('corpus', type=Path)
    parser.add_argument('--native', type=Path, default=repository / 'target/debug/examples/raw_fixture_dump')
    parser.add_argument('--include-local-cr3', action='store_true', help='Require both user-provided repository EOS R8 fixtures as well')
    parser.add_argument('--fetch', action='store_true', help='Download missing public CC0 fixtures, verifying hashes')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--memory-mb', type=int, help='Require budgeted source/output ownership during native decoding')
    args = parser.parse_args()
    if args.memory_mb is not None and args.memory_mb < 0:
        parser.error('--memory-mb must be nonnegative')
    if not args.native.is_file() or not os.access(args.native, os.X_OK):
        parser.error(f'native dumper is missing or not executable: {args.native}')
    manifest = json.loads((repository / 'tests/fixtures/raw-color-corpus.json').read_text())
    args.corpus.mkdir(parents=True, exist_ok=True)
    results = []
    with tempfile.TemporaryDirectory(prefix='rrrah-raw-qualification-') as directory:
        cases = manifest['cases'] + (manifest.get('local_cases', []) if args.include_local_cr3 else [])
        for case in cases:
            result = {'id': case['id'], 'model': case['model'], 'filename': case['filename']}
            try:
                source = (repository if case.get('local_source') else args.corpus) / case['filename']
                if args.fetch and not source.exists() and not case.get('local_source'):
                    if case['license'] != 'CC0-1.0':
                        raise ValueError('automatic download requires CC0')
                    url = urllib.parse.quote(case['url'], safe=':/?=&')
                    staging = source.with_suffix(source.suffix + '.part')
                    try:
                        with urllib.request.urlopen(url, timeout=60) as response, staging.open('wb') as output:
                            while chunk := response.read(1024 * 1024):
                                output.write(chunk)
                        if sha256(staging) != case['sha256']:
                            raise ValueError('download SHA256 mismatch')
                        staging.replace(source)
                    finally:
                        staging.unlink(missing_ok=True)
                problems, metadata = qualify(case, source, args.native.resolve(), Path(directory), args.memory_mb)
                result.update(passed=not problems, problems=problems, metadata=metadata)
            except (OSError, ValueError, KeyError, ZeroDivisionError, subprocess.SubprocessError) as error:
                detail = error.stderr.strip() if isinstance(error, subprocess.CalledProcessError) else str(error)
                result.update(passed=False, problems=[detail])
            results.append(result)
            print(f'{case["filename"]}: {"PASS" if result["passed"] else "FAIL"}', flush=True)
    report = {'scope': manifest['scope'], 'native_sha256': sha256(args.native), 'memory_mb': args.memory_mb, 'cases': results,
              'passed': all(result['passed'] for result in results)}
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
