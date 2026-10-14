#!/usr/bin/env python3
"""Present six pinned raster fixtures in native Metal HDR windows, sequentially."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

def validate_log(text, code, source, width, height, aspect):
    completion = re.findall(r'^presentation complete: frames=(\d+) elapsed=([\d.]+)s$', text, re.MULTILINE)
    resize = re.findall(r'^live resize presentation: confirmed=true frames_after_resize=(\d+)$', text, re.MULTILINE)
    presentation_info = [line for line in text.splitlines() if line.startswith('display after present:')]
    hdr_active = any('high_dynamic_range: Some(true)' in line
                     and any(float(value) > 1. for value in re.findall(r'current: Some\(([0-9.]+)\)', line))
                     for line in presentation_info)
    source_line = f'native raster HDR surface: {width}x{height}; pixel_aspect=Some({aspect:.1f}); source={source}'
    return (code == 0 and hdr_active and len(completion) == 1 and len(resize) == 1
            and int(completion[0][0]) >= 60 and float(completion[0][1]) >= 3
            and int(resize[0]) >= 30 and source_line in text.splitlines()
            and 'native raster source credit after upload: 0' in text.splitlines()
            and 'presented: RGBA16Float / ExtendedSrgbLinear; no validation errors' in text.splitlines()
            and re.search(r'^adapter: .*backend: Metal[, }]', text, re.MULTILINE) is not None)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    binary = args.binary.resolve()
    fixtures = [
        'heif-physical-alpha.heic', 'heif-physical-rotate90.heic', 'heif-physical-mirror.heif',
        'float-shifted-window-negative.exr', 'float-shifted-window-positive-crop.exr',
        'float-shifted-window-translated.exr',
    ]
    args.output_dir.mkdir(parents=True, exist_ok=False)
    report = {'status': 'running', 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'cases': [], 'limitations': ['live probe, not full viewer navigation or prefetch',
                                         'presentation is not calibrated luminance/color measurement',
                                         'offscreen exact pixel tests are separate evidence']}
    expected = [(64, 48, 2.), (48, 64, 2.), (64, 48, 2.), (6, 5, 1.), (3, 3, 1.), (3, 2, 1.)]
    for name, (width, height, aspect) in zip(fixtures, expected):
        path = root / 'tests/fixtures/raster' / name
        env = os.environ.copy()
        for variable in ['RRRAH_HDR_RAW_SOURCE', 'RRRAH_HDR_MODEL_SOURCE']:
            env.pop(variable, None)
        env.update(RRRAH_HDR_RASTER_SOURCE=str(path), RRRAH_HDR_SURFACE_SECONDS='3',
                   RRRAH_HDR_RESIZE_PROBE='1', RRRAH_GPU_BACKEND='metal', RRRAH_GPU_VENDOR='any')
        log = args.output_dir / (name + '.log')
        with log.open('w') as stream:
            try:
                process = subprocess.run([str(binary)], env=env, stdout=stream, stderr=subprocess.STDOUT, timeout=55)
                code = process.returncode
            except subprocess.TimeoutExpired:
                code = None
        text = log.read_text()
        completion = re.search(r'presentation complete: frames=(\d+) elapsed=([\d.]+)s', text)
        resize = re.search(r'live resize presentation: confirmed=true frames_after_resize=(\d+)', text)
        valid = validate_log(text, code, str(path), width, height, aspect)
        valid = valid and hashlib.sha256(binary.read_bytes()).hexdigest() == report['binary_sha256']
        case = {'fixture': name, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                'exit_code': code, 'passed': valid, 'presented_frames': int(completion[1]) if completion else 0,
                'elapsed_seconds': float(completion[2]) if completion else None,
                'frames_after_resize': int(resize[1]) if resize else 0,
                'log_sha256': hashlib.sha256(log.read_bytes()).hexdigest()}
        report['cases'].append(case)
        report['status'] = 'running' if valid else 'failed'
        (args.output_dir / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(case), flush=True)
        if not valid:
            raise SystemExit(1)
    report['status'] = 'passed'
    report['total_presented_frames'] = sum(v['presented_frames'] for v in report['cases'])
    (args.output_dir / 'result.json').write_text(json.dumps(report, indent=2) + '\n')


if __name__ == "__main__":
    main()
