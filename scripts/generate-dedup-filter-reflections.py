"""Losslessly mirror pinned JPEG derivatives for reflected filtered verification."""
from pathlib import Path
import hashlib
import json
from PIL import Image, __version__

root = Path(__file__).resolve().parents[1] / 'crates/rrrah-dedup/tests/fixtures/photos'
manifest = json.loads((root / 'manifest.json').read_text())
records = []
for case in manifest['cases']:
    source_record = next(f for f in case['files'] if f['variant'] == 'jpeg')
    source = root / source_record['file']
    assert hashlib.sha256(source.read_bytes()).hexdigest() == source_record['sha256']
    target = root / f"{case['source']['id']}-mirror-jpeg.png"
    with Image.open(source) as image:
        image.transpose(Image.Transpose.FLIP_LEFT_RIGHT).save(target)
        dimensions = image.size
    records.append({'id': case['source']['id'], 'license': case['source']['license'],
                    'input': source_record, 'output': target.name, 'dimensions': dimensions,
                    'sha256': hashlib.sha256(target.read_bytes()).hexdigest()})
(root / 'filtered-reflections-manifest.json').write_text(json.dumps({
    'generator': Path(__file__).name, 'pillow': __version__,
    'scope': 'Known camera-preview JPEG derivatives mirrored losslessly; not held-out photographs or full RAW development.',
    'cases': records,
}, indent=2) + '\n')
