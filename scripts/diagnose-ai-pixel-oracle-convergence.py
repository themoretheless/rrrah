#!/usr/bin/env python3
"""Independent crop/MediaBox experiment; never qualifies production AI fidelity."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import struct
from pypdf import PdfReader, PdfWriter
from pypdf.generic import RectangleObject
from PIL import Image
import numpy as np

parser = argparse.ArgumentParser()
parser.add_argument('--oracle', type=Path, required=True)
parser.add_argument('--output-dir', type=Path, required=True)
parser.add_argument('--scales', default='64,128,256,512')
parser.add_argument('--modes', default='crop-only,media-and-crop')
parser.add_argument('--timeout', type=int, default=55)
parser.add_argument('--alias-perceptual-table', action='store_true', help='Diagnostic only: expose the existing A2B0 LUT as A2B1 for Poppler input validation')
args = parser.parse_args()
scales = [int(v) for v in args.scales.split(',')]
assert len(set(scales)) == len(scales) and scales == sorted(scales)
assert all(1 <= v <= 1024 for v in scales)
modes = args.modes.split(',')
assert modes and len(set(modes)) == len(modes) and all(v in ['crop-only', 'media-and-crop'] for v in modes)
assert 1 <= args.timeout <= 180
root = Path(__file__).resolve().parents[1]
source = root / 'tests/fixtures/data/metal-recovery/ai-corpus/VectorApple.ai'
profile = root / 'vendor/hayro-interpret/assets/CGATS001Compat-v2-micro.icc'
assert hashlib.sha256(source.read_bytes()).hexdigest() == 'c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06'
args.output_dir.mkdir(parents=True, exist_ok=False)
original_profile = profile
if args.alias_perceptual_table:
    original = profile.read_bytes()
    count, = struct.unpack_from('>I', original, 128)
    table_end = 132 + count * 12
    tags = [(original[132 + i * 12:136 + i * 12], *struct.unpack_from('>II', original, 136 + i * 12)) for i in range(count)]
    assert original[12:16] == b'scnr' and original[16:20] == b'CMYK'
    assert not any(tag == b'A2B1' for tag, _, _ in tags)
    lut = next((offset, size) for tag, offset, size in tags if tag == b'A2B0')
    content = bytearray(original[:132])
    struct.pack_into('>I', content, 0, len(original) + 12)
    struct.pack_into('>I', content, 128, count + 1)
    content[84:100] = bytes(16)
    for tag, offset, size in tags + [(b'A2B1', *lut)]:
        assert offset >= table_end
        content.extend(tag + struct.pack('>II', offset + 12, size))
    content.extend(original[table_end:])
    profile = args.output_dir / 'diagnostic-a2b1-alias.icc'
    profile.write_bytes(content)
reader = PdfReader(source)
box = [199, 219.5, 200, 220.5]
report = {'scope': 'Original resources and content, derived one-pixel page box experiment',
          'oracle': str(args.oracle.resolve()), 'box': box, 'runs': [],
          'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
          'profile_sha256': hashlib.sha256(profile.read_bytes()).hexdigest(),
          'original_profile_sha256': hashlib.sha256(original_profile.read_bytes()).hexdigest(),
          'perceptual_lut_aliased_to_relative': args.alias_perceptual_table,
          'oracle_sha256': hashlib.sha256(args.oracle.read_bytes()).hexdigest(),
          'limitations': ['derived page, not full-page fidelity', 'Poppler rendering is an independent comparator, not exact analytical area',
                          'changing MediaBox may affect page-dependent clipping and group behavior',
                          'no native renderer acceptance from this experiment',
                          'optional A2B1 alias exposes original perceptual LUT; this is a diagnostic profile derivative']}
for mode in modes:
    writer = PdfWriter()
    writer.add_page(reader.pages[0])
    writer.pages[0].cropbox = RectangleObject(box)
    if mode == 'media-and-crop':
        writer.pages[0].mediabox = RectangleObject(box)
    pdf = args.output_dir / f'{mode}.pdf'
    with pdf.open('wb') as stream:
        writer.write(stream)
    for scale in scales:
        prefix = args.output_dir / f'{mode}-{scale}'
        command = [str(args.oracle.resolve()), '-r', str(72 * scale), '-f', '1', '-l', '1', '-singlefile',
                   '-cropbox', '-defaultcmykprofile', str(profile), str(pdf), str(prefix)]
        try:
            process = subprocess.run(command, capture_output=True, text=True, timeout=args.timeout)
            ppm = prefix.with_suffix('.ppm')
            pixels = np.asarray(Image.open(ppm), dtype=np.float64) if process.returncode == 0 and ppm.exists() else None
            valid = process.returncode == 0 and not process.stderr.strip() and pixels is not None and pixels.shape == (scale, scale, 3)
            item = {'mode': mode, 'scale': scale, 'valid': valid, 'exit_code': process.returncode,
                    'stderr': process.stderr, 'mean_rgb': pixels.mean(axis=(0, 1)).tolist() if valid else None,
                    'ppm_sha256': hashlib.sha256(ppm.read_bytes()).hexdigest() if ppm.exists() else None}
        except subprocess.TimeoutExpired:
            item = {'mode': mode, 'scale': scale, 'valid': False, 'timeout_seconds': args.timeout}
        report['runs'].append(item)
        (args.output_dir / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(item), flush=True)
report['terminal'] = True
for mode in modes:
    valid = [v for v in report['runs'] if v['mode'] == mode and v['valid']]
    report[f'{mode}-last-scale-delta'] = max(abs(a-b) for a,b in zip(valid[-1]['mean_rgb'],valid[-2]['mean_rgb'])) if len(valid)>1 else None
report['derived_pdf_sha256'] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in args.output_dir.glob('*.pdf')}
(args.output_dir / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
