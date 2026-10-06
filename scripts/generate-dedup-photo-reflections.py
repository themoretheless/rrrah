"""Lossless Pillow reflections of pinned CC0 embedded-camera-preview fixtures."""
from pathlib import Path
import hashlib, json
from PIL import Image, __version__
root = Path(__file__).resolve().parents[1] / 'crates/rrrah-dedup/tests/fixtures/photos'
manifest = json.loads((root / 'manifest.json').read_text())
records = []
for case in manifest['cases']:
    base = next(f for f in case['files'] if f['variant'] == 'base')
    source = root / base['file']
    assert hashlib.sha256(source.read_bytes()).hexdigest() == base['sha256']
    target = root / f"{case['source']['id']}-mirror.png"
    image = Image.open(source)
    image.transpose(Image.Transpose.FLIP_LEFT_RIGHT).save(target)
    brightness = next(f for f in case['files'] if f['variant'] == 'brightness')
    brightness_source = root / brightness['file']
    assert hashlib.sha256(brightness_source.read_bytes()).hexdigest() == brightness['sha256']
    brightness_target = root / f"{case['source']['id']}-mirror-brightness.png"
    Image.open(brightness_source).transpose(Image.Transpose.FLIP_LEFT_RIGHT).save(brightness_target)
    records.append({'id': case['source']['id'], 'license': case['source']['license'], 'input': base, 'output': target.name, 'brightness_input': brightness, 'brightness_output': brightness_target.name, 'brightness_sha256': hashlib.sha256(brightness_target.read_bytes()).hexdigest(), 'dimensions': image.size, 'sha256': hashlib.sha256(target.read_bytes()).hexdigest()})
(root / 'reflections-manifest.json').write_text(json.dumps({'generator': Path(__file__).name, 'pillow': __version__, 'scope': 'Known embedded rendered camera previews; not held-out photographs or full RAW development', 'cases': records}, indent=2)+'\n')
