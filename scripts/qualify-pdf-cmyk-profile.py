#!/usr/bin/env python3
"""Compare an authored CMYK strip fixture against independent LittleCMS.

CMYK inputs are quantized to 8 bits for Pillow, so this is a bounded diagnostic,
not a strict PDF renderer or float-color oracle.
"""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image, ImageCms, __version__ as pillow_version

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--profile', type=Path, required=True)
parser.add_argument('--native', type=Path, required=True)
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
fixture = root / 'tests/fixtures/pdf/gradient-center-strips-cmyk.pdf'
manifest = json.loads(fixture.with_suffix('.json').read_text())
digest = lambda data: hashlib.sha256(data).hexdigest()
if digest(fixture.read_bytes()) != manifest['sha256']:
    raise SystemExit('fixture hash mismatch')
profile_bytes = args.profile.read_bytes()
expected_profile = '73e1ba37d2bad5bab2a964f40a9eed96209666efc067c3322626214bbef234a0'
if digest(profile_bytes) != expected_profile:
    raise SystemExit('profile hash mismatch')
native = args.native.read_bytes()
if len(native) != 12 * 8 * 4 or any(native[i] != 255 for i in range(3, len(native), 4)):
    raise SystemExit('expected opaque 12x8 RGBA output')
source = ImageCms.ImageCmsProfile(str(args.profile))
target = ImageCms.createProfile('sRGB')
samples = [(int((1-(x+.5)/12)*255+.5), int((x+.5)/12*255+.5), 0, 0) for x in range(12)]
image = Image.new('CMYK', (12, 8))
image.putdata(samples * 8)
actual = bytes(v for i,v in enumerate(native) if i % 4 != 3)
rows = []
for intent in range(4):
    transform = ImageCms.buildTransformFromOpenProfiles(source, target, 'CMYK', 'RGB', renderingIntent=intent)
    reference = ImageCms.applyTransform(image, transform).tobytes()
    errors = [abs(a-b) for a,b in zip(actual, reference)]
    rows.append({'intent': intent, 'max_channel_difference': max(errors),
                 'different_pixels': sum(actual[i:i+3] != reference[i:i+3] for i in range(0,len(actual),3)),
                 'reference_first_row_hex': reference[:36].hex()})
report = {'fixture_sha256': manifest['sha256'], 'profile_sha256': expected_profile,
          'profile_bytes': len(profile_bytes), 'native_sha256': digest(native),
          'pillow_version': pillow_version, 'littlecms_version': ImageCms.core.littlecms_version,
          'quantized_cmyk_samples': samples, 'results': rows,
          'limitations': ['Independent CMM with identical ICC bytes; Pillow input is 8-bit CMYK while PDF input is float.',
                          'One opaque strip fixture; no general PDF/AI, blend, mask or rendering-intent qualification.',
                          'Does not change or relax the strict Poppler AI gate.']}
args.report.write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps(rows))
if any(row['max_channel_difference'] > 1 for row in rows):
    raise SystemExit(1)
