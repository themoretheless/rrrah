"""Independent libjpeg/Pillow readback of existing encoded real-photo fixtures."""
from pathlib import Path
import hashlib, json
from PIL import Image, features, __version__
root = Path(__file__).resolve().parents[1]
source = root / 'crates/rrrah-dedup/tests/fixtures/photos'
out = root / 'crates/rrrah-dedup/tests/fixtures/photos-jpeg-oracle'
out.mkdir(parents=True, exist_ok=True)
records = []
for scene in [830, 898, 1084, 1294]:
    path = source / f'{scene}-jpeg.jpg'
    image = Image.open(path).convert('RGB')
    dest = out / f'{scene}.png'
    image.save(dest)
    records.append({'scene': scene, 'input': f'../photos/{path.name}', 'input_sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'oracle': dest.name, 'oracle_sha256': hashlib.sha256(dest.read_bytes()).hexdigest(), 'dimensions': image.size})
(out / 'manifest.json').write_text(json.dumps({'scope': 'Pillow/libjpeg decoding of the exact same JPEG bytes, not comparison to the uncompressed source', 'generator': Path(__file__).name, 'pillow': __version__, 'libjpeg': features.version('jpg'), 'records': records}, indent=2) + '\n')
