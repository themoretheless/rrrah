#!/usr/bin/env python3
"""Generate bounded textured HDR/alpha collisions; no inference or qualification."""
from pathlib import Path
import hashlib
import json
import shutil
import struct
from PIL import Image
import PIL

ROOT = Path(__file__).resolve().parents[1]
BASE = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
WORK = BASE / 'projected-loftr-controls-20261011'
MANIFEST = ROOT / 'docs/research/dedup-projected-loftr-fixtures-20261011.json'

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def pfm(image, highlight):
    width, height = image.size
    out = bytearray(f'PF\n{width} {height}\n-1.0\n'.encode())
    pixels = image.load()
    for y in range(height - 1, -1, -1):
        for x in range(width):
            rgb = pixels[x, y]
            for c in rgb:
                s = c / 255
                linear = s / 12.92 if s <= 0.04045 else ((s + 0.055) / 1.055) ** 2.4
                value = highlight if 80 <= x < 240 and 60 <= y < 180 else 4 * linear
                out.extend(struct.pack('<f', value))
    return out

def qoi(image, alpha):
    width, height = image.size
    out = bytearray(b'qoif' + struct.pack('>II', width, height) + bytes([4, 1]))
    pixels = image.load()
    for y in range(height):
        for x in range(width):
            patch = 80 <= x < 240 and 60 <= y < 180
            rgb = (0, 0, 0) if patch else pixels[x, y]
            out.extend(bytes([255, *rgb, alpha if patch else 255]))
    out.extend(bytes([0, 0, 0, 0, 0, 0, 0, 1]))
    return out

def main():
    assert not WORK.exists() and not MANIFEST.exists()
    WORK.mkdir()
    originals = [BASE / 'original-resolution-negative-200201' / name for name in ['209400.jpg', '209500.jpg']]
    images = [Image.open(p).convert('RGB').resize((320, 240), Image.Resampling.LANCZOS) for p in originals]
    files = {
        'hdr.pfm': pfm(images[0], 100.),
        'hdr-collision.pfm': pfm(images[0], 110.),
        'hdr-unrelated.pfm': pfm(images[1], 100.),
        'alpha.qoi': qoi(images[0], 64),
        'alpha-collision.qoi': qoi(images[0], 255),
    }
    for name, data in files.items():
        (WORK / name).write_bytes(data)
    for source, target in [('hdr.pfm', 'hdr-copy.pfm'), ('alpha.qoi', 'alpha-copy.qoi')]:
        shutil.copy2(WORK / source, WORK / target)
    assert sha(WORK / 'hdr.pfm') == sha(WORK / 'hdr-copy.pfm')
    assert sha(WORK / 'alpha.qoi') == sha(WORK / 'alpha-copy.qoi')
    manifest = {
        'status': 'generated_unqualified_textured_hdr_alpha_controls',
        'work': str(WORK), 'dimensions': [320, 240], 'patch': [80, 60, 160, 120],
        'patch_pixels': 19200, 'pillow_version': PIL.__version__,
        'source_pins': {str(p): sha(p) for p in [*originals, Path(__file__)]},
        'fixture_pins': {str(p): sha(p) for p in sorted(WORK.iterdir())},
        'hdr_recipe': 'PFM bottom-up little-endian f32; inverse sRGB of resized RGB times four; patch 100 versus 110 linear RGB.',
        'alpha_recipe': 'QOI linear RGB tag; identical opaque texture, black patch alpha 64/255 versus 1.',
        'scope': 'Generated controls only. Actual Rust projected-byte equality and native inference must be measured separately.',
    }
    MANIFEST.write_text(json.dumps(manifest, indent=2) + '\n')
    print(len(manifest['fixture_pins']), 'fixtures generated')

if __name__ == '__main__':
    main()
