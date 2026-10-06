#!/usr/bin/env python3
"""Authored low-information and informative-hash collision challenges (Pillow)."""
import hashlib
import json
from pathlib import Path
from PIL import Image

ROOT = Path('crates/rrrah-dedup/tests/fixtures/information')
ROOT.mkdir(parents=True, exist_ok=True)
records = []
for name, pixels in [
    ('black', [(0, 0, 0, 255)] * 144**2),
    ('white', [(255, 255, 255, 255)] * 144**2),
    ('transparent-red', [(255, 0, 0, 0)] * 144**2),
    ('transparent-blue', [(0, 0, 255, 0)] * 144**2),
    ('gradient', [(40 + (x // 16) * 20,) * 3 + (255,) for y in range(144) for x in range(144)]),
    ('gradient-checker', [(40 + (x // 16) * 20 + (30 if (x + y) % 2 else -30),) * 3 + (255,) for y in range(144) for x in range(144)]),
]:
    image = Image.new('RGBA', (144, 144))
    image.putdata(pixels)
    path = ROOT / f'{name}.png'
    image.save(path)
    records.append({'name': name, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
(ROOT / 'manifest.json').write_text(json.dumps({'authored': True, 'width':144, 'height':144,
    'scope':'Synthetic information/collision controls, not captured bursts',
    'gradient':'nine 16-pixel vertical cells, encoded RGB=40+20*cell',
    'checker':'alternating encoded +/-30 at each pixel; monotonic area means preserve dHash signs',
    'expected_pixel_equal':[['transparent-red','transparent-blue']],
    'expected_pixel_different':[['black','white'],['gradient','gradient-checker']],
    'files': records}, indent=2)+'\n')
